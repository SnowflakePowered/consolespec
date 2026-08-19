//! The archive container: a header, a section table, and one independently
//! compressed zstd frame per section.
//!
//! Sections are separate frames rather than one so that a consumer can decode
//! only what it needs. `consolespec` built without the `partition-specs`
//! feature still has to check that every mtree reference in a machine spec
//! resolves, but it never touches the several megabytes of entry tables that
//! back them.

mod codec;
mod decode;
#[cfg(feature = "compile")]
mod encode;

use std::{fs, io::Read, path::Path};

use crate::{Definitions, Document, Error, PartitionSpec, Result};

#[cfg(feature = "compile")]
pub use encode::{DEFAULT_LEVEL, write};

const MAGIC: [u8; 4] = *b"CSAR";
const VERSION: u16 = 1;
const HEADER_LEN: usize = 8;
const SECTION_ENTRY_LEN: usize = 28;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SectionKind {
    /// Input and machine spec documents.
    Documents,
    /// Every partition spec reference, so references can be validated without
    /// decoding the trees themselves.
    PartitionIndex,
    /// The interned partition trees, in [`SectionKind::PartitionIndex`] order.
    PartitionData,
}

impl SectionKind {
    #[cfg(feature = "compile")]
    const fn code(self) -> u32 {
        match self {
            Self::Documents => 1,
            Self::PartitionIndex => 2,
            Self::PartitionData => 3,
        }
    }

    const fn from_code(code: u32) -> Option<Self> {
        match code {
            1 => Some(Self::Documents),
            2 => Some(Self::PartitionIndex),
            3 => Some(Self::PartitionData),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Documents => "documents",
            Self::PartitionIndex => "partition index",
            Self::PartitionData => "partition data",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Section {
    pub kind: SectionKind,
    pub compressed_len: u64,
    pub uncompressed_len: u64,
    offset: u64,
}

/// A definition archive, held in memory with its sections still compressed.
pub struct Archive {
    bytes: Vec<u8>,
    sections: Vec<Section>,
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self> {
        let bytes =
            fs::read(path).map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
        Self::from_bytes(bytes).map_err(|error| Error::new(format!("{}: {error}", path.display())))
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < HEADER_LEN || bytes[..4] != MAGIC {
            return Err(Error::new("not a consolespec definition archive"));
        }
        let version = u16::from_le_bytes([bytes[4], bytes[5]]);
        if version != VERSION {
            return Err(Error::new(format!(
                "archive is version {version}, but this build understands version {VERSION}"
            )));
        }
        let count = usize::from(u16::from_le_bytes([bytes[6], bytes[7]]));
        let table_end = HEADER_LEN + count * SECTION_ENTRY_LEN;
        if bytes.len() < table_end {
            return Err(Error::new("archive section table is truncated"));
        }

        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let entry = &bytes[HEADER_LEN + index * SECTION_ENTRY_LEN..][..SECTION_ENTRY_LEN];
            let number = |range: std::ops::Range<usize>| {
                u64::from_le_bytes(entry[range].try_into().expect("eight bytes"))
            };
            let code = u32::from_le_bytes(entry[0..4].try_into().expect("four bytes"));
            let offset = number(4..12);
            let compressed_len = number(12..20);
            let uncompressed_len = number(20..28);
            offset
                .checked_add(compressed_len)
                .filter(|end| *end <= bytes.len() as u64)
                .ok_or_else(|| Error::new("archive section runs past the end of the file"))?;
            // Unknown section kinds are skipped rather than rejected, so a
            // newer archive that adds one still reads on this version.
            if let Some(kind) = SectionKind::from_code(code) {
                sections.push(Section {
                    kind,
                    compressed_len,
                    uncompressed_len,
                    offset,
                });
            }
        }
        Ok(Self { bytes, sections })
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// Total size on disk, which is what the published crate pays for.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn section(&self, kind: SectionKind) -> Result<Vec<u8>> {
        let section = self
            .sections
            .iter()
            .find(|section| section.kind == kind)
            .ok_or_else(|| Error::new(format!("archive has no {} section", kind.name())))?;
        let start = usize::try_from(section.offset).expect("offset fits in memory");
        let len = usize::try_from(section.compressed_len).expect("length fits in memory");
        let capacity = usize::try_from(section.uncompressed_len)
            .map_err(|_| Error::new("archive section is larger than this target can address"))?;

        let mut decoder = ruzstd::decoding::StreamingDecoder::new(&self.bytes[start..start + len])
            .map_err(|error| Error::new(format!("{} section: {error}", kind.name())))?;
        let mut payload = Vec::with_capacity(capacity);
        decoder
            .read_to_end(&mut payload)
            .map_err(|error| Error::new(format!("{} section: {error}", kind.name())))?;
        if payload.len() as u64 != section.uncompressed_len {
            return Err(Error::new(format!(
                "{} section decoded to {} bytes, but the header claims {}",
                kind.name(),
                payload.len(),
                section.uncompressed_len
            )));
        }
        Ok(payload)
    }

    /// The input and machine spec documents, as `(inputs, machines)`.
    pub fn documents(&self) -> Result<(Vec<Document>, Vec<Document>)> {
        decode::documents(&self.section(SectionKind::Documents)?)
    }

    /// Every partition spec reference the archive carries, sorted.
    ///
    /// Cheap: this decodes a few kilobytes and leaves the trees alone.
    pub fn partition_references(&self) -> Result<Vec<String>> {
        decode::partition_index(&self.section(SectionKind::PartitionIndex)?)
    }

    /// The partition trees, sorted by reference.
    pub fn partition_specs(&self) -> Result<Vec<PartitionSpec>> {
        let references = self.partition_references()?;
        decode::partition_data(&self.section(SectionKind::PartitionData)?, references)
    }

    /// Everything at once, which only the archive tooling wants.
    pub fn definitions(&self) -> Result<Definitions> {
        let (inputs, machines) = self.documents()?;
        Ok(Definitions {
            inputs,
            machines,
            partition_specs: self.partition_specs()?,
        })
    }
}

#[cfg(all(test, feature = "compile"))]
mod tests {
    use super::*;
    use crate::{DirEntry, DirEntryKind};

    fn sample() -> Definitions {
        Definitions {
            inputs: vec![Document {
                name: "NES_CONTROLLER.toml".to_owned(),
                source: "[input]\nid = \"NES_CONTROLLER\"\n".to_owned(),
            }],
            machines: vec![Document {
                name: "NINTENDO_NES.toml".to_owned(),
                source: "[machine]\nid = \"NINTENDO_NES\"\n".to_owned(),
            }],
            partition_specs: vec![
                PartitionSpec {
                    reference: "SONY_PSV/vs0/360.mtree".to_owned(),
                    entries: vec![
                        DirEntry {
                            path: ".".to_owned(),
                            kind: DirEntryKind::Directory,
                            size: None,
                            link: None,
                            md5: None,
                            sha1: None,
                            sha256: None,
                        },
                        DirEntry {
                            path: "./app".to_owned(),
                            kind: DirEntryKind::Directory,
                            size: None,
                            link: None,
                            md5: None,
                            sha1: None,
                            sha256: None,
                        },
                        DirEntry {
                            path: "./app/eboot.bin".to_owned(),
                            kind: DirEntryKind::File,
                            size: Some(1_074_203),
                            link: None,
                            md5: Some([1; 16]),
                            sha1: Some([2; 20]),
                            sha256: Some([3; 32]),
                        },
                    ],
                },
                PartitionSpec {
                    reference: "SONY_PSV/vs0/374.mtree".to_owned(),
                    entries: vec![
                        DirEntry {
                            path: ".".to_owned(),
                            kind: DirEntryKind::Directory,
                            size: None,
                            link: None,
                            md5: None,
                            sha1: None,
                            sha256: None,
                        },
                        DirEntry {
                            path: "./app/eboot.bin".to_owned(),
                            kind: DirEntryKind::File,
                            size: Some(1_074_203),
                            link: None,
                            md5: Some([1; 16]),
                            sha1: Some([2; 20]),
                            sha256: Some([3; 32]),
                        },
                        DirEntry {
                            path: "./link".to_owned(),
                            kind: DirEntryKind::Link,
                            size: None,
                            link: Some("./app/eboot.bin".to_owned()),
                            md5: None,
                            sha1: None,
                            sha256: None,
                        },
                    ],
                },
            ],
        }
    }

    #[test]
    fn definitions_survive_a_round_trip() {
        let definitions = sample();
        let archive = Archive::from_bytes(write(&definitions, DEFAULT_LEVEL).unwrap()).unwrap();
        assert_eq!(archive.definitions().unwrap(), definitions);
    }

    #[test]
    fn references_decode_without_touching_the_trees() {
        let archive = Archive::from_bytes(write(&sample(), DEFAULT_LEVEL).unwrap()).unwrap();
        assert_eq!(
            archive.partition_references().unwrap(),
            ["SONY_PSV/vs0/360.mtree", "SONY_PSV/vs0/374.mtree"]
        );
    }

    #[test]
    fn a_truncated_archive_is_rejected_rather_than_panicking() {
        let bytes = write(&sample(), DEFAULT_LEVEL).unwrap();
        assert!(Archive::from_bytes(bytes[..HEADER_LEN + 4].to_vec()).is_err());
        assert!(Archive::from_bytes(b"not an archive".to_vec()).is_err());
    }
}
