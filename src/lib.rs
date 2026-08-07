//! Compile-time database of console machine and input specifications.
//!
//! Source TOML is parsed and cross-checked by the build script. Runtime access
//! reads immutable generated tables and performs no parsing or filesystem I/O.
//! Mtree references are retained as strings but their files are intentionally
//! not parsed until a portable mtree implementation is available.

#![forbid(unsafe_code)]

use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct StrId(u32);

#[derive(Clone, Copy)]
struct Slice {
    start: u32,
    len: u32,
}

impl Slice {
    fn get<T>(self, values: &'static [T]) -> &'static [T] {
        let start = self.start as usize;
        &values[start..start + self.len as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum InputKind {
    Controller,
    Handheld,
    Device,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineKind {
    Console,
    Handheld,
    Addon,
    Arcade,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Alignment {
    Left,
    Right,
    Front,
    Rear,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum AnalogClass {
    Stick,
    Slider,
    Rotary,
    Gyroscope,
    Accelerometer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RumbleSize {
    Big,
    Small,
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

#[derive(Clone, Copy)]
struct RegionRecord {
    region: Region,
    name: Option<StrId>,
    name_en: Option<StrId>,
    short_name: Option<StrId>,
    model_numbers: Slice,
    release_date: Option<StrId>,
}

#[derive(Clone, Copy)]
struct InputRecord {
    id: StrId,
    kind: InputKind,
    name: Option<StrId>,
    model_numbers: Slice,
    regions: Slice,
    buttons: Slice,
    directionals: Slice,
    analog: Slice,
    triggers: Slice,
    rumble: Slice,
    pointers: Slice,
    touchscreens: Slice,
    microphones: Slice,
    cameras: Slice,
}

#[derive(Clone, Copy)]
struct ButtonRecord {
    label: StrId,
    element: StrId,
    analog: bool,
}

#[derive(Clone, Copy)]
struct DirectionalRecord {
    label: Option<StrId>,
    directions: u8,
    alignment: Option<Alignment>,
}

#[derive(Clone, Copy)]
struct AnalogRecord {
    label: Option<StrId>,
    axes: u8,
    class: AnalogClass,
    alignment: Option<Alignment>,
    digital: bool,
}

#[derive(Clone, Copy)]
struct TriggerRecord {
    label: Option<StrId>,
    alignment: Alignment,
    digital: bool,
}

#[derive(Clone, Copy)]
struct RumbleRecord {
    label: Option<StrId>,
    size: RumbleSize,
    alignment: Option<Alignment>,
    optional: bool,
}

#[derive(Clone, Copy)]
struct PointerRecord {
    label: Option<StrId>,
    dimensions: u8,
    alignment: Option<Alignment>,
}

#[derive(Clone, Copy)]
struct PlainRecord {
    label: Option<StrId>,
    alignment: Option<Alignment>,
}

#[derive(Clone, Copy)]
struct TouchscreenRecord {
    label: Option<StrId>,
    alignment: Option<Alignment>,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy)]
struct MachineRecord {
    id: StrId,
    name: StrId,
    dependencies: Slice,
    short_name: Option<StrId>,
    model_numbers: Slice,
    licensor: Option<StrId>,
    manufacturer: Option<StrId>,
    kind: MachineKind,
    regions: Slice,
    groups: Slice,
    accessories: Slice,
    storage: Slice,
    bios: Slice,
}

#[derive(Clone, Copy)]
struct GroupRecord {
    name: StrId,
    inputs: Slice,
    ports: u8,
    accessories: bool,
}

#[derive(Clone, Copy)]
struct StorageRecord {
    id: StrId,
    name: StrId,
    raw: bool,
    user: bool,
    partitions: Slice,
}

#[derive(Clone, Copy)]
struct PartitionRecord {
    id: StrId,
    name: StrId,
    specs: Slice,
    user: bool,
}

#[derive(Clone, Copy)]
struct BiosRecord {
    name: StrId,
    md5: Slice,
    sha1: Slice,
    sha256: Slice,
}

include!(concat!(env!("OUT_DIR"), "/database.rs"));

fn text(id: StrId) -> &'static str {
    let index = id.0 as usize;
    let start = STRING_OFFSETS[index] as usize;
    let end = STRING_OFFSETS[index + 1] as usize;
    std::str::from_utf8(&STRING_DATA[start..end]).expect("generated strings are UTF-8")
}

fn strings(slice: Slice) -> impl ExactSizeIterator<Item = &'static str> {
    slice.get(STRING_IDS).iter().copied().map(text)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownSpec {
    kind: &'static str,
    id: String,
}

impl fmt::Display for UnknownSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown {} `{}`", self.kind, self.id)
    }
}

impl std::error::Error for UnknownSpec {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct InputSpec(usize);

impl InputSpec {
    fn record(self) -> &'static InputRecord {
        &INPUTS[self.0]
    }
    pub fn id(self) -> &'static str {
        text(self.record().id)
    }
    pub fn kind(self) -> InputKind {
        self.record().kind
    }
    pub fn name(self) -> Option<&'static str> {
        self.record().name.map(text)
    }
    pub fn model_numbers(self) -> impl ExactSizeIterator<Item = &'static str> {
        strings(self.record().model_numbers)
    }
    pub fn regions(self) -> impl ExactSizeIterator<Item = RegionMetadata> {
        self.record()
            .regions
            .get(REGIONS)
            .iter()
            .map(RegionMetadata)
    }
    pub fn buttons(self) -> impl ExactSizeIterator<Item = Button> {
        self.record().buttons.get(BUTTONS).iter().map(Button)
    }
    pub fn directionals(self) -> impl ExactSizeIterator<Item = Directional> {
        self.record()
            .directionals
            .get(DIRECTIONALS)
            .iter()
            .map(Directional)
    }
    pub fn analog(self) -> impl ExactSizeIterator<Item = Analog> {
        self.record().analog.get(ANALOG).iter().map(Analog)
    }
    pub fn triggers(self) -> impl ExactSizeIterator<Item = Trigger> {
        self.record().triggers.get(TRIGGERS).iter().map(Trigger)
    }
    pub fn rumble(self) -> impl ExactSizeIterator<Item = Rumble> {
        self.record().rumble.get(RUMBLE).iter().map(Rumble)
    }
    pub fn pointers(self) -> impl ExactSizeIterator<Item = Pointer> {
        self.record().pointers.get(POINTERS).iter().map(Pointer)
    }
    pub fn touchscreens(self) -> impl ExactSizeIterator<Item = Touchscreen> {
        self.record()
            .touchscreens
            .get(TOUCHSCREENS)
            .iter()
            .map(Touchscreen)
    }
    pub fn microphones(self) -> impl ExactSizeIterator<Item = Peripheral> {
        self.record().microphones.get(PLAIN).iter().map(Peripheral)
    }
    pub fn cameras(self) -> impl ExactSizeIterator<Item = Peripheral> {
        self.record().cameras.get(PLAIN).iter().map(Peripheral)
    }
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        (0..INPUTS.len()).map(Self)
    }
}

impl TryFrom<&str> for InputSpec {
    type Error = UnknownSpec;
    fn try_from(id: &str) -> Result<Self, Self::Error> {
        INPUT_LOOKUP
            .binary_search_by_key(&id, |(key, _)| text(*key))
            .map(|index| Self(INPUT_LOOKUP[index].1 as usize))
            .map_err(|_| UnknownSpec {
                kind: "inputspec",
                id: id.to_owned(),
            })
    }
}

impl FromStr for InputSpec {
    type Err = UnknownSpec;
    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::try_from(id)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MachineSpec(usize);

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
            .binary_search_by_key(&id, |(key, _)| text(*key))
            .map(|index| Self(MACHINE_LOOKUP[index].1 as usize))
            .map_err(|_| UnknownSpec {
                kind: "machinespec",
                id: id.to_owned(),
            })
    }
}

impl FromStr for MachineSpec {
    type Err = UnknownSpec;
    fn from_str(id: &str) -> Result<Self, Self::Err> {
        Self::try_from(id)
    }
}

macro_rules! view {
    ($name:ident, $record:ty) => {
        #[derive(Clone, Copy)]
        pub struct $name(&'static $record);
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }
    };
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

view!(Button, ButtonRecord);
impl Button {
    pub fn label(self) -> &'static str {
        text(self.0.label)
    }
    pub fn element(self) -> &'static str {
        text(self.0.element)
    }
    pub fn is_analog(self) -> bool {
        self.0.analog
    }
}
view!(Directional, DirectionalRecord);
impl Directional {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn directions(self) -> u8 {
        self.0.directions
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
}
view!(Analog, AnalogRecord);
impl Analog {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn axes(self) -> u8 {
        self.0.axes
    }
    pub fn class(self) -> AnalogClass {
        self.0.class
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
    pub fn is_digital(self) -> bool {
        self.0.digital
    }
}
view!(Trigger, TriggerRecord);
impl Trigger {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn alignment(self) -> Alignment {
        self.0.alignment
    }
    pub fn is_digital(self) -> bool {
        self.0.digital
    }
}
view!(Rumble, RumbleRecord);
impl Rumble {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn size(self) -> RumbleSize {
        self.0.size
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
    pub fn is_optional(self) -> bool {
        self.0.optional
    }
}
view!(Pointer, PointerRecord);
impl Pointer {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn dimensions(self) -> u8 {
        self.0.dimensions
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
}
view!(Peripheral, PlainRecord);
impl Peripheral {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
}
view!(Touchscreen, TouchscreenRecord);
impl Touchscreen {
    pub fn label(self) -> Option<&'static str> {
        self.0.label.map(text)
    }
    pub fn alignment(self) -> Option<Alignment> {
        self.0.alignment
    }
    pub fn width(self) -> u32 {
        self.0.width
    }
    pub fn height(self) -> u32 {
        self.0.height
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_generated_specs() {
        let wiiu = MachineSpec::try_from("NINTENDO_WIIU").unwrap();
        assert_eq!(wiiu.name(), "WiiU");
        assert!(
            wiiu.input_group("gamepad")
                .unwrap()
                .accepts("WIIU_DRC_CONTROLLER")
        );
        let xbox = InputSpec::try_from("XBOX_CONTROLLER").unwrap();
        assert_eq!(xbox.kind(), InputKind::Controller);
        assert!(xbox.buttons().any(|button| button.element() == "a"));
    }

    #[test]
    fn every_machine_input_resolves() {
        for machine in MachineSpec::all() {
            for group in machine.input_groups() {
                for input in group.inputs() {
                    assert!(
                        InputSpec::try_from(input).is_ok(),
                        "{}/{}: {input}",
                        machine.id(),
                        group.name()
                    );
                }
            }
        }
    }
}
