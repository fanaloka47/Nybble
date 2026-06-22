//! PowerCalc core: pure, UI-free numeric logic.
//!
//! Everything the calculator can compute lives here and is unit-tested without
//! any GUI. The central type is [`Value`] — a raw bit pattern (`u128`) that is
//! always masked to a [`Width`] (1..=128 bits). [`Signedness`] is *not* stored
//! on the value: it only matters when you ask for a decimal rendering or an
//! arithmetic (sign-extending) right shift, so it is passed in at those points.

pub mod expr;
pub mod fixed;
pub mod float;
pub mod ops;
pub mod parse;
pub mod value;

pub use expr::{eval, EvalError};
pub use float::{eval_float, f64_to_value};
pub use parse::{parse_literal, ParseError};
pub use value::{Signedness, Value, Width};
