//! Fixed-point (Qm.n) interpretation of a [`Value`].
//!
//! A Qm.n number treats the low `n` bits as a fraction: the real value is the
//! integer interpretation divided by `2^n`. With `n` fractional bits, the
//! remaining `width - n` bits are the integer part. Signedness selects whether
//! the integer interpretation is unsigned or two's-complement.
//!
//! Conversion goes through `f64`, which is a display/entry convenience — for
//! very wide values or many fractional bits the 53-bit mantissa loses
//! precision. The raw bit pattern in [`Value`] remains the source of truth.

use crate::value::{Signedness, Value, Width};

/// Interpret `value` as Qm.n and return the real number it represents.
pub fn to_real(value: Value, frac_bits: u32, sign: Signedness) -> f64 {
    let scale = 2f64.powi(frac_bits as i32);
    let int = match sign {
        Signedness::Unsigned => value.as_unsigned() as f64,
        Signedness::Signed => value.as_signed() as f64,
    };
    int / scale
}

/// Convert a real number to the nearest Qm.n raw value of the given width.
///
/// The result is rounded to the nearest representable step (`2^-n`) and masked
/// to `width`; negative values land on their two's-complement pattern. Casting
/// out-of-range or NaN inputs saturates/zeroes per Rust's `as` semantics.
pub fn from_real(real: f64, width: Width, frac_bits: u32) -> Value {
    let scale = 2f64.powi(frac_bits as i32);
    let scaled = (real * scale).round();
    let int = scaled as i128;
    Value::new(int as u128, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(raw: u128, bits: u32) -> Value {
        Value::new(raw, Width::new(bits).unwrap())
    }

    fn w(bits: u32) -> Width {
        Width::new(bits).unwrap()
    }

    #[test]
    fn unsigned_q4_4() {
        // 0x18 = 24, /16 = 1.5
        assert_eq!(to_real(v(0x18, 8), 4, Signedness::Unsigned), 1.5);
        // 0xFF = 255, /16 = 15.9375
        assert_eq!(to_real(v(0xFF, 8), 4, Signedness::Unsigned), 15.9375);
    }

    #[test]
    fn signed_q4_4() {
        // 0xF8 = -8 signed, /16 = -0.5
        assert_eq!(to_real(v(0xF8, 8), 4, Signedness::Signed), -0.5);
        // 0x80 = -128 signed, /16 = -8.0
        assert_eq!(to_real(v(0x80, 8), 4, Signedness::Signed), -8.0);
    }

    #[test]
    fn frac_bits_zero_is_integer() {
        assert_eq!(to_real(v(5, 8), 0, Signedness::Unsigned), 5.0);
        assert_eq!(from_real(5.0, w(8), 0).raw(), 5);
    }

    #[test]
    fn from_real_round_trip() {
        assert_eq!(from_real(1.5, w(8), 4).raw(), 0x18);
        assert_eq!(from_real(-0.5, w(8), 4).raw(), 0xF8);
        assert_eq!(from_real(15.9375, w(8), 4).raw(), 0xFF);
    }

    #[test]
    fn from_real_rounds_to_nearest_step() {
        // 1.53 in Q4.4: nearest step is 1.5 (0x18) since step is 1/16 = 0.0625.
        // 1.53 * 16 = 24.48 -> rounds to 24 = 0x18.
        assert_eq!(from_real(1.53, w(8), 4).raw(), 0x18);
        // 1.57 * 16 = 25.12 -> rounds to 25 = 0x19 = 1.5625.
        assert_eq!(from_real(1.57, w(8), 4).raw(), 0x19);
    }

    #[test]
    fn round_trip_many() {
        for raw in [0u128, 1, 0x18, 0x7F, 0x80, 0xC0, 0xFF] {
            for &sign in &[Signedness::Unsigned, Signedness::Signed] {
                let val = v(raw, 8);
                let real = to_real(val, 4, sign);
                assert_eq!(from_real(real, w(8), 4).raw(), raw, "raw={raw:#x} sign={sign:?}");
            }
        }
    }
}
