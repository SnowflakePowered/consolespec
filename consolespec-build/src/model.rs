//! What an archive holds: the definition documents and the partition trees.

/// A definition document, kept as the TOML source it was written as.
///
/// The documents are a rounding error next to the partition trees, and the
/// schema they describe is deserialized by `consolespec`'s build script and
/// nowhere else. Storing the source verbatim keeps that schema in one place
/// instead of splitting it across a serializer here and a deserializer there.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    /// File name the document was read from, such as `NINTENDO_NES.toml`.
    pub name: String,
    pub source: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DirEntryKind {
    Directory,
    File,
    Link,
}

/// One line of an mtree listing, parsed and validated.
///
/// The field order is also the sort order the archive and `consolespec`'s
/// packer rely on: entries sort by path first, so path ids run non-decreasing
/// through the deduplicated entry table.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DirEntry {
    pub path: String,
    pub kind: DirEntryKind,
    pub size: Option<u64>,
    pub link: Option<String>,
    pub md5: Option<[u8; 16]>,
    pub sha1: Option<[u8; 20]>,
    pub sha256: Option<[u8; 32]>,
}

/// The contents of one partition at one firmware revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionSpec {
    /// Path of the source listing relative to `definitions/partitionspec`,
    /// such as `SONY_PSV/vs0/374.mtree`. Machine specs cite this.
    pub reference: String,
    pub entries: Vec<DirEntry>,
}

/// Everything `consolespec`'s build script needs to generate its database.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Definitions {
    pub inputs: Vec<Document>,
    pub machines: Vec<Document>,
    /// Sorted by [`PartitionSpec::reference`].
    pub partition_specs: Vec<PartitionSpec>,
}
