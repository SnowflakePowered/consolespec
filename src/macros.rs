//! `key!` macro for writing binding key literals.

/// Macro for writing binding key literals. See [the module docs](self).
#[macro_export]
macro_rules! key {
    // button.<element>
    (button.$element:tt) => {
        $crate::BindingKey::Button($crate::__button_element!($element))
    };

    // directional[.<alignment>].<direction>
    (directional.$alignment:ident.$direction:ident) => {
        $crate::BindingKey::Directional {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            direction: $crate::__direction!($direction),
        }
    };
    (directional.$direction:ident) => {
        $crate::BindingKey::Directional {
            alignment: ::core::option::Option::None,
            direction: $crate::__direction!($direction),
        }
    };

    // analog.<class>[.<alignment>].<axis><sign>
    (analog.$class:ident.$alignment:ident.$axis:tt $sign:tt) => {
        $crate::BindingKey::Analog {
            class: $crate::__analog_class!($class),
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: $crate::__component!($axis $sign),
        }
    };
    (analog.$class:ident.$axis:tt $sign:tt) => {
        $crate::BindingKey::Analog {
            class: $crate::__analog_class!($class),
            alignment: ::core::option::Option::None,
            component: $crate::__component!($axis $sign),
        }
    };

    // trigger.<alignment>
    (trigger.$alignment:ident) => {
        $crate::BindingKey::Trigger { alignment: $crate::__alignment!($alignment) }
    };

    // rumble.<alignment|size>
    (rumble.$motor:ident) => {
        $crate::BindingKey::Rumble($crate::__rumble_motor!($motor))
    };

    // pointer[.<alignment>][.<axis><sign>]
    (pointer.$alignment:ident.$axis:tt $sign:tt) => {
        $crate::BindingKey::Pointer {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: ::core::option::Option::Some($crate::__component!($axis $sign)),
        }
    };
    (pointer.$axis:tt $sign:tt) => {
        $crate::BindingKey::Pointer {
            alignment: ::core::option::Option::None,
            component: ::core::option::Option::Some($crate::__component!($axis $sign)),
        }
    };
    (pointer.$alignment:ident) => {
        $crate::BindingKey::Pointer {
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
            component: ::core::option::Option::None,
        }
    };
    (pointer) => {
        $crate::BindingKey::Pointer {
            alignment: ::core::option::Option::None,
            component: ::core::option::Option::None,
        }
    };

    // touchscreen|microphone|camera[.<alignment>]
    ($kind:ident.$alignment:ident) => {
        $crate::BindingKey::Peripheral {
            kind: $crate::__peripheral!($kind),
            alignment: ::core::option::Option::Some($crate::__alignment!($alignment)),
        }
    };
    ($kind:ident) => {
        $crate::BindingKey::Peripheral {
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
        $crate::ButtonElement::A
    };
    (b) => {
        $crate::ButtonElement::B
    };
    (c) => {
        $crate::ButtonElement::C
    };
    (x) => {
        $crate::ButtonElement::X
    };
    (y) => {
        $crate::ButtonElement::Y
    };
    (z) => {
        $crate::ButtonElement::Z
    };
    (l) => {
        $crate::ButtonElement::L
    };
    (r) => {
        $crate::ButtonElement::R
    };
    (start) => {
        $crate::ButtonElement::Start
    };
    (select) => {
        $crate::ButtonElement::Select
    };
    (guide) => {
        $crate::ButtonElement::Guide
    };
    (clickl) => {
        $crate::ButtonElement::ClickL
    };
    (clickr) => {
        $crate::ButtonElement::ClickR
    };
    // The schema stops at 31; the bound is checked here so it cannot be
    // exceeded in a const the parser would have rejected as text.
    ($index:literal) => {
        $crate::ButtonElement::Numbered(
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
        $crate::Alignment::Left
    };
    (right) => {
        $crate::Alignment::Right
    };
    (front) => {
        $crate::Alignment::Front
    };
    (rear) => {
        $crate::Alignment::Rear
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __direction {
    (n) => {
        $crate::Direction::N
    };
    (e) => {
        $crate::Direction::E
    };
    (s) => {
        $crate::Direction::S
    };
    (w) => {
        $crate::Direction::W
    };
    (ne) => {
        $crate::Direction::Ne
    };
    (se) => {
        $crate::Direction::Se
    };
    (sw) => {
        $crate::Direction::Sw
    };
    (nw) => {
        $crate::Direction::Nw
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __analog_class {
    (stick) => {
        $crate::AnalogClass::Stick
    };
    (slider) => {
        $crate::AnalogClass::Slider
    };
    (rotary) => {
        $crate::AnalogClass::Rotary
    };
    (gyroscope) => {
        $crate::AnalogClass::Gyroscope
    };
    (accelerometer) => {
        $crate::AnalogClass::Accelerometer
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rumble_motor {
    (big) => {
        $crate::RumbleMotor::Size($crate::RumbleSize::Big)
    };
    (small) => {
        $crate::RumbleMotor::Size($crate::RumbleSize::Small)
    };
    ($alignment:ident) => {
        $crate::RumbleMotor::Aligned($crate::__alignment!($alignment))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __peripheral {
    (touchscreen) => {
        $crate::Peripheral::Touchscreen
    };
    (microphone) => {
        $crate::Peripheral::Microphone
    };
    (camera) => {
        $crate::Peripheral::Camera
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __axis {
    (x) => {
        $crate::Axis::X
    };
    (y) => {
        $crate::Axis::Y
    };
    (z) => {
        $crate::Axis::Z
    };
    (w) => {
        $crate::Axis::W
    };
    ($index:literal) => {
        $crate::Axis::Numbered($index)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __sign {
    (+) => {
        $crate::Sign::Positive
    };
    (-) => {
        $crate::Sign::Negative
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __component {
    ($axis:tt $sign:tt) => {
        $crate::Component {
            axis: $crate::__axis!($axis),
            sign: $crate::__sign!($sign),
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        Alignment, AnalogClass, Axis, BindingKey, ButtonElement, Component, Direction, Peripheral,
        RumbleMotor, RumbleSize, Sign,
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
