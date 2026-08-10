//! Console machine specifications and their storage, firmware, and input-port metadata.

use crate::{
    ACCESSORIES, BIOS, BiosRecord, GROUPS, GroupRecord, MACHINE_LOOKUP, MACHINES, MachineRecord,
    PARTITIONS, PartitionRecord, REGIONS, RegionRecord, STORAGE, StorageRecord, strings, text,
};
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

    pub fn mtree_references(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.0.specs)
    }

    pub fn is_user_data(self) -> bool {
        self.0.user
    }
}

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
