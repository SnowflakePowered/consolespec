//! Binding keys — the typed form of `button.a`, `analog.stick.left.x+`.
//!
//! A key names one control of one device. It is built from the declaring
//! cluster's own fields and nothing else, so a device gaining a cluster never
//! renames an existing key. consolespec's inputspec is the grammar's authority:
//! every value below is one of that schema's `allowedvalues`.
//!
//! ```text
//! button.<element>
//! directional[.<alignment>].<direction>
//! analog.<class>[.<alignment>].<axis><sign>
//! trigger.<alignment>
//! rumble.<alignment|size>
//! pointer[.<alignment>][.<axis><sign>]
//! touchscreen|microphone|camera[.<alignment>]
//! ```
//!
//! The dotted string remains the *wire* form — it is what a TOML table key, a
//! JSON object key and the frontend all carry — but nothing downstream picks it
//! apart with `starts_with` or a `Positive`/`Negative` substring. [`BindingKey`]
//! is `Copy` and parses without allocating, so there is no reason to keep a
//! string around once a document has been read.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// A button's semantic element: the lettered face buttons, the digital
/// shoulders, the UI controls, the stick clicks, or a bare number for a button
/// with no shared meaning.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ButtonElement {
    A,
    B,
    C,
    X,
    Y,
    Z,
    L,
    R,
    Start,
    Select,
    Guide,
    ClickL,
    ClickR,
    /// `button.0` through `button.31`.
    Numbered(u8),
}

/// Where a cluster sits. Present only where a device has two of one kind and
/// class to tell apart — the N64's D-pad and C-buttons, the Wii Remote's
/// accelerometer and the Nunchuk's.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Alignment {
    Left,
    Right,
    Front,
    Rear,
}

/// One direction of a directional cluster: the cardinals, then the
/// intercardinals a `directions = 8` cluster adds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Direction {
    N,
    E,
    S,
    W,
    Ne,
    Se,
    Sw,
    Nw,
}

/// What an analog cluster *is*. Never where it sits — that is [`Alignment`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AnalogClass {
    Stick,
    Slider,
    Rotary,
    Gyroscope,
    Accelerometer,
}

/// One degree of freedom of an analog or pointer cluster.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Axis {
    X,
    Y,
    Z,
    W,
    /// For a cluster with more degrees of freedom than the alphabet spends on
    /// them. No inputspec declares one today; the grammar admits it so that one
    /// eventually can without a second key format.
    Numbered(u8),
}

/// Which half of an axis. An inputspec declares the degree of freedom; the two
/// signed halves are what a binding names, because most emulators bind them
/// separately.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Sign {
    Positive,
    Negative,
}

/// One signed half of one axis: the `x+` of `analog.stick.left.x+`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Component {
    pub axis: Axis,
    pub sign: Sign,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RumbleSize {
    Big,
    Small,
}

/// Which motor. A device with one of each size names them by size; one with two
/// of a size — the DualSense's two small motors — names them by side instead.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RumbleMotor {
    Size(RumbleSize),
    Aligned(Alignment),
}

/// The kinds whose whole cluster is a single control.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Peripheral {
    Touchscreen,
    Microphone,
    Camera,
}

/// One control of one device.
///
/// `alignment` is `Option` exactly where the grammar makes it optional, so an
/// unqualified key is representable and means "the device's primary cluster of
/// this kind. Consumers may resolve it against a device's primary cluster.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BindingKey {
    Button(ButtonElement),
    Directional {
        alignment: Option<Alignment>,
        direction: Direction,
    },
    Analog {
        class: AnalogClass,
        alignment: Option<Alignment>,
        component: Component,
    },
    Trigger {
        alignment: Alignment,
    },
    Rumble(RumbleMotor),
    Pointer {
        alignment: Option<Alignment>,
        /// `None` is the absolute position, which is the cluster itself.
        component: Option<Component>,
    },
    Peripheral {
        kind: Peripheral,
        alignment: Option<Alignment>,
    },
}

impl BindingKey {
    /// Whether this key names a continuous degree of freedom, and so accepts an
    /// axis binding. Buttons and directionals are digital.
    pub fn is_analog(&self) -> bool {
        matches!(self, Self::Analog { .. } | Self::Trigger { .. })
    }

    /// This key with its alignment cleared, which is the form a canonical
    /// document — the keyboard defaults, the SDL button defaults — writes.
    pub fn unaligned(self) -> Self {
        match self {
            Self::Directional { direction, .. } => Self::Directional {
                alignment: None,
                direction,
            },
            Self::Analog {
                class, component, ..
            } => Self::Analog {
                class,
                alignment: None,
                component,
            },
            Self::Pointer { component, .. } => Self::Pointer {
                alignment: None,
                component,
            },
            Self::Peripheral { kind, .. } => Self::Peripheral {
                kind,
                alignment: None,
            },
            other => other,
        }
    }

    /// This key with `alignment` substituted, for rebuilding a canonical key
    /// against the cluster that actually answers it.
    pub fn with_alignment(self, alignment: Option<Alignment>) -> Self {
        match self {
            Self::Directional { direction, .. } => Self::Directional {
                alignment,
                direction,
            },
            Self::Analog {
                class, component, ..
            } => Self::Analog {
                class,
                alignment,
                component,
            },
            Self::Pointer { component, .. } => Self::Pointer {
                alignment,
                component,
            },
            Self::Peripheral { kind, .. } => Self::Peripheral { kind, alignment },
            other => other,
        }
    }
}

/// Why a string is not a binding key. Carrying the offending text is what makes
/// a mistyped table key diagnosable at load rather than silently dropped.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseKeyError(String);

impl fmt::Display for ParseKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "`{}` is not a binding key", self.0)
    }
}

impl std::error::Error for ParseKeyError {}

impl ButtonElement {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "x" => Self::X,
            "y" => Self::Y,
            "z" => Self::Z,
            "l" => Self::L,
            "r" => Self::R,
            "start" => Self::Start,
            "select" => Self::Select,
            "guide" => Self::Guide,
            "clickl" => Self::ClickL,
            "clickr" => Self::ClickR,
            // Leading zeros would give one button two spellings.
            digits if digits.bytes().all(|byte| byte.is_ascii_digit()) => {
                let index: u8 = digits.parse().ok()?;
                if index > 31 || (digits.len() > 1 && digits.starts_with('0')) {
                    return None;
                }
                Self::Numbered(index)
            }
            _ => return None,
        })
    }
}

impl FromStr for ButtonElement {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for ButtonElement {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::A => formatter.write_str("a"),
            Self::B => formatter.write_str("b"),
            Self::C => formatter.write_str("c"),
            Self::X => formatter.write_str("x"),
            Self::Y => formatter.write_str("y"),
            Self::Z => formatter.write_str("z"),
            Self::L => formatter.write_str("l"),
            Self::R => formatter.write_str("r"),
            Self::Start => formatter.write_str("start"),
            Self::Select => formatter.write_str("select"),
            Self::Guide => formatter.write_str("guide"),
            Self::ClickL => formatter.write_str("clickl"),
            Self::ClickR => formatter.write_str("clickr"),
            Self::Numbered(index) => write!(formatter, "{index}"),
        }
    }
}

impl Alignment {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "left" => Self::Left,
            "right" => Self::Right,
            "front" => Self::Front,
            "rear" => Self::Rear,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Front => "front",
            Self::Rear => "rear",
        }
    }
}

impl FromStr for Alignment {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for Alignment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Direction {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "n" => Self::N,
            "e" => Self::E,
            "s" => Self::S,
            "w" => Self::W,
            "ne" => Self::Ne,
            "se" => Self::Se,
            "sw" => Self::Sw,
            "nw" => Self::Nw,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::N => "n",
            Self::E => "e",
            Self::S => "s",
            Self::W => "w",
            Self::Ne => "ne",
            Self::Se => "se",
            Self::Sw => "sw",
            Self::Nw => "nw",
        }
    }

    /// The label an inputspec would have used, for a UI that shows the control
    /// rather than its key.
    pub fn label(self) -> &'static str {
        match self {
            Self::N => "Up",
            Self::E => "Right",
            Self::S => "Down",
            Self::W => "Left",
            Self::Ne => "Up Right",
            Self::Se => "Down Right",
            Self::Sw => "Down Left",
            Self::Nw => "Up Left",
        }
    }
}

impl FromStr for Direction {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AnalogClass {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "stick" => Self::Stick,
            "slider" => Self::Slider,
            "rotary" => Self::Rotary,
            "gyroscope" => Self::Gyroscope,
            "accelerometer" => Self::Accelerometer,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stick => "stick",
            Self::Slider => "slider",
            Self::Rotary => "rotary",
            Self::Gyroscope => "gyroscope",
            Self::Accelerometer => "accelerometer",
        }
    }
}

impl FromStr for AnalogClass {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for AnalogClass {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Axis {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "x" => Self::X,
            "y" => Self::Y,
            "z" => Self::Z,
            "w" => Self::W,
            digits if digits.bytes().all(|byte| byte.is_ascii_digit()) => {
                if digits.is_empty() || (digits.len() > 1 && digits.starts_with('0')) {
                    return None;
                }
                Self::Numbered(digits.parse().ok()?)
            }
            _ => return None,
        })
    }

    /// The nth axis of a compiled inputspec cluster.
    pub fn nth(index: usize) -> Self {
        match index {
            0 => Self::X,
            1 => Self::Y,
            2 => Self::Z,
            3 => Self::W,
            other => Self::Numbered(other as u8),
        }
    }
}

impl FromStr for Axis {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for Axis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X => formatter.write_str("x"),
            Self::Y => formatter.write_str("y"),
            Self::Z => formatter.write_str("z"),
            Self::W => formatter.write_str("w"),
            Self::Numbered(index) => write!(formatter, "{index}"),
        }
    }
}

impl Component {
    pub const fn new(axis: Axis, sign: Sign) -> Self {
        Self { axis, sign }
    }

    pub fn is_positive(self) -> bool {
        self.sign == Sign::Positive
    }

    /// A component is the only part of a key that ends in a sign, which is what
    /// tells it apart from an alignment in the same position.
    fn parse(text: &str) -> Option<Self> {
        let (axis, sign) = match text.as_bytes().last()? {
            b'+' => (&text[..text.len() - 1], Sign::Positive),
            b'-' => (&text[..text.len() - 1], Sign::Negative),
            _ => return None,
        };
        Some(Self {
            axis: Axis::parse(axis)?,
            sign,
        })
    }
}

impl FromStr for Component {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for Component {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{}",
            self.axis,
            if self.is_positive() { '+' } else { '-' }
        )
    }
}

impl RumbleSize {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "big" => Self::Big,
            "small" => Self::Small,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Big => "big",
            Self::Small => "small",
        }
    }
}

impl FromStr for RumbleSize {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for RumbleMotor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Size(size) => formatter.write_str(size.as_str()),
            Self::Aligned(alignment) => formatter.write_str(alignment.as_str()),
        }
    }
}

impl Peripheral {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "touchscreen" => Self::Touchscreen,
            "microphone" => Self::Microphone,
            "camera" => Self::Camera,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Touchscreen => "touchscreen",
            Self::Microphone => "microphone",
            Self::Camera => "camera",
        }
    }
}

impl FromStr for Peripheral {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

impl fmt::Display for Peripheral {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for BindingKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn aligned(
            formatter: &mut fmt::Formatter<'_>,
            alignment: Option<Alignment>,
        ) -> fmt::Result {
            match alignment {
                Some(alignment) => write!(formatter, ".{alignment}"),
                None => Ok(()),
            }
        }
        match self {
            Self::Button(element) => write!(formatter, "button.{element}"),
            Self::Directional {
                alignment,
                direction,
            } => {
                formatter.write_str("directional")?;
                aligned(formatter, *alignment)?;
                write!(formatter, ".{direction}")
            }
            Self::Analog {
                class,
                alignment,
                component,
            } => {
                write!(formatter, "analog.{class}")?;
                aligned(formatter, *alignment)?;
                write!(formatter, ".{component}")
            }
            Self::Trigger { alignment } => write!(formatter, "trigger.{alignment}"),
            Self::Rumble(motor) => write!(formatter, "rumble.{motor}"),
            Self::Pointer {
                alignment,
                component,
            } => {
                formatter.write_str("pointer")?;
                aligned(formatter, *alignment)?;
                match component {
                    Some(component) => write!(formatter, ".{component}"),
                    None => Ok(()),
                }
            }
            Self::Peripheral { kind, alignment } => {
                write!(formatter, "{kind}")?;
                aligned(formatter, *alignment)
            }
        }
    }
}

impl FromStr for BindingKey {
    type Err = ParseKeyError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        parse(text).ok_or_else(|| ParseKeyError(text.to_owned()))
    }
}

/// Splitting on `.` yields at most four parts, so the parser reads them into a
/// fixed array rather than a collection — no allocation, and the arity checks
/// below are exhaustive over what the grammar admits.
fn parse(text: &str) -> Option<BindingKey> {
    let mut parts = [""; 4];
    let mut count = 0;
    for part in text.split('.') {
        if count == parts.len() || part.is_empty() {
            return None;
        }
        parts[count] = part;
        count += 1;
    }

    Some(match &parts[..count] {
        ["button", element] => BindingKey::Button(ButtonElement::parse(element)?),
        // Directions and alignments are disjoint words, so which one a lone
        // middle part is never has to be guessed.
        ["directional", direction] => BindingKey::Directional {
            alignment: None,
            direction: Direction::parse(direction)?,
        },
        ["directional", alignment, direction] => BindingKey::Directional {
            alignment: Some(Alignment::parse(alignment)?),
            direction: Direction::parse(direction)?,
        },
        ["analog", class, component] => BindingKey::Analog {
            class: AnalogClass::parse(class)?,
            alignment: None,
            component: Component::parse(component)?,
        },
        ["analog", class, alignment, component] => BindingKey::Analog {
            class: AnalogClass::parse(class)?,
            alignment: Some(Alignment::parse(alignment)?),
            component: Component::parse(component)?,
        },
        ["trigger", alignment] => BindingKey::Trigger {
            alignment: Alignment::parse(alignment)?,
        },
        ["rumble", motor] => BindingKey::Rumble(match RumbleSize::parse(motor) {
            Some(size) => RumbleMotor::Size(size),
            None => RumbleMotor::Aligned(Alignment::parse(motor)?),
        }),
        // A component always ends in a sign and an alignment never does, so the
        // one optional part in the middle is unambiguous here too.
        ["pointer"] => BindingKey::Pointer {
            alignment: None,
            component: None,
        },
        ["pointer", tail] => match Component::parse(tail) {
            Some(component) => BindingKey::Pointer {
                alignment: None,
                component: Some(component),
            },
            None => BindingKey::Pointer {
                alignment: Some(Alignment::parse(tail)?),
                component: None,
            },
        },
        ["pointer", alignment, component] => BindingKey::Pointer {
            alignment: Some(Alignment::parse(alignment)?),
            component: Some(Component::parse(component)?),
        },
        [kind] => BindingKey::Peripheral {
            kind: Peripheral::parse(kind)?,
            alignment: None,
        },
        [kind, alignment] => BindingKey::Peripheral {
            kind: Peripheral::parse(kind)?,
            alignment: Some(Alignment::parse(alignment)?),
        },
        _ => return None,
    })
}

/// The wire form stays the dotted string: it is what a TOML table key, a JSON
/// object key and the frontend all carry, and a typed key that serialized as a
/// struct could not be a map key at all.
impl Serialize for BindingKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for BindingKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <std::borrow::Cow<'de, str>>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str) -> BindingKey {
        let key: BindingKey = text
            .parse()
            .unwrap_or_else(|_| panic!("{text} should parse"));
        assert_eq!(key.to_string(), text, "{text} should round-trip");
        key
    }

    #[test]
    fn every_shape_of_the_grammar_round_trips() {
        assert_eq!(round_trip("button.a"), BindingKey::Button(ButtonElement::A));
        assert_eq!(
            round_trip("button.31"),
            BindingKey::Button(ButtonElement::Numbered(31))
        );
        assert_eq!(
            round_trip("directional.n"),
            BindingKey::Directional {
                alignment: None,
                direction: Direction::N
            }
        );
        assert_eq!(
            round_trip("directional.right.nw"),
            BindingKey::Directional {
                alignment: Some(Alignment::Right),
                direction: Direction::Nw
            }
        );
        assert_eq!(
            round_trip("analog.stick.left.x+"),
            BindingKey::Analog {
                class: AnalogClass::Stick,
                alignment: Some(Alignment::Left),
                component: Component::new(Axis::X, Sign::Positive),
            }
        );
        assert_eq!(
            round_trip("analog.accelerometer.z-"),
            BindingKey::Analog {
                class: AnalogClass::Accelerometer,
                alignment: None,
                component: Component::new(Axis::Z, Sign::Negative),
            }
        );
        round_trip("trigger.left");
        round_trip("rumble.big");
        round_trip("rumble.right");
        round_trip("pointer");
        round_trip("pointer.x+");
        round_trip("pointer.rear.y-");
        round_trip("touchscreen");
        round_trip("microphone.front");
    }

    #[test]
    fn a_lone_middle_part_is_never_ambiguous() {
        // An alignment is a word, a component ends in a sign, a direction is
        // neither — so the same position parses three ways without lookahead.
        assert_eq!(
            "pointer.rear".parse(),
            Ok(BindingKey::Pointer {
                alignment: Some(Alignment::Rear),
                component: None
            })
        );
        assert_eq!(
            "pointer.y-".parse(),
            Ok(BindingKey::Pointer {
                alignment: None,
                component: Some(Component::new(Axis::Y, Sign::Negative))
            })
        );
        // `rumble.left` is a side, `rumble.small` a size, in the same slot.
        assert_eq!(
            "rumble.left".parse(),
            Ok(BindingKey::Rumble(RumbleMotor::Aligned(Alignment::Left)))
        );
        assert_eq!(
            "rumble.small".parse(),
            Ok(BindingKey::Rumble(RumbleMotor::Size(RumbleSize::Small)))
        );
    }

    #[test]
    fn rejects_what_the_grammar_does_not_admit() {
        for text in [
            "",
            "button",
            "button.",
            "button.q",
            "button.32",
            "button.01",
            "ButtonA",
            "directional.left",
            "directional.n.left",
            "analog.stick.left",
            "analog.wheel.x+",
            "analog.stick.left.x",
            "analog.stick.middle.x+",
            "trigger",
            "trigger.big",
            "rumble.x+",
            "pointer.x",
            "pointer..x+",
            "touchscreen.x+",
            "nonsense",
            "analog.stick.left.x+.extra",
        ] {
            assert!(
                text.parse::<BindingKey>().is_err(),
                "`{text}` should not parse"
            );
        }
    }

    #[test]
    fn alignment_can_be_erased_and_restored() {
        let aligned: BindingKey = "analog.stick.left.x+".parse().unwrap();
        assert_eq!(aligned.unaligned().to_string(), "analog.stick.x+");
        assert_eq!(
            aligned.unaligned().with_alignment(Some(Alignment::Right)),
            "analog.stick.right.x+".parse().unwrap()
        );
        // A button has no alignment to erase and is left alone.
        let button: BindingKey = "button.a".parse().unwrap();
        assert_eq!(button.unaligned(), button);
    }

    #[test]
    fn only_continuous_controls_accept_an_axis() {
        for text in ["analog.stick.left.x+", "trigger.right"] {
            assert!(text.parse::<BindingKey>().unwrap().is_analog(), "{text}");
        }
        for text in ["button.a", "directional.n", "rumble.big", "pointer.x+"] {
            assert!(!text.parse::<BindingKey>().unwrap().is_analog(), "{text}");
        }
    }
}
