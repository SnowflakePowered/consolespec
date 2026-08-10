//! Compile-time database of console machine and input specifications.
//!
//! Source TOML is parsed and cross-checked by the build script. Runtime access
//! reads immutable generated tables through build-time PHF indexes and performs
//! no parsing or filesystem I/O.
//! Mtree references are retained as strings but their files are intentionally
//! not parsed until a portable mtree implementation is available.

#![forbid(unsafe_code)]

macro_rules! view {
    ($name:ident, $record:ty) => {
        #[derive(Clone, Copy)]
        pub struct $name(pub(crate) &'static $record);
        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .finish_non_exhaustive()
            }
        }
    };
}

pub mod input;
pub mod machine;

pub use input::InputSpec;
pub use machine::MachineSpec;

use input::{
    Alignment, AnalogClass, Axis, BindingKey, ButtonElement, Component, Direction, InputKind,
    Peripheral, RumbleMotor, RumbleSize, Sign,
};
use machine::{AccessoryClass, MachineKind, Region};

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
    elements: Slice,
    clusters: Slice,
}

#[derive(Clone, Copy)]
struct ElementRecord {
    id: StrId,
    binding: BindingKey,
    label: StrId,
    kind: StrId,
}

#[derive(Clone, Copy)]
struct ClusterRecord {
    kind: StrId,
    class: Option<AnalogClass>,
    alignment: Option<Alignment>,
    discriminator: Option<StrId>,
    arity: u8,
    elements: Slice,
}

#[derive(Clone, Copy)]
struct ButtonRecord {
    label: StrId,
    element: ButtonElement,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{ButtonElement, InputKind};

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
        assert!(
            xbox.buttons()
                .any(|button| button.element() == ButtonElement::A)
        );
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

    #[test]
    fn every_input_is_compiled_into_elements_and_clusters() {
        for input in InputSpec::all() {
            let element_ids = input
                .elements()
                .map(|element| element.id())
                .collect::<Vec<_>>();
            assert!(!element_ids.is_empty(), "{}", input.id());
            for cluster in input.clusters() {
                for element in cluster.elements() {
                    assert!(
                        element_ids.contains(&element),
                        "{}/{} references unknown element {element}",
                        input.id(),
                        cluster.kind()
                    );
                }
            }
        }
    }
}
