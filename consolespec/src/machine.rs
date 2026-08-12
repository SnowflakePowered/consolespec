//! Console machine specifications and their storage, firmware, and input-port metadata.

use crate::{
    ACCESSORIES, BIOS, BiosRecord, GROUPS, GroupRecord, MACHINE_LOOKUP, MACHINES, MachineRecord,
    PARTITIONS, PartitionRecord, REGIONS, RegionRecord, STORAGE, StorageRecord, strings, text,
};
#[cfg(feature = "partition-specs")]
use crate::{
    DIR_ENTRY_RECORD_COUNT, DIR_ENTRY_RECORD_SIZE, DIR_ENTRY_RECORDS_OFFSET,
    DIR_ENTRY_SIZES_OFFSET, PARTITION_SPEC_DATA, PARTITION_SPEC_IDS, PARTITION_SPECS,
    PartitionSpecRecord, StrId,
};
#[cfg(feature = "partition-spec-digests")]
use crate::{MD5_DIGESTS_OFFSET, SHA1_DIGESTS_OFFSET, SHA256_DIGESTS_OFFSET};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineKind {
    Console,
    Handheld,
    Addon,
    Arcade,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Region {
    NorthAmerica,
    Europe,
    Japan,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AccessoryClass {
    DisneyInfinity,
    Skylander,
    LegoDimensions,
    Guitar,
    Piano,
    Drums,
    Keyboard,
    Mouse,
    Camera,
    Microphone,
    Storage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownSpec {
    id: String,
}

impl fmt::Display for UnknownSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown machinespec `{}`", self.id)
    }
}

impl std::error::Error for UnknownSpec {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MachineSpec(pub(crate) usize);

impl MachineSpec {
    fn record(self) -> &'static MachineRecord {
        &MACHINES[self.0]
    }

    pub fn id(self) -> &'static str {
        text(self.record().id)
    }

    pub fn name(self) -> &'static str {
        text(self.record().name)
    }

    pub fn kind(self) -> MachineKind {
        self.record().kind
    }

    pub fn short_name(self) -> Option<&'static str> {
        self.record().short_name.map(text)
    }

    pub fn model_numbers(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.record().model_numbers)
    }

    pub fn licensor(self) -> Option<&'static str> {
        self.record().licensor.map(text)
    }

    pub fn manufacturer(self) -> Option<&'static str> {
        self.record().manufacturer.map(text)
    }

    pub fn dependencies(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.record().dependencies)
    }

    pub fn regions(self) -> impl ExactSizeIterator<Item = RegionMetadata> {
        self.record()
            .regions
            .get(REGIONS)
            .iter()
            .map(RegionMetadata)
    }

    pub fn input_groups(self) -> impl ExactSizeIterator<Item = InputGroup> {
        self.record().groups.get(GROUPS).iter().map(InputGroup)
    }

    pub fn input_group(self, name: &str) -> Option<InputGroup> {
        self.input_groups().find(|group| group.name() == name)
    }

    pub fn group_for(self, input: &str) -> Result<InputGroup, String> {
        let mut groups = self
            .input_groups()
            .filter(|group| group.ports() > 0 && group.accepts(input));
        let first = groups
            .next()
            .ok_or_else(|| format!("machine {} has no port that accepts {input}", self.id()))?;
        match groups.next() {
            None => Ok(first),
            Some(second) => Err(format!(
                "machine {} accepts {input} in both `{}` and `{}`; name the port",
                self.id(),
                first.name(),
                second.name()
            )),
        }
    }

    pub fn accessories(self) -> &'static [AccessoryClass] {
        self.record().accessories.get(ACCESSORIES)
    }

    pub fn storage_devices(self) -> impl ExactSizeIterator<Item = StorageDevice> {
        self.record().storage.get(STORAGE).iter().map(StorageDevice)
    }

    pub fn bios(self) -> impl ExactSizeIterator<Item = Bios> {
        self.record().bios.get(BIOS).iter().map(Bios)
    }

    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        (0..MACHINES.len()).map(Self)
    }
}

impl TryFrom<&str> for MachineSpec {
    type Error = UnknownSpec;

    fn try_from(id: &str) -> Result<Self, Self::Error> {
        MACHINE_LOOKUP
            .get(id)
            .map(|index| Self(*index as usize))
            .ok_or_else(|| UnknownSpec { id: id.to_owned() })
    }
}

impl FromStr for MachineSpec {
    type Err = UnknownSpec;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::try_from(id)
    }
}

view!(RegionMetadata, RegionRecord);
impl RegionMetadata {
    pub fn region(self) -> Region {
        self.0.region
    }

    pub fn name(self) -> Option<&'static str> {
        self.0.name.map(text)
    }

    pub fn english_name(self) -> Option<&'static str> {
        self.0.name_en.map(text)
    }

    pub fn short_name(self) -> Option<&'static str> {
        self.0.short_name.map(text)
    }

    pub fn model_numbers(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.model_numbers)
    }

    pub fn release_date(self) -> Option<&'static str> {
        self.0.release_date.map(text)
    }
}

view!(InputGroup, GroupRecord);
impl InputGroup {
    pub fn name(self) -> &'static str {
        text(self.0.name)
    }

    pub fn inputs(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.inputs)
    }

    pub fn ports(self) -> u8 {
        self.0.ports
    }

    pub fn is_accessory_group(self) -> bool {
        self.0.accessories
    }

    pub fn accepts(self, input: &str) -> bool {
        self.inputs().any(|id| id == input)
    }
}

view!(StorageDevice, StorageRecord);
impl StorageDevice {
    pub fn id(self) -> &'static str {
        text(self.0.id)
    }

    pub fn name(self) -> &'static str {
        text(self.0.name)
    }

    pub fn is_raw(self) -> bool {
        self.0.raw
    }

    pub fn is_user_storage(self) -> bool {
        self.0.user
    }

    pub fn partitions(self) -> impl ExactSizeIterator<Item = Partition> {
        self.0.partitions.get(PARTITIONS).iter().map(Partition)
    }
}

view!(Partition, PartitionRecord);
impl Partition {
    pub fn id(self) -> &'static str {
        text(self.0.id)
    }

    pub fn name(self) -> &'static str {
        text(self.0.name)
    }

    #[cfg(feature = "partition-specs")]
    /// Returns the compiled directory specifications for this partition.
    pub fn specs(self) -> impl ExactSizeIterator<Item = PartitionSpec> {
        self.0
            .specs
            .get(PARTITION_SPEC_IDS)
            .iter()
            .map(|id| PartitionSpec(&PARTITION_SPECS[*id as usize]))
    }

    pub fn is_user_data(self) -> bool {
        self.0.user
    }
}

#[cfg(feature = "partition-specs")]
view!(PartitionSpec, PartitionSpecRecord);
#[cfg(feature = "partition-specs")]
impl PartitionSpec {
    /// Returns the source path of this specification relative to `definitions/partitionspec`.
    pub fn reference(self) -> &'static str {
        text(self.0.reference)
    }

    /// Returns the directory entries described by this specification.
    pub fn entries(self) -> impl ExactSizeIterator<Item = DirEntry> {
        DirEntries {
            cursor: self.0.entries_start as usize,
            remaining: self.0.entries_len as usize,
            previous: 0,
            first: true,
        }
    }
}

#[cfg(feature = "partition-specs")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// The filesystem object represented by a [`DirEntry`].
pub enum DirEntryKind {
    /// A directory.
    Directory,
    /// A regular file.
    File,
    /// A symbolic link.
    Link,
}

#[cfg(feature = "partition-specs")]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
/// An immutable directory entry from a compiled [`PartitionSpec`].
pub struct DirEntry(u32);

#[cfg(feature = "partition-specs")]
impl fmt::Debug for DirEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DirEntry")
            .field("path", &self.path())
            .field("kind", &self.kind())
            .field("size", &self.size())
            .field("link", &self.link())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "partition-specs")]
impl DirEntry {
    fn record_offset(self) -> usize {
        let id = self.0 as usize;
        assert!(
            id < DIR_ENTRY_RECORD_COUNT,
            "invalid generated directory entry ID"
        );
        DIR_ENTRY_RECORDS_OFFSET + id * DIR_ENTRY_RECORD_SIZE
    }

    fn packed_id(self, field_offset: usize) -> Option<u32> {
        let value = read_u24(self.record_offset() + field_offset);
        (value != PACKED_NONE).then_some(value)
    }

    /// Returns the mtree path relative to the partition root.
    pub fn path(self) -> &'static str {
        text(StrId(
            self.packed_id(0)
                .expect("generated directory entry has no path"),
        ))
    }

    /// Returns the filesystem object kind.
    pub fn kind(self) -> DirEntryKind {
        match PARTITION_SPEC_DATA[self.record_offset() + DIR_ENTRY_RECORD_SIZE - 1] {
            0 => DirEntryKind::Directory,
            1 => DirEntryKind::File,
            2 => DirEntryKind::Link,
            _ => unreachable!("invalid generated directory entry kind"),
        }
    }

    /// Returns the file size when one was recorded.
    pub fn size(self) -> Option<u64> {
        let id = self.packed_id(3)? as usize;
        let offset = DIR_ENTRY_SIZES_OFFSET + id * std::mem::size_of::<u64>();
        Some(u64::from_le_bytes(
            PARTITION_SPEC_DATA[offset..offset + std::mem::size_of::<u64>()]
                .try_into()
                .expect("generated file size is truncated"),
        ))
    }

    /// Returns the symbolic-link target when this is a link.
    pub fn link(self) -> Option<&'static str> {
        self.packed_id(6).map(|id| text(StrId(id)))
    }

    #[cfg(feature = "partition-spec-digests")]
    /// Returns the recorded MD5 digest.
    pub fn md5(self) -> Option<&'static [u8; 16]> {
        self.digest::<16>(9, MD5_DIGESTS_OFFSET)
    }

    #[cfg(feature = "partition-spec-digests")]
    /// Returns the recorded SHA-1 digest.
    pub fn sha1(self) -> Option<&'static [u8; 20]> {
        self.digest::<20>(12, SHA1_DIGESTS_OFFSET)
    }

    #[cfg(feature = "partition-spec-digests")]
    /// Returns the recorded SHA-256 digest.
    pub fn sha256(self) -> Option<&'static [u8; 32]> {
        self.digest::<32>(15, SHA256_DIGESTS_OFFSET)
    }

    #[cfg(feature = "partition-spec-digests")]
    fn digest<const N: usize>(
        self,
        field_offset: usize,
        table_offset: usize,
    ) -> Option<&'static [u8; N]> {
        let id = self.packed_id(field_offset)? as usize;
        let offset = table_offset + id * N;
        Some(
            PARTITION_SPEC_DATA[offset..offset + N]
                .try_into()
                .expect("generated digest is truncated"),
        )
    }
}

#[cfg(feature = "partition-specs")]
const PACKED_NONE: u32 = 0x00ff_ffff;

#[cfg(feature = "partition-specs")]
fn read_u24(offset: usize) -> u32 {
    let bytes = &PARTITION_SPEC_DATA[offset..offset + 3];
    u32::from(bytes[0]) | (u32::from(bytes[1]) << 8) | (u32::from(bytes[2]) << 16)
}

#[cfg(feature = "partition-specs")]
fn read_uleb(cursor: &mut usize) -> u64 {
    let mut value = 0u64;
    let mut shift = 0;
    loop {
        let byte = PARTITION_SPEC_DATA[*cursor];
        *cursor += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
        assert!(shift < 64, "invalid generated LEB128 value");
    }
}

#[cfg(feature = "partition-specs")]
struct DirEntries {
    cursor: usize,
    remaining: usize,
    previous: i64,
    first: bool,
}

#[cfg(feature = "partition-specs")]
impl Iterator for DirEntries {
    type Item = DirEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let encoded = read_uleb(&mut self.cursor);
        let id = if self.first {
            self.first = false;
            i64::try_from(encoded).expect("generated directory entry ID exceeds i64")
        } else {
            let delta = ((encoded >> 1) as i64) ^ -((encoded & 1) as i64);
            self.previous + delta
        };
        self.previous = id;
        self.remaining -= 1;
        Some(DirEntry(u32::try_from(id).expect(
            "generated directory entry ID is negative or exceeds u32",
        )))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

#[cfg(feature = "partition-specs")]
impl ExactSizeIterator for DirEntries {}

view!(Bios, BiosRecord);
impl Bios {
    pub fn name(self) -> &'static str {
        text(self.0.name)
    }

    pub fn md5(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.md5)
    }

    pub fn sha1(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.sha1)
    }

    pub fn sha256(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.sha256)
    }
}
