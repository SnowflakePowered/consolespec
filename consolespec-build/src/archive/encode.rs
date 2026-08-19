//! Building an archive from a [`Definitions`] tree.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    HEADER_LEN, MAGIC, SECTION_ENTRY_LEN, SectionKind, VERSION,
    codec::{Column, Writer},
};
use crate::{Definitions, DirEntry, DirEntryKind, Document, Error, PartitionSpec, Result};

/// Level 19 keeps the window inside 8 MiB and lands within a hundred bytes of
/// level 22 on this data, because what remains after interning is digests.
pub const DEFAULT_LEVEL: i32 = 19;

pub fn write(definitions: &Definitions, level: i32) -> Result<Vec<u8>> {
    let specs = &definitions.partition_specs;
    if specs
        .windows(2)
        .any(|pair| pair[0].reference >= pair[1].reference)
    {
        return Err(Error::new(
            "partition specs must be sorted by reference and unique",
        ));
    }

    let sections = [
        (SectionKind::Documents, documents(definitions)),
        (SectionKind::PartitionIndex, partition_index(specs)),
        (SectionKind::PartitionData, partition_data(specs)?),
    ];

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&(sections.len() as u16).to_le_bytes());

    let mut payloads = Vec::with_capacity(sections.len());
    let mut offset = (HEADER_LEN + sections.len() * SECTION_ENTRY_LEN) as u64;
    for (kind, payload) in &sections {
        let compressed = zstd::bulk::compress(payload, level).map_err(|error| {
            Error::new(format!("compressing the {} section: {error}", kind.name()))
        })?;
        bytes.extend_from_slice(&kind.code().to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
        offset += compressed.len() as u64;
        payloads.push(compressed);
    }
    for payload in payloads {
        bytes.extend_from_slice(&payload);
    }
    Ok(bytes)
}

fn documents(definitions: &Definitions) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.u32(definitions.inputs.len() as u32);
    writer.u32(definitions.machines.len() as u32);
    let all = || definitions.inputs.iter().chain(&definitions.machines);
    writer.strings(all().map(|document: &Document| document.name.as_str()));
    for document in all() {
        writer.uleb(document.source.len() as u64);
    }
    for document in all() {
        writer.raw(document.source.as_bytes());
    }
    writer.bytes
}

fn partition_index(specs: &[PartitionSpec]) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.u32(specs.len() as u32);
    writer.strings(specs.iter().map(|spec| spec.reference.as_str()));
    writer.bytes
}

fn partition_data(specs: &[PartitionSpec]) -> Result<Vec<u8>> {
    // Paths first: every path, plus the ancestors needed to reach it, so the
    // trie the reader walks is always closed.
    let mut interior = BTreeSet::new();
    for spec in specs {
        for entry in &spec.entries {
            let mut path = entry.path.as_str();
            while path != "." && interior.insert(path.to_owned()) {
                match path.rsplit_once('/') {
                    Some((parent, _)) if !parent.is_empty() => path = parent,
                    _ => break,
                }
            }
        }
    }
    // The root is id 0; the rest follow in lexicographic order, where a parent
    // is a proper prefix of its children and so always precedes them. Ids can
    // therefore point backwards unconditionally.
    let paths = std::iter::once(".".to_owned())
        .chain(interior)
        .collect::<Vec<_>>();
    let path_ids = index(&paths);

    let mut components = BTreeSet::new();
    for path in paths.iter().skip(1) {
        components.insert(component_of(path));
    }
    let components = components.into_iter().collect::<Vec<_>>();
    let component_ids = index(&components);

    let entries = specs
        .iter()
        .flat_map(|spec| &spec.entries)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let entry_ids = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| (*entry, index as u64))
        .collect::<BTreeMap<_, _>>();

    let links = entries
        .iter()
        .filter_map(|entry| entry.link.as_deref())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let link_ids = index(&links);
    // Sizes are sorted so the table itself delta-encodes; digests are random,
    // so their tables are ordered by first use instead and it is the entry
    // columns that turn into runs of ones.
    let sizes = entries
        .iter()
        .filter_map(|entry| entry.size)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let size_ids = index(&sizes);
    let md5 = first_use(&entries, |entry| entry.md5);
    let sha1 = first_use(&entries, |entry| entry.sha1);
    let sha256 = first_use(&entries, |entry| entry.sha256);

    let mut writer = Writer::default();
    writer.u32(components.len() as u32);
    writer.strings(components.iter().copied());

    writer.u32(paths.len() as u32);
    let mut parents = Column::default();
    let mut names = Column::default();
    for path in paths.iter().skip(1) {
        let parent = match path.rsplit_once('/') {
            Some((parent, _)) if !parent.is_empty() => parent,
            _ => ".",
        };
        parents.push_required(path_ids[parent]);
        names.push_required(component_ids[component_of(path)]);
    }
    writer.raw(&parents.finish());
    writer.raw(&names.finish());

    writer.u32(links.len() as u32);
    writer.strings(links.iter().copied());

    writer.u32(sizes.len() as u32);
    let mut previous = 0;
    for size in &sizes {
        writer.uleb(size - previous);
        previous = *size;
    }

    for (values, width) in [
        (md5.0.concat(), 16),
        (sha1.0.concat(), 20),
        (sha256.0.concat(), 32),
    ] {
        writer.u32((values.len() / width) as u32);
        writer.raw(&values);
    }

    writer.u32(entries.len() as u32);
    let mut kinds = Vec::with_capacity(entries.len());
    let mut path_column = Column::default();
    let mut size_column = Column::default();
    let mut link_column = Column::default();
    let mut md5_column = Column::default();
    let mut sha1_column = Column::default();
    let mut sha256_column = Column::default();
    for entry in &entries {
        kinds.push(match entry.kind {
            DirEntryKind::Directory => 0,
            DirEntryKind::File => 1,
            DirEntryKind::Link => 2,
        });
        path_column.push_required(*path_ids.get(entry.path.as_str()).ok_or_else(|| {
            Error::new(format!(
                "entry path `{}` is missing from the trie",
                entry.path
            ))
        })?);
        size_column.push(entry.size.map(|size| size_ids[&size]));
        link_column.push(entry.link.as_deref().map(|link| link_ids[link]));
        md5_column.push(entry.md5.map(|digest| md5.1[&digest]));
        sha1_column.push(entry.sha1.map(|digest| sha1.1[&digest]));
        sha256_column.push(entry.sha256.map(|digest| sha256.1[&digest]));
    }
    writer.raw(&kinds);
    for column in [
        path_column,
        size_column,
        link_column,
        md5_column,
        sha1_column,
        sha256_column,
    ] {
        writer.raw(&column.finish());
    }

    writer.u32(specs.len() as u32);
    for spec in specs {
        writer.uleb(spec.entries.len() as u64);
    }
    // Entries within a spec are path-sorted and so are their ids, which makes
    // the deltas small; the run restarts per spec so one long tree cannot
    // widen the next one's first delta.
    let mut references = Column::default();
    for spec in specs {
        references.reset();
        for entry in &spec.entries {
            references.push_required(entry_ids[entry]);
        }
    }
    writer.raw(&references.finish());

    Ok(writer.bytes)
}

fn component_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(_, name)| name).unwrap_or(path)
}

fn index<T: Ord + Clone>(values: &[T]) -> BTreeMap<T, u64> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (value.clone(), index as u64))
        .collect()
}

/// Orders a digest table by first appearance so the column that references it
/// counts upwards in ones instead of jumping around a sorted table.
fn first_use<const N: usize>(
    entries: &[&DirEntry],
    digest: impl Fn(&DirEntry) -> Option<[u8; N]>,
) -> (Vec<[u8; N]>, BTreeMap<[u8; N], u64>) {
    let mut values = Vec::new();
    let mut ids = BTreeMap::new();
    for entry in entries {
        if let Some(value) = digest(entry)
            && let std::collections::btree_map::Entry::Vacant(slot) = ids.entry(value)
        {
            slot.insert(values.len() as u64);
            values.push(value);
        }
    }
    (values, ids)
}
