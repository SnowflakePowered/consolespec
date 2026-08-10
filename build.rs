use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fmt::{self, Write as _},
    fs,
    path::{Path, PathBuf},
};
use string_interner::{DefaultStringInterner, Symbol};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputDocument {
    input: InputTable,
    #[serde(default)]
    meta: InputMeta,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputTable {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    button: Vec<Button>,
    #[serde(default)]
    directional: Vec<Directional>,
    #[serde(default)]
    analog: Vec<Analog>,
    #[serde(default)]
    trigger: Vec<Trigger>,
    #[serde(default)]
    rumble: Vec<Rumble>,
    #[serde(default)]
    pointer: Vec<Pointer>,
    #[serde(default)]
    touchscreen: Vec<Touchscreen>,
    #[serde(default)]
    microphone: Vec<Plain>,
    #[serde(default)]
    camera: Vec<Plain>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputMeta {
    name: Option<String>,
    #[serde(default, rename = "model-number")]
    model_numbers: Vec<String>,
    na: Option<RegionMeta>,
    eu: Option<RegionMeta>,
    jp: Option<RegionMeta>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionMeta {
    name: Option<String>,
    #[serde(rename = "name-en")]
    name_en: Option<String>,
    #[serde(rename = "short-name")]
    short_name: Option<String>,
    #[serde(default, rename = "model-number")]
    model_numbers: Vec<String>,
    #[serde(rename = "release-date")]
    release_date: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Button {
    label: String,
    element: String,
    #[serde(default)]
    analog: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Directional {
    label: Option<String>,
    directions: u8,
    alignment: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Analog {
    label: Option<String>,
    axes: u8,
    class: Option<String>,
    alignment: Option<String>,
    #[serde(default)]
    digital: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trigger {
    label: Option<String>,
    alignment: String,
    #[serde(default)]
    digital: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rumble {
    label: Option<String>,
    size: String,
    alignment: Option<String>,
    #[serde(default)]
    optional: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer {
    label: Option<String>,
    dimensions: u8,
    alignment: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plain {
    label: Option<String>,
    alignment: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Touchscreen {
    label: Option<String>,
    alignment: Option<String>,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineDocument {
    machine: MachineIdentity,
    meta: MachineMeta,
    input: Option<MachineInput>,
    accessories: Option<Accessories>,
    storage: Option<StorageTable>,
    #[serde(default)]
    bios: Vec<Bios>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineIdentity {
    id: String,
    name: String,
    #[serde(default, rename = "depends-on")]
    dependencies: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineMeta {
    #[serde(rename = "short-name")]
    short_name: Option<String>,
    #[serde(default, rename = "model-number")]
    model_numbers: Vec<String>,
    licensor: Option<String>,
    manufacturer: Option<String>,
    #[serde(rename = "type")]
    kind: String,
    na: Option<RegionMeta>,
    eu: Option<RegionMeta>,
    jp: Option<RegionMeta>,
}

#[derive(Deserialize)]
struct MachineInput {
    groups: Vec<String>,
    #[serde(flatten)]
    tables: BTreeMap<String, Group>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    #[serde(default)]
    inputs: Vec<String>,
    ports: Option<u8>,
    #[serde(default)]
    accessories: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Accessories {
    class: Vec<String>,
}

#[derive(Deserialize)]
struct StorageTable {
    devices: Vec<String>,
    #[serde(flatten)]
    tables: BTreeMap<String, Storage>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Storage {
    name: String,
    #[serde(default)]
    raw: bool,
    #[serde(default)]
    user: bool,
    #[serde(default, rename = "partition")]
    partitions: Vec<Partition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Partition {
    id: String,
    name: String,
    #[serde(default)]
    spec: Vec<String>,
    #[serde(default)]
    user: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bios {
    name: String,
    #[serde(default)]
    md5: Vec<String>,
    #[serde(default)]
    sha1: Vec<String>,
    #[serde(default)]
    sha256: Vec<String>,
}

#[derive(Clone, Copy)]
struct Span {
    start: u32,
    len: u32,
}

struct ExpandedElement {
    id: String,
    binding: String,
    label: String,
    kind: &'static str,
}

struct ExpandedCluster {
    kind: &'static str,
    class: Option<&'static str>,
    alignment: Option<String>,
    discriminator: Option<String>,
    arity: u8,
    elements: Vec<String>,
}

impl Span {
    fn code(self) -> String {
        format!("Slice {{ start: {}, len: {} }}", self.start, self.len)
    }
}

#[derive(Default)]
struct Generator {
    interner: DefaultStringInterner,
    string_ids: Vec<u32>,
    regions: Vec<String>,
    buttons: Vec<String>,
    directionals: Vec<String>,
    analog: Vec<String>,
    triggers: Vec<String>,
    rumble: Vec<String>,
    pointers: Vec<String>,
    plain: Vec<String>,
    touchscreens: Vec<String>,
    elements: Vec<String>,
    clusters: Vec<String>,
    groups: Vec<String>,
    accessories: Vec<String>,
    storage: Vec<String>,
    partitions: Vec<String>,
    bios: Vec<String>,
    inputs: Vec<(String, String)>,
    machines: Vec<(String, String)>,
}

impl Generator {
    fn id(&mut self, value: &str) -> u32 {
        self.interner.get_or_intern(value).to_usize() as u32
    }

    fn id_code(&mut self, value: &str) -> String {
        format!("StrId({})", self.id(value))
    }

    fn option_id(&mut self, value: Option<&str>) -> String {
        value
            .map(|value| format!("Some({})", self.id_code(value)))
            .unwrap_or_else(|| "None".to_owned())
    }

    fn string_slice<'a>(&mut self, values: impl IntoIterator<Item = &'a str>) -> Span {
        let start = self.string_ids.len() as u32;
        for value in values {
            let id = self.id(value);
            self.string_ids.push(id);
        }
        Span {
            start,
            len: self.string_ids.len() as u32 - start,
        }
    }

    fn push_codes(target: &mut Vec<String>, values: impl IntoIterator<Item = String>) -> Span {
        let start = target.len() as u32;
        target.extend(values);
        Span {
            start,
            len: target.len() as u32 - start,
        }
    }

    fn region(&mut self, region: &str, value: &RegionMeta) -> String {
        let models = self.string_slice(value.model_numbers.iter().map(String::as_str));
        format!(
            "RegionRecord {{ region: Region::{region}, name: {}, name_en: {}, short_name: {}, model_numbers: {}, release_date: {} }}",
            self.option_id(value.name.as_deref()),
            self.option_id(value.name_en.as_deref()),
            self.option_id(value.short_name.as_deref()),
            models.code(),
            self.option_id(value.release_date.as_deref())
        )
    }

    fn regions(
        &mut self,
        na: Option<&RegionMeta>,
        eu: Option<&RegionMeta>,
        jp: Option<&RegionMeta>,
    ) -> Span {
        let mut codes = Vec::new();
        if let Some(value) = na {
            codes.push(self.region("NorthAmerica", value));
        }
        if let Some(value) = eu {
            codes.push(self.region("Europe", value));
        }
        if let Some(value) = jp {
            codes.push(self.region("Japan", value));
        }
        Self::push_codes(&mut self.regions, codes)
    }

    fn input(&mut self, document: &InputDocument) -> Result<(), String> {
        let input = &document.input;
        let kind = input_kind(&input.kind)?;
        if input.button.is_empty()
            && input.directional.is_empty()
            && input.analog.is_empty()
            && input.trigger.is_empty()
            && input.rumble.is_empty()
            && input.pointer.is_empty()
            && input.touchscreen.is_empty()
            && input.microphone.is_empty()
            && input.camera.is_empty()
        {
            return Err(format!("{}: inputspec expands to no elements", input.id));
        }
        let model_numbers =
            self.string_slice(document.meta.model_numbers.iter().map(String::as_str));
        let regions = self.regions(
            document.meta.na.as_ref(),
            document.meta.eu.as_ref(),
            document.meta.jp.as_ref(),
        );

        let mut codes = Vec::new();
        for value in &input.button {
            codes.push(format!(
                "ButtonRecord {{ label: {}, element: {}, analog: {} }}",
                self.id_code(&value.label),
                button_element(&value.element)?,
                value.analog
            ));
        }
        let buttons = Self::push_codes(&mut self.buttons, codes);

        let mut codes = Vec::new();
        for value in &input.directional {
            if !matches!(value.directions, 4 | 8) {
                return Err(format!("{}: directional count must be 4 or 8", input.id));
            }
            codes.push(format!(
                "DirectionalRecord {{ label: {}, directions: {}, alignment: {} }}",
                self.option_id(value.label.as_deref()),
                value.directions,
                option_alignment(value.alignment.as_deref())?
            ));
        }
        let directionals = Self::push_codes(&mut self.directionals, codes);

        let mut codes = Vec::new();
        for value in &input.analog {
            if !(1..=3).contains(&value.axes) {
                return Err(format!("{}: analog axes must be 1..=3", input.id));
            }
            let class = analog_class(value.class.as_deref().unwrap_or("stick"))?;
            if class == "Slider" && value.alignment.is_none() && value.label.is_none() {
                return Err(format!(
                    "{}: slider cluster needs an alignment or label",
                    input.id
                ));
            }
            codes.push(format!("AnalogRecord {{ label: {}, axes: {}, class: AnalogClass::{class}, alignment: {}, digital: {} }}", self.option_id(value.label.as_deref()), value.axes, option_alignment(value.alignment.as_deref())?, value.digital));
        }
        let analog = Self::push_codes(&mut self.analog, codes);

        let mut codes = Vec::new();
        for value in &input.trigger {
            codes.push(format!(
                "TriggerRecord {{ label: {}, alignment: Alignment::{}, digital: {} }}",
                self.option_id(value.label.as_deref()),
                trigger_alignment(&value.alignment)?,
                value.digital
            ));
        }
        let triggers = Self::push_codes(&mut self.triggers, codes);

        let mut codes = Vec::new();
        for value in &input.rumble {
            codes.push(format!(
                "RumbleRecord {{ label: {}, size: RumbleSize::{}, alignment: {}, optional: {} }}",
                self.option_id(value.label.as_deref()),
                rumble_size(&value.size)?,
                option_alignment(value.alignment.as_deref())?,
                value.optional
            ));
        }
        let rumble = Self::push_codes(&mut self.rumble, codes);

        let mut codes = Vec::new();
        for value in &input.pointer {
            if !matches!(value.dimensions, 2 | 3) {
                return Err(format!("{}: pointer dimensions must be 2 or 3", input.id));
            }
            codes.push(format!(
                "PointerRecord {{ label: {}, dimensions: {}, alignment: {} }}",
                self.option_id(value.label.as_deref()),
                value.dimensions,
                option_alignment(value.alignment.as_deref())?
            ));
        }
        let pointers = Self::push_codes(&mut self.pointers, codes);

        let mut codes = Vec::new();
        for value in &input.microphone {
            codes.push(format!(
                "PlainRecord {{ label: {}, alignment: {} }}",
                self.option_id(value.label.as_deref()),
                option_alignment(value.alignment.as_deref())?
            ));
        }
        let microphones = Self::push_codes(&mut self.plain, codes);
        let mut codes = Vec::new();
        for value in &input.camera {
            codes.push(format!(
                "PlainRecord {{ label: {}, alignment: {} }}",
                self.option_id(value.label.as_deref()),
                option_alignment(value.alignment.as_deref())?
            ));
        }
        let cameras = Self::push_codes(&mut self.plain, codes);

        let mut codes = Vec::new();
        for value in &input.touchscreen {
            if value.width == 0 || value.height == 0 {
                return Err(format!(
                    "{}: touchscreen dimensions must be nonzero",
                    input.id
                ));
            }
            codes.push(format!(
                "TouchscreenRecord {{ label: {}, alignment: {}, width: {}, height: {} }}",
                self.option_id(value.label.as_deref()),
                option_alignment(value.alignment.as_deref())?,
                value.width,
                value.height
            ));
        }
        let touchscreens = Self::push_codes(&mut self.touchscreens, codes);

        let (elements, clusters) = self.expand_input(input)?;

        let code = format!(
            "InputRecord {{ id: {}, kind: InputKind::{kind}, name: {}, model_numbers: {}, regions: {}, buttons: {}, directionals: {}, analog: {}, triggers: {}, rumble: {}, pointers: {}, touchscreens: {}, microphones: {}, cameras: {}, elements: {}, clusters: {} }}",
            self.id_code(&input.id),
            self.option_id(document.meta.name.as_deref()),
            model_numbers.code(),
            regions.code(),
            buttons.code(),
            directionals.code(),
            analog.code(),
            triggers.code(),
            rumble.code(),
            pointers.code(),
            touchscreens.code(),
            microphones.code(),
            cameras.code(),
            elements.code(),
            clusters.code()
        );
        self.inputs.push((input.id.clone(), code));
        Ok(())
    }

    fn expand_input(&mut self, input: &InputTable) -> Result<(Span, Span), String> {
        let mut elements = Vec::new();
        let mut clusters = Vec::new();
        let mut seen = BTreeSet::new();

        for value in &input.button {
            let element = button_element(&value.element)?;
            let suffix = button_suffix(&value.element);
            let id = push_element(
                &mut elements,
                &mut seen,
                format!("Button{suffix}"),
                format!("BindingKey::Button({element})"),
                value.label.clone(),
                "Button",
            );
            clusters.push(ExpandedCluster {
                kind: "button",
                class: None,
                alignment: None,
                discriminator: Some(value.element.clone()),
                arity: 1,
                elements: vec![id],
            });
        }

        for value in &input.directional {
            let directions = if value.directions == 4 {
                &DIRECTIONS[..4]
            } else {
                &DIRECTIONS[..]
            };
            let alignment = option_alignment(value.alignment.as_deref())?;
            let label = value.label.as_deref().unwrap_or("D-Pad");
            let ids = directions
                .iter()
                .map(|(variant, suffix, direction_label)| {
                    push_element(
                        &mut elements,
                        &mut seen,
                        format!("Directional{suffix}"),
                        format!(
                            "BindingKey::Directional {{ alignment: {alignment}, direction: Direction::{variant} }}"
                        ),
                        format!("{label} {direction_label}"),
                        "Directional",
                    )
                })
                .collect();
            clusters.push(ExpandedCluster {
                kind: "directional",
                class: None,
                alignment: value.alignment.clone(),
                discriminator: value.alignment.clone(),
                arity: value.directions,
                elements: ids,
            });
        }

        for value in &input.analog {
            let class = analog_class(value.class.as_deref().unwrap_or("stick"))?;
            let alignment = option_alignment(value.alignment.as_deref())?;
            let prefix = match (class, value.alignment.as_deref()) {
                ("Stick" | "Rotary", Some("left") | None) => "AxisLeftAnalog".to_owned(),
                ("Stick" | "Rotary", Some("right")) => "AxisRightAnalog".to_owned(),
                ("Gyroscope", None) => "GyroscopeAxis".to_owned(),
                ("Accelerometer", None) => "AccelerometerAxis".to_owned(),
                (class, Some(side)) => {
                    format!("{}{}Axis", pascal_case(side), pascal_case(class))
                }
                (class, None) => format!(
                    "{}{}Axis",
                    value.label.as_deref().map(pascal_case).unwrap_or_default(),
                    pascal_case(class)
                ),
            };
            let cluster_label =
                value
                    .label
                    .clone()
                    .unwrap_or_else(|| match (class, value.alignment.as_deref()) {
                        ("Stick", None) => "Stick".to_owned(),
                        ("Stick", Some("left")) => "Left Stick".to_owned(),
                        ("Stick", Some("right")) => "Right Stick".to_owned(),
                        ("Rotary", _) => "Dial".to_owned(),
                        _ => pascal_case(class),
                    });
            let mut ids = Vec::new();
            for (axis, suffix) in AXES.iter().take(value.axes as usize) {
                for (sign, sign_name) in [("Positive", "Positive"), ("Negative", "Negative")] {
                    ids.push(push_element(
                        &mut elements,
                        &mut seen,
                        format!("{prefix}{sign_name}{suffix}"),
                        format!("BindingKey::Analog {{ class: AnalogClass::{class}, alignment: {alignment}, component: Component::new(Axis::{axis}, Sign::{sign}) }}"),
                        format!(
                            "{cluster_label} {}",
                            axis_label(class, axis, sign == "Positive")
                        ),
                        if sign == "Positive" {
                            "AxisPositive"
                        } else {
                            "AxisNegative"
                        },
                    ));
                }
            }
            clusters.push(ExpandedCluster {
                kind: "analog",
                class: Some(class),
                alignment: value.alignment.clone(),
                discriminator: value.alignment.clone(),
                arity: value.axes,
                elements: ids,
            });
        }

        for value in &input.trigger {
            let side = trigger_alignment(&value.alignment)?;
            let side_label = pascal_case(&value.alignment);
            let id = push_element(
                &mut elements,
                &mut seen,
                format!("Trigger{side_label}"),
                format!("BindingKey::Trigger {{ alignment: Alignment::{side} }}"),
                value
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("{side_label} Trigger")),
                "Trigger",
            );
            clusters.push(ExpandedCluster {
                kind: "trigger",
                class: None,
                alignment: Some(value.alignment.clone()),
                discriminator: Some(value.alignment.clone()),
                arity: 1,
                elements: vec![id],
            });
        }

        for value in &input.rumble {
            let size = rumble_size(&value.size)?;
            let motor = match value.alignment.as_deref() {
                Some(value) => {
                    format!("RumbleMotor::Aligned(Alignment::{})", alignment(value)?)
                }
                None => format!("RumbleMotor::Size(RumbleSize::{size})"),
            };
            let (suffix, fallback) = if size == "Big" {
                ("Big", "Strong Rumble")
            } else {
                ("Small", "Weak Rumble")
            };
            let id = push_element(
                &mut elements,
                &mut seen,
                format!("Rumble{suffix}"),
                format!("BindingKey::Rumble({motor})"),
                value.label.clone().unwrap_or_else(|| fallback.to_owned()),
                "Rumble",
            );
            clusters.push(ExpandedCluster {
                kind: "rumble",
                class: None,
                alignment: None,
                discriminator: value.alignment.clone().or_else(|| Some(value.size.clone())),
                arity: 1,
                elements: vec![id],
            });
        }

        for value in &input.pointer {
            let alignment = option_alignment(value.alignment.as_deref())?;
            let label = value.label.as_deref().unwrap_or("Pointer");
            push_element(
                &mut elements,
                &mut seen,
                format!("Pointer{}D", value.dimensions),
                format!("BindingKey::Pointer {{ alignment: {alignment}, component: None }}"),
                label.to_owned(),
                "Pointer",
            );
            let mut ids = Vec::new();
            for (axis, suffix) in AXES.iter().take(value.dimensions as usize) {
                for (sign, sign_name) in [("Positive", "Positive"), ("Negative", "Negative")] {
                    ids.push(push_element(
                        &mut elements,
                        &mut seen,
                        format!("PointerAxis{sign_name}{suffix}"),
                        format!("BindingKey::Pointer {{ alignment: {alignment}, component: Some(Component::new(Axis::{axis}, Sign::{sign})) }}"),
                        format!("{label} {}", axis_label("Pointer", axis, sign == "Positive")),
                        if sign == "Positive" { "AxisPositive" } else { "AxisNegative" },
                    ));
                }
            }
            clusters.push(ExpandedCluster {
                kind: "pointer",
                class: None,
                alignment: value.alignment.clone(),
                discriminator: value.alignment.clone(),
                arity: value.dimensions,
                elements: ids,
            });
        }

        for (values, name, kind) in [
            (&input.microphone, "Microphone", "Microphone"),
            (&input.camera, "Camera", "Camera"),
        ] {
            for value in values {
                let alignment = option_alignment(value.alignment.as_deref())?;
                let id = push_element(
                    &mut elements,
                    &mut seen,
                    name.to_owned(),
                    format!(
                        "BindingKey::Peripheral {{ kind: Peripheral::{kind}, alignment: {alignment} }}"
                    ),
                    value.label.clone().unwrap_or_else(|| name.to_owned()),
                    name,
                );
                clusters.push(ExpandedCluster {
                    kind: if name == "Microphone" {
                        "microphone"
                    } else {
                        "camera"
                    },
                    class: None,
                    alignment: value.alignment.clone(),
                    discriminator: value.alignment.clone(),
                    arity: 1,
                    elements: vec![id],
                });
            }
        }
        for value in &input.touchscreen {
            let alignment = option_alignment(value.alignment.as_deref())?;
            let id = push_element(
                &mut elements,
                &mut seen,
                "Touchscreen".to_owned(),
                format!(
                    "BindingKey::Peripheral {{ kind: Peripheral::Touchscreen, alignment: {alignment} }}"
                ),
                value
                    .label
                    .clone()
                    .unwrap_or_else(|| "Touchscreen".to_owned()),
                "Touchscreen",
            );
            clusters.push(ExpandedCluster {
                kind: "touchscreen",
                class: None,
                alignment: value.alignment.clone(),
                discriminator: value.alignment.clone(),
                arity: 1,
                elements: vec![id],
            });
        }

        elements.sort_by(|left, right| left.id.cmp(&right.id));
        let mut element_codes = Vec::new();
        for value in elements {
            element_codes.push(format!(
                "ElementRecord {{ id: {}, binding: {}, label: {}, kind: {} }}",
                self.id_code(&value.id),
                value.binding,
                self.id_code(&value.label),
                self.id_code(value.kind)
            ));
        }
        let elements = Self::push_codes(&mut self.elements, element_codes);

        let mut cluster_codes = Vec::new();
        for value in clusters {
            let element_ids = self.string_slice(value.elements.iter().map(String::as_str));
            let class = value
                .class
                .map(|class| format!("Some(AnalogClass::{class})"))
                .unwrap_or_else(|| "None".to_owned());
            let alignment = option_alignment(value.alignment.as_deref())?;
            cluster_codes.push(format!(
                "ClusterRecord {{ kind: {}, class: {class}, alignment: {alignment}, discriminator: {}, arity: {}, elements: {} }}",
                self.id_code(value.kind),
                self.option_id(value.discriminator.as_deref()),
                value.arity,
                element_ids.code()
            ));
        }
        let clusters = Self::push_codes(&mut self.clusters, cluster_codes);
        Ok((elements, clusters))
    }

    fn machine(&mut self, document: &MachineDocument) -> Result<(), String> {
        let identity = &document.machine;
        let meta = &document.meta;
        let dependencies = self.string_slice(identity.dependencies.iter().map(String::as_str));
        let model_numbers = self.string_slice(meta.model_numbers.iter().map(String::as_str));
        let regions = self.regions(meta.na.as_ref(), meta.eu.as_ref(), meta.jp.as_ref());

        let mut group_codes = Vec::new();
        if let Some(input) = &document.input {
            let listed: BTreeSet<_> = input.groups.iter().collect();
            for name in &input.groups {
                let value = input
                    .tables
                    .get(name)
                    .ok_or_else(|| format!("{}: input group `{name}` has no table", identity.id))?;
                let ports = match value.ports {
                    Some(0) => {
                        return Err(format!(
                            "{}: input group `{name}` has zero ports",
                            identity.id
                        ));
                    }
                    Some(ports) => ports,
                    None if value.accessories => 0,
                    None => {
                        return Err(format!(
                            "{}: input group `{name}` needs nonzero ports",
                            identity.id
                        ));
                    }
                };
                let inputs = self.string_slice(value.inputs.iter().map(String::as_str));
                group_codes.push(format!(
                    "GroupRecord {{ name: {}, inputs: {}, ports: {ports}, accessories: {} }}",
                    self.id_code(name),
                    inputs.code(),
                    value.accessories
                ));
            }
            for name in input.tables.keys() {
                if !listed.contains(name) {
                    return Err(format!(
                        "{}: input group table `{name}` is not listed",
                        identity.id
                    ));
                }
            }
        }
        let groups = Self::push_codes(&mut self.groups, group_codes);

        let accessory_values = document
            .accessories
            .as_ref()
            .map(|value| value.class.as_slice())
            .unwrap_or_default();
        let mut accessory_codes = Vec::new();
        for value in accessory_values {
            accessory_codes.push(format!("AccessoryClass::{}", accessory_class(value)?));
        }
        let accessories = Self::push_codes(&mut self.accessories, accessory_codes);

        let mut storage_codes = Vec::new();
        if let Some(storage) = &document.storage {
            let listed: BTreeSet<_> = storage.devices.iter().collect();
            for id in &storage.devices {
                let value = storage.tables.get(id).ok_or_else(|| {
                    format!("{}: storage device `{id}` has no table", identity.id)
                })?;
                let mut partition_codes = Vec::new();
                for partition in &value.partitions {
                    for reference in &partition.spec {
                        if !reference.ends_with(".mtree")
                            || Path::new(reference).is_absolute()
                            || reference.split(['/', '\\']).any(|part| part == "..")
                        {
                            return Err(format!(
                                "{}: invalid mtree reference `{reference}`",
                                identity.id
                            ));
                        }
                    }
                    let specs = self.string_slice(partition.spec.iter().map(String::as_str));
                    partition_codes.push(format!(
                        "PartitionRecord {{ id: {}, name: {}, specs: {}, user: {} }}",
                        self.id_code(&partition.id),
                        self.id_code(&partition.name),
                        specs.code(),
                        partition.user
                    ));
                }
                let partitions = Self::push_codes(&mut self.partitions, partition_codes);
                storage_codes.push(format!(
                    "StorageRecord {{ id: {}, name: {}, raw: {}, user: {}, partitions: {} }}",
                    self.id_code(id),
                    self.id_code(&value.name),
                    value.raw,
                    value.user,
                    partitions.code()
                ));
            }
            for id in storage.tables.keys() {
                if !listed.contains(id) {
                    return Err(format!(
                        "{}: storage table `{id}` is not listed",
                        identity.id
                    ));
                }
            }
        }
        let storage = Self::push_codes(&mut self.storage, storage_codes);

        let mut bios_codes = Vec::new();
        for value in &document.bios {
            validate_hashes(&identity.id, "MD5", &value.md5, 32)?;
            validate_hashes(&identity.id, "SHA-1", &value.sha1, 40)?;
            validate_hashes(&identity.id, "SHA-256", &value.sha256, 64)?;
            let md5 = self.string_slice(value.md5.iter().map(String::as_str));
            let sha1 = self.string_slice(value.sha1.iter().map(String::as_str));
            let sha256 = self.string_slice(value.sha256.iter().map(String::as_str));
            bios_codes.push(format!(
                "BiosRecord {{ name: {}, md5: {}, sha1: {}, sha256: {} }}",
                self.id_code(&value.name),
                md5.code(),
                sha1.code(),
                sha256.code()
            ));
        }
        let bios = Self::push_codes(&mut self.bios, bios_codes);

        let code = format!(
            "MachineRecord {{ id: {}, name: {}, dependencies: {}, short_name: {}, model_numbers: {}, licensor: {}, manufacturer: {}, kind: MachineKind::{}, regions: {}, groups: {}, accessories: {}, storage: {}, bios: {} }}",
            self.id_code(&identity.id),
            self.id_code(&identity.name),
            dependencies.code(),
            self.option_id(meta.short_name.as_deref()),
            model_numbers.code(),
            self.option_id(meta.licensor.as_deref()),
            self.option_id(meta.manufacturer.as_deref()),
            machine_kind(&meta.kind)?,
            regions.code(),
            groups.code(),
            accessories.code(),
            storage.code(),
            bios.code()
        );
        self.machines.push((identity.id.clone(), code));
        Ok(())
    }

    fn finish(mut self, output: &Path) {
        self.inputs.sort_by(|left, right| left.0.cmp(&right.0));
        self.machines.sort_by(|left, right| left.0.cmp(&right.0));

        let mut ordered = vec![String::new(); self.interner.len()];
        for (symbol, value) in self.interner.iter() {
            ordered[symbol.to_usize()] = value.to_owned();
        }
        let mut bytes = Vec::new();
        let mut offsets = vec![0u32];
        for value in &ordered {
            bytes.extend_from_slice(value.as_bytes());
            offsets.push(bytes.len().try_into().expect("string table exceeds 4 GiB"));
        }
        fs::write(output.join("strings.bin"), bytes).expect("write string table");

        let mut code = String::from(
            "// @generated by consolespec/build.rs; do not edit.\nstatic STRING_DATA: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/strings.bin\"));\n",
        );
        emit_numbers(&mut code, "STRING_OFFSETS", "u32", &offsets);
        emit_numbers(
            &mut code,
            "STRING_IDS",
            "StrId",
            &self
                .string_ids
                .iter()
                .map(|id| format!("StrId({id})"))
                .collect::<Vec<_>>(),
        );
        emit_codes(&mut code, "REGIONS", "RegionRecord", &self.regions);
        emit_codes(&mut code, "BUTTONS", "ButtonRecord", &self.buttons);
        emit_codes(
            &mut code,
            "DIRECTIONALS",
            "DirectionalRecord",
            &self.directionals,
        );
        emit_codes(&mut code, "ANALOG", "AnalogRecord", &self.analog);
        emit_codes(&mut code, "TRIGGERS", "TriggerRecord", &self.triggers);
        emit_codes(&mut code, "RUMBLE", "RumbleRecord", &self.rumble);
        emit_codes(&mut code, "POINTERS", "PointerRecord", &self.pointers);
        emit_codes(&mut code, "PLAIN", "PlainRecord", &self.plain);
        emit_codes(
            &mut code,
            "TOUCHSCREENS",
            "TouchscreenRecord",
            &self.touchscreens,
        );
        emit_codes(&mut code, "ELEMENTS", "ElementRecord", &self.elements);
        emit_codes(&mut code, "CLUSTERS", "ClusterRecord", &self.clusters);
        emit_codes(&mut code, "GROUPS", "GroupRecord", &self.groups);
        emit_codes(
            &mut code,
            "ACCESSORIES",
            "AccessoryClass",
            &self.accessories,
        );
        emit_codes(&mut code, "PARTITIONS", "PartitionRecord", &self.partitions);
        emit_codes(&mut code, "STORAGE", "StorageRecord", &self.storage);
        emit_codes(&mut code, "BIOS", "BiosRecord", &self.bios);
        emit_codes(
            &mut code,
            "INPUTS",
            "InputRecord",
            &self
                .inputs
                .iter()
                .map(|(_, code)| code.clone())
                .collect::<Vec<_>>(),
        );
        emit_codes(
            &mut code,
            "MACHINES",
            "MachineRecord",
            &self
                .machines
                .iter()
                .map(|(_, code)| code.clone())
                .collect::<Vec<_>>(),
        );

        emit_lookup(&mut code, "INPUT_LOOKUP", &self.inputs);
        emit_lookup(&mut code, "MACHINE_LOOKUP", &self.machines);
        fs::write(output.join("database.rs"), code).expect("write generated database");
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .canonicalize()
        .expect("consolespec repository root");
    let input_dir = root.join("inputspec");
    let machine_dir = root.join("machinespec");
    println!("cargo:rerun-if-changed={}", input_dir.display());
    println!("cargo:rerun-if-changed={}", machine_dir.display());

    let mut generator = Generator::default();
    let mut input_ids = BTreeSet::new();
    for path in toml_files(&input_dir) {
        println!("cargo:rerun-if-changed={}", path.display());
        let document: InputDocument = parse(&path);
        validate_filename(&path, &document.input.id);
        assert!(
            input_ids.insert(document.input.id.clone()),
            "duplicate inputspec {}",
            document.input.id
        );
        generator
            .input(&document)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }

    let mut machine_documents = Vec::new();
    let mut machine_ids = BTreeSet::new();
    for path in toml_files(&machine_dir) {
        println!("cargo:rerun-if-changed={}", path.display());
        let document: MachineDocument = parse(&path);
        validate_filename(&path, &document.machine.id);
        assert!(
            machine_ids.insert(document.machine.id.clone()),
            "duplicate machinespec {}",
            document.machine.id
        );
        machine_documents.push((path, document));
    }
    for (path, document) in &machine_documents {
        for dependency in &document.machine.dependencies {
            assert!(
                machine_ids.contains(dependency),
                "{}: unknown machine dependency `{dependency}`",
                path.display()
            );
        }
        if let Some(input) = &document.input {
            for group in input.tables.values() {
                for id in &group.inputs {
                    assert!(
                        input_ids.contains(id),
                        "{}: unknown inputspec `{id}`",
                        path.display()
                    );
                }
            }
        }
        generator
            .machine(document)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    }
    assert!(
        !input_ids.is_empty() && !machine_ids.is_empty(),
        "consolespec database is empty"
    );
    generator.finish(&PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")));
}

fn toml_files(directory: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .map(|entry| entry.expect("directory entry").path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn parse<T: for<'de> Deserialize<'de>>(path: &Path) -> T {
    let source =
        fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    toml::from_str(&source).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn validate_filename(path: &Path, id: &str) {
    assert_eq!(
        path.file_stem().and_then(|value| value.to_str()),
        Some(id),
        "{}: filename must match document id",
        path.display()
    );
}

fn validate_hashes(
    machine: &str,
    kind: &str,
    values: &[String],
    length: usize,
) -> Result<(), String> {
    for value in values {
        if value.len() != length
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(format!("{machine}: invalid {kind} digest `{value}`"));
        }
    }
    Ok(())
}

const DIRECTIONS: [(&str, &str, &str); 8] = [
    ("N", "N", "Up"),
    ("E", "E", "Right"),
    ("S", "S", "Down"),
    ("W", "W", "Left"),
    ("Ne", "NE", "Up Right"),
    ("Se", "SE", "Down Right"),
    ("Sw", "SW", "Down Left"),
    ("Nw", "NW", "Up Left"),
];
const AXES: [(&str, &str); 3] = [("X", "X"), ("Y", "Y"), ("Z", "Z")];

fn push_element(
    elements: &mut Vec<ExpandedElement>,
    seen: &mut BTreeSet<String>,
    id: String,
    binding: String,
    label: String,
    kind: &'static str,
) -> String {
    let id = if seen.insert(id.clone()) {
        id
    } else {
        let mut ordinal = 2;
        loop {
            let candidate = format!("{id}{ordinal}");
            if seen.insert(candidate.clone()) {
                break candidate;
            }
            ordinal += 1;
        }
    };
    elements.push(ExpandedElement {
        id: id.clone(),
        binding,
        label,
        kind,
    });
    id
}

fn button_suffix(value: &str) -> String {
    match value {
        "start" => "Start".to_owned(),
        "select" => "Select".to_owned(),
        "guide" => "Guide".to_owned(),
        "clickl" => "ClickL".to_owned(),
        "clickr" => "ClickR".to_owned(),
        value if value.len() == 1 && value.as_bytes()[0].is_ascii_alphabetic() => {
            value.to_ascii_uppercase()
        }
        value => value.to_owned(),
    }
}

fn pascal_case(label: &str) -> String {
    label
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut word = word.to_owned();
            word[..1].make_ascii_uppercase();
            word
        })
        .collect()
}

fn axis_label(class: &str, axis: &str, positive: bool) -> &'static str {
    match (class, axis, positive) {
        ("Gyroscope", "X", true) => "Roll Right",
        ("Gyroscope", "X", false) => "Roll Left",
        ("Gyroscope", "Y", true) => "Pitch Up",
        ("Gyroscope", "Y", false) => "Pitch Down",
        ("Gyroscope", _, true) => "Yaw Right",
        ("Gyroscope", _, false) => "Yaw Left",
        ("Accelerometer", "X", true) => "Right",
        ("Accelerometer", "X", false) => "Left",
        ("Accelerometer", "Y", true) => "Forward",
        ("Accelerometer", "Y", false) => "Backward",
        ("Accelerometer", _, true) => "Up",
        ("Accelerometer", _, false) => "Down",
        (_, "X", true) => "Right",
        (_, "X", false) => "Left",
        (_, "Y", true) => "Up",
        (_, "Y", false) => "Down",
        (_, _, true) => "Forward",
        (_, _, false) => "Backward",
    }
}

fn input_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "controller" => Ok("Controller"),
        "handheld" => Ok("Handheld"),
        "device" => Ok("Device"),
        _ => Err(format!("unknown input type `{value}`")),
    }
}
fn button_element(value: &str) -> Result<String, String> {
    let variant = match value {
        "a" => "A",
        "b" => "B",
        "c" => "C",
        "x" => "X",
        "y" => "Y",
        "z" => "Z",
        "l" => "L",
        "r" => "R",
        "start" => "Start",
        "select" => "Select",
        "guide" => "Guide",
        "clickl" => "ClickL",
        "clickr" => "ClickR",
        digits
            if !digits.is_empty()
                && digits.bytes().all(|byte| byte.is_ascii_digit())
                && !(digits.len() > 1 && digits.starts_with('0')) =>
        {
            let index = digits
                .parse::<u8>()
                .map_err(|_| format!("unknown button element `{value}`"))?;
            if index > 31 {
                return Err(format!("unknown button element `{value}`"));
            }
            return Ok(format!("ButtonElement::Numbered({index})"));
        }
        _ => return Err(format!("unknown button element `{value}`")),
    };
    Ok(format!("ButtonElement::{variant}"))
}
fn machine_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "console" => Ok("Console"),
        "handheld" => Ok("Handheld"),
        "addon" => Ok("Addon"),
        "arcade" => Ok("Arcade"),
        _ => Err(format!("unknown machine type `{value}`")),
    }
}
fn alignment(value: &str) -> Result<&'static str, String> {
    match value {
        "left" => Ok("Left"),
        "right" => Ok("Right"),
        "front" => Ok("Front"),
        "rear" => Ok("Rear"),
        _ => Err(format!("unknown alignment `{value}`")),
    }
}
fn trigger_alignment(value: &str) -> Result<&'static str, String> {
    match value {
        "left" => Ok("Left"),
        "right" => Ok("Right"),
        _ => Err(format!(
            "trigger alignment must be `left` or `right`, not `{value}`"
        )),
    }
}
fn option_alignment(value: Option<&str>) -> Result<String, String> {
    value
        .map(|value| alignment(value).map(|value| format!("Some(Alignment::{value})")))
        .transpose()
        .map(|value| value.unwrap_or_else(|| "None".to_owned()))
}
fn analog_class(value: &str) -> Result<&'static str, String> {
    match value {
        "stick" => Ok("Stick"),
        "slider" => Ok("Slider"),
        "rotary" => Ok("Rotary"),
        "gyroscope" => Ok("Gyroscope"),
        "accelerometer" => Ok("Accelerometer"),
        _ => Err(format!("unknown analog class `{value}`")),
    }
}
fn rumble_size(value: &str) -> Result<&'static str, String> {
    match value {
        "big" => Ok("Big"),
        "small" => Ok("Small"),
        _ => Err(format!("unknown rumble size `{value}`")),
    }
}
fn accessory_class(value: &str) -> Result<&'static str, String> {
    match value {
        "disney_infinity" => Ok("DisneyInfinity"),
        "skylander" => Ok("Skylander"),
        "lego_dimensions" => Ok("LegoDimensions"),
        "guitar" => Ok("Guitar"),
        "piano" => Ok("Piano"),
        "drums" => Ok("Drums"),
        "keyboard" => Ok("Keyboard"),
        "mouse" => Ok("Mouse"),
        "camera" => Ok("Camera"),
        "microphone" => Ok("Microphone"),
        "storage" => Ok("Storage"),
        _ => Err(format!("unknown accessory class `{value}`")),
    }
}

fn emit_codes(output: &mut String, name: &str, kind: &str, values: &[String]) {
    writeln!(output, "static {name}: &[{kind}] = &[").unwrap();
    for value in values {
        writeln!(output, "    {value},").unwrap();
    }
    output.push_str("];\n");
}

fn emit_numbers<T: fmt::Display>(output: &mut String, name: &str, kind: &str, values: &[T]) {
    writeln!(output, "static {name}: &[{kind}] = &[").unwrap();
    for value in values {
        writeln!(output, "    {value},").unwrap();
    }
    output.push_str("];\n");
}

fn emit_lookup(output: &mut String, name: &str, records: &[(String, String)]) {
    let mut map = phf_codegen::Map::new();
    let values = (0..records.len())
        .map(|index| format!("{index}u32"))
        .collect::<Vec<_>>();
    for ((id, _), value) in records.iter().zip(&values) {
        map.entry(id, value);
    }
    writeln!(
        output,
        "static {name}: phf::Map<&'static str, u32> = {};",
        map.build()
    )
    .unwrap();
}
