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
                self.id_code(&value.element),
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
            codes.push(format!("AnalogRecord {{ label: {}, axes: {}, class: AnalogClass::{}, alignment: {}, digital: {} }}", self.option_id(value.label.as_deref()), value.axes, analog_class(value.class.as_deref().unwrap_or("stick"))?, option_alignment(value.alignment.as_deref())?, value.digital));
        }
        let analog = Self::push_codes(&mut self.analog, codes);

        let mut codes = Vec::new();
        for value in &input.trigger {
            codes.push(format!(
                "TriggerRecord {{ label: {}, alignment: Alignment::{}, digital: {} }}",
                self.option_id(value.label.as_deref()),
                alignment(&value.alignment)?,
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

        let code = format!(
            "InputRecord {{ id: {}, kind: InputKind::{kind}, name: {}, model_numbers: {}, regions: {}, buttons: {}, directionals: {}, analog: {}, triggers: {}, rumble: {}, pointers: {}, touchscreens: {}, microphones: {}, cameras: {} }}",
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
            cameras.code()
        );
        self.inputs.push((input.id.clone(), code));
        Ok(())
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

        let input_lookup = self
            .inputs
            .iter()
            .enumerate()
            .map(|(index, (id, _))| {
                format!("({}, {index})", self.interner.get(id).unwrap().to_usize())
            })
            .map(|value| value.replacen('(', "(StrId(", 1).replacen(',', "),", 1))
            .collect::<Vec<_>>();
        let machine_lookup = self
            .machines
            .iter()
            .enumerate()
            .map(|(index, (id, _))| {
                format!("({}, {index})", self.interner.get(id).unwrap().to_usize())
            })
            .map(|value| value.replacen('(', "(StrId(", 1).replacen(',', "),", 1))
            .collect::<Vec<_>>();
        emit_codes(&mut code, "INPUT_LOOKUP", "(StrId, u32)", &input_lookup);
        emit_codes(&mut code, "MACHINE_LOOKUP", "(StrId, u32)", &machine_lookup);
        fs::write(output.join("database.rs"), code).expect("write generated database");
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest
        .join("../../consolespec")
        .canonicalize()
        .expect("consolespec data directory");
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

fn input_kind(value: &str) -> Result<&'static str, String> {
    match value {
        "controller" => Ok("Controller"),
        "handheld" => Ok("Handheld"),
        "device" => Ok("Device"),
        _ => Err(format!("unknown input type `{value}`")),
    }
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
