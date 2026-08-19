//! Expanding archive sections back into the model.

use super::codec::{ColumnReader, Reader};
use crate::{DirEntry, DirEntryKind, Document, Error, PartitionSpec, Result};

pub(super) fn documents(payload: &[u8]) -> Result<(Vec<Document>, Vec<Document>)> {
    let mut reader = Reader::new(payload);
    let inputs = reader.count()?;
    let machines = reader.count()?;
    let total = inputs
        .checked_add(machines)
        .ok_or_else(|| Error::new("archive holds more documents than this target can address"))?;
    let names = reader.strings(total)?;
    let mut lengths = Vec::with_capacity(total);
    for _ in 0..total {
        lengths.push(reader.usize()?);
    }

    let mut documents = Vec::with_capacity(total);
    for (name, len) in names.into_iter().zip(lengths) {
        let source = String::from_utf8(reader.bytes(len)?.to_vec())
            .map_err(|_| Error::new(format!("document `{name}` is not valid UTF-8")))?;
        documents.push(Document { name, source });
    }
    let machines = documents.split_off(inputs);
    Ok((documents, machines))
}

pub(super) fn partition_index(payload: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::new(payload);
    let count = reader.count()?;
    reader.strings(count)
}

pub(super) fn partition_data(
    payload: &[u8],
    references: Vec<String>,
) -> Result<Vec<PartitionSpec>> {
    let mut reader = Reader::new(payload);

    let component_count = reader.count()?;
    let components = reader.strings(component_count)?;
    let paths = paths(&mut reader, &components)?;

    let link_count = reader.count()?;
    let link_targets = reader.strings(link_count)?;

    let size_count = reader.count()?;
    let mut sizes = Vec::with_capacity(size_count);
    let mut previous = 0u64;
    for _ in 0..size_count {
        previous = previous
            .checked_add(reader.uleb()?)
            .ok_or_else(|| Error::new("archive size table overflows"))?;
        sizes.push(previous);
    }

    let count = reader.count()?;
    let md5 = reader.digests::<16>(count)?;
    let count = reader.count()?;
    let sha1 = reader.digests::<20>(count)?;
    let count = reader.count()?;
    let sha256 = reader.digests::<32>(count)?;

    let entry_count = reader.count()?;
    let kinds = reader.bytes(entry_count)?.to_vec();
    let mut path_ids = Vec::with_capacity(entry_count);
    {
        let mut column = ColumnReader::new(&mut reader);
        for _ in 0..entry_count {
            path_ids.push(column.next_required()?);
        }
    }
    let mut optional = Vec::with_capacity(5);
    for _ in 0..5 {
        let mut column = ColumnReader::new(&mut reader);
        let mut values = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            values.push(column.next()?);
        }
        optional.push(values);
    }
    let [size_ids, link_ids, md5_ids, sha1_ids, sha256_ids] =
        <[Vec<Option<u64>>; 5]>::try_from(optional).expect("five optional columns");

    let mut entries = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        entries.push(DirEntry {
            path: pick(&paths, Some(path_ids[index]), "path")?
                .cloned()
                .ok_or_else(|| Error::new("archive entry has no path"))?,
            kind: match kinds[index] {
                0 => DirEntryKind::Directory,
                1 => DirEntryKind::File,
                2 => DirEntryKind::Link,
                other => return Err(Error::new(format!("archive entry has kind {other}"))),
            },
            size: pick(&sizes, size_ids[index], "size")?.copied(),
            link: pick(&link_targets, link_ids[index], "link target")?.cloned(),
            md5: pick(&md5, md5_ids[index], "MD5 digest")?.copied(),
            sha1: pick(&sha1, sha1_ids[index], "SHA-1 digest")?.copied(),
            sha256: pick(&sha256, sha256_ids[index], "SHA-256 digest")?.copied(),
        });
    }

    let spec_count = reader.count()?;
    if spec_count != references.len() {
        return Err(Error::new(format!(
            "archive indexes {} partition specs but stores {spec_count}",
            references.len()
        )));
    }
    let mut lengths = Vec::with_capacity(spec_count);
    for _ in 0..spec_count {
        lengths.push(reader.usize()?);
    }
    let mut column = ColumnReader::new(&mut reader);
    let mut specs = Vec::with_capacity(spec_count);
    for (reference, len) in references.into_iter().zip(lengths) {
        column.reset();
        let mut listing = Vec::with_capacity(len);
        for _ in 0..len {
            let entry = pick(&entries, Some(column.next_required()?), "directory entry")
                .map_err(|_| Error::new(format!("`{reference}` names an unknown entry")))?;
            listing.push(entry.expect("present ids resolve").clone());
        }
        specs.push(PartitionSpec {
            reference,
            entries: listing,
        });
    }
    Ok(specs)
}

/// Rebuilds the path trie, whose parent ids always point at an earlier path.
fn paths(reader: &mut Reader<'_>, components: &[String]) -> Result<Vec<String>> {
    let count = reader.count()?;
    let children = count
        .checked_sub(1)
        .ok_or_else(|| Error::new("archive path table has no root"))?;
    let mut parents = Vec::with_capacity(children);
    {
        let mut column = ColumnReader::new(reader);
        for _ in 0..children {
            parents.push(column.next_required()?);
        }
    }
    let mut column = ColumnReader::new(reader);
    let mut paths = Vec::with_capacity(count);
    paths.push(".".to_owned());
    for parent in parents {
        let component = pick(components, Some(column.next_required()?), "component")?
            .expect("present ids resolve");
        let path = {
            let parent = pick(&paths, Some(parent), "path")?.expect("present ids resolve");
            format!("{parent}/{component}")
        };
        paths.push(path);
    }
    Ok(paths)
}

/// Resolves an optional column slot against the table it indexes.
///
/// Ids that point past their table mean the archive is corrupt rather than
/// merely stale, so this reports instead of panicking on the slice.
fn pick<'a, T>(table: &'a [T], id: Option<u64>, name: &str) -> Result<Option<&'a T>> {
    let Some(id) = id else {
        return Ok(None);
    };
    usize::try_from(id)
        .ok()
        .and_then(|id| table.get(id))
        .map(Some)
        .ok_or_else(|| Error::new(format!("archive entry names an unknown {name}")))
}
