//! `key!` macro for writing binding key literals.

/// Macro for writing binding key literals. See [`crate::input`].
#[doc(hidden)]
#[macro_export]
macro_rules! __consolespec_key {
    // button.<element>
    (button.$element:tt) => {
        $crate::input::BindingKey::Button($crate::__button_element!($element))
    };

    // directional[.<alignment>].<direction>
    (directional.$alignment:ident.$direction:ident) => {
        $crate::input::BindingKey::Directional {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            direction: $crate::__direction!($direction),
        }
    };
    (directional.$direction:ident) => {
        $crate::input::BindingKey::Directional {
            alignment: ::core::option::Option::None,
            direction: $crate::__direction!($direction),
        }
    };

    // analog.<class>[.<alignment>].<axis><sign>
    (analog.$class:ident.$alignment:ident.$axis:tt $sign:tt) => {
        $crate::input::BindingKey::Analog {
            class: $crate::__analog_class!($class),
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: $crate::__component!($axis $sign),
        }
    };
    (analog.$class:ident.$axis:tt $sign:tt) => {
        $crate::input::BindingKey::Analog {
            class: $crate::__analog_class!($class),
            alignment: ::core::option::Option::None,
            component: $crate::__component!($axis $sign),
        }
    };

    // trigger.<alignment>
    (trigger.$alignment:ident) => {
        $crate::input::BindingKey::Trigger { alignment: $crate::__alignment!($alignment) }
    };

    // rumble.<alignment|size>
    (rumble.$motor:ident) => {
        $crate::input::BindingKey::Rumble($crate::__rumble_motor!($motor))
    };

    // pointer[.<alignment>][.<axis><sign>]
    (pointer.$alignment:ident.$axis:tt $sign:tt) => {
        $crate::input::BindingKey::Pointer {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: ::core::option::Option::Some($crate::__component!($axis $sign)),
        }
    };
    (pointer.$axis:tt $sign:tt) => {
        $crate::input::BindingKey::Pointer {
            alignment: ::core::option::Option::None,
            component: ::core::option::Option::Some($crate::__component!($axis $sign)),
        }
    };
    (pointer.$alignment:ident) => {
        $crate::input::BindingKey::Pointer {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: ::core::option::Option::None,
        }
    };
    (pointer) => {
        $crate::input::BindingKey::Pointer {
            alignment: ::core::option::Option::None,
            component: ::core::option::Option::None,
        }
    };

    // touchscreen|microphone|camera[.<alignment>]
    ($kind:ident.$alignment:ident) => {
        $crate::input::BindingKey::Peripheral {
            kind: $crate::__peripheral!($kind),
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
        }
    };
    ($kind:ident) => {
        $crate::input::BindingKey::Peripheral {
            kind: $crate::__peripheral!($kind),
            alignment: ::core::option::Option::None,
        }
    };
}

/// A button's element. Numbers stay numbers; everything else is a named variant,
/// so a misspelling names a variant that does not exist and fails to compile.
#[doc(hidden)]
#[macro_export]
macro_rules! __button_element {
    (a) => {
        $crate::input::ButtonElement::A
    };
    (b) => {
        $crate::input::ButtonElement::B
    };
    (c) => {
        $crate::input::ButtonElement::C
    };
    (x) => {
        $crate::input::ButtonElement::X
    };
    (y) => {
        $crate::input::ButtonElement::Y
    };
    (z) => {
        $crate::input::ButtonElement::Z
    };
    (l) => {
        $crate::input::ButtonElement::L
    };
    (r) => {
        $crate::input::ButtonElement::R
    };
    (start) => {
        $crate::input::ButtonElement::Start
    };
    (select) => {
        $crate::input::ButtonElement::Select
    };
    (guide) => {
        $crate::input::ButtonElement::Guide
    };
    (clickl) => {
        $crate::input::ButtonElement::ClickL
    };
    (clickr) => {
        $crate::input::ButtonElement::ClickR
    };
    // The schema stops at 31; the bound is checked here so it cannot be
    // exceeded in a const the parser would have rejected as text.
    ($index:literal) => {
        $crate::input::ButtonElement::Numbered(
            const {
                assert!($index <= 31, "button elements stop at 31");
                $index
            },
        )
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __alignment {
    (left) => {
        $crate::input::Alignment::Left
    };
    (right) => {
        $crate::input::Alignment::Right
    };
    (front) => {
        $crate::input::Alignment::Front
    };
    (rear) => {
        $crate::input::Alignment::Rear
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __direction {
    (n) => {
        $crate::input::Direction::N
    };
    (e) => {
        $crate::input::Direction::E
    };
    (s) => {
        $crate::input::Direction::S
    };
    (w) => {
        $crate::input::Direction::W
    };
    (ne) => {
        $crate::input::Direction::Ne
    };
    (se) => {
        $crate::input::Direction::Se
    };
    (sw) => {
        $crate::input::Direction::Sw
    };
    (nw) => {
        $crate::input::Direction::Nw
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __analog_class {
    (stick) => {
        $crate::input::AnalogClass::Stick
    };
    (slider) => {
        $crate::input::AnalogClass::Slider
    };
    (rotary) => {
        $crate::input::AnalogClass::Rotary
    };
    (gyroscope) => {
        $crate::input::AnalogClass::Gyroscope
    };
    (accelerometer) => {
        $crate::input::AnalogClass::Accelerometer
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rumble_motor {
    (big) => {
        $crate::input::RumbleMotor::Size($crate::input::RumbleSize::Big)
    };
    (small) => {
        $crate::input::RumbleMotor::Size($crate::input::RumbleSize::Small)
    };
    ($alignment:ident) => {
        $crate::input::RumbleMotor::Aligned($crate::__alignment!($alignment))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __peripheral {
    (touchscreen) => {
        $crate::input::Peripheral::Touchscreen
    };
    (microphone) => {
        $crate::input::Peripheral::Microphone
    };
    (camera) => {
        $crate::input::Peripheral::Camera
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __axis {
    (x) => {
        $crate::input::Axis::X
    };
    (y) => {
        $crate::input::Axis::Y
    };
    (z) => {
        $crate::input::Axis::Z
    };
    (w) => {
        $crate::input::Axis::W
    };
    ($index:literal) => {
        $crate::input::Axis::Numbered($index)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __sign {
    (+) => {
        $crate::input::Sign::Positive
    };
    (-) => {
        $crate::input::Sign::Negative
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __component {
    ($axis:tt $sign:tt) => {
        $crate::input::Component {
            axis: $crate::__axis!($axis),
            sign: $crate::__sign!($sign),
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::input::{
        Alignment, AnalogClass, Axis, BindingKey, ButtonElement, Component, Direction, Peripheral,
        RumbleMotor, RumbleSize, Sign, key,
    };

    /// The macro and the parser are two spellings of one grammar; every shape is
    /// checked against both, so neither can drift.
    #[test]
    fn every_shape_agrees_with_the_parser() {
        macro_rules! same {
            ($text:literal, $($key:tt)+) => {
                assert_eq!(key!($($key)+), $text.parse::<BindingKey>().unwrap(), $text);
                assert_eq!(key!($($key)+).to_string(), $text);
            };
        }
        same!("button.a", button.a);
        same!("button.clickr", button.clickr);
        same!("button.0", button.0);
        same!("button.31", button.31);
        same!("directional.n", directional.n);
        same!("directional.right.nw", directional.right.nw);
        same!("analog.stick.left.x+", analog.stick.left.x+);
        same!("analog.stick.right.y-", analog.stick.right.y-);
        same!("analog.accelerometer.z-", analog.accelerometer.z-);
        same!("analog.rotary.x+", analog.rotary.x+);
        same!("trigger.left", trigger.left);
        same!("rumble.big", rumble.big);
        same!("rumble.small", rumble.small);
        same!("rumble.right", rumble.right);
        same!("pointer", pointer);
        same!("pointer.x+", pointer.x+);
        same!("pointer.rear.y-", pointer.rear.y-);
        same!("pointer.front", pointer.front);
        same!("touchscreen", touchscreen);
        same!("microphone.front", microphone.front);
        same!("camera", camera);
    }

    /// The numbered forms are the ones a lexer could plausibly mangle — `.0`
    /// after an identifier must stay two tokens and not become a float.
    #[test]
    fn numbered_buttons_survive_the_lexer() {
        assert_eq!(
            key!(button.0),
            BindingKey::Button(ButtonElement::Numbered(0))
        );
        assert_eq!(
            key!(button.7),
            BindingKey::Button(ButtonElement::Numbered(7))
        );
        assert_eq!(
            key!(button.31),
            BindingKey::Button(ButtonElement::Numbered(31))
        );
    }

    #[test]
    fn keys_are_const_and_usable_as_patterns() {
        const SOUTH: BindingKey = key!(button.a);
        const LEFT_X: BindingKey = key!(analog.stick.left.x+);
        // A `"…".parse()` could be neither of these.
        static WIRE: [BindingKey; 2] = [SOUTH, LEFT_X];
        assert!(matches!(WIRE[0], SOUTH));
        assert!(matches!(WIRE[1], LEFT_X));
    }

    #[test]
    fn expands_to_the_expected_values() {
        assert_eq!(
            key!(directional.left.se),
            BindingKey::Directional {
                alignment: Some(Alignment::Left),
                direction: Direction::Se,
            }
        );
        assert_eq!(
            key!(analog.gyroscope.right.z+),
            BindingKey::Analog {
                class: AnalogClass::Gyroscope,
                alignment: Some(Alignment::Right),
                component: Component {
                    axis: Axis::Z,
                    sign: Sign::Positive
                },
            }
        );
        assert_eq!(
            key!(rumble.small),
            BindingKey::Rumble(RumbleMotor::Size(RumbleSize::Small))
        );
        assert_eq!(
            key!(camera.rear),
            BindingKey::Peripheral {
                kind: Peripheral::Camera,
                alignment: Some(Alignment::Rear),
            }
        );
    }
}
