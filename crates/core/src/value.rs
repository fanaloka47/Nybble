//! The canonical value model: [`Width`], [`Signedness`], and [`Value`].
//!
//! A [`Value`] stores a raw `u128` bit pattern that is *always* masked to its
//! [`Width`]. Hex/binary/octal renderings show that raw pattern directly;
//! only the decimal rendering depends on [`Signedness`] (two's complement).

/// A bit width, constrained to `1..=128`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Width(u32);

impl Width {
    pub const MIN: u32 = 1;
    pub const MAX: u32 = 128;

    /// Create a width, returning `None` if `bits` is outside `1..=128`.
    pub fn new(bits: u32) -> Option<Width> {
        if (Self::MIN..=Self::MAX).contains(&bits) {
            Some(Width(bits))
        } else {
            None
        }
    }

    /// Create a width, clamping into `1..=128` instead of failing.
    pub fn clamped(bits: u32) -> Width {
        Width(bits.clamp(Self::MIN, Self::MAX))
    }

    /// The number of bits.
    pub fn bits(self) -> u32 {
        self.0
    }

    /// A mask with the low `bits` bits set. Handles the width-128 case where
    /// `1 << 128` would overflow.
    pub fn mask(self) -> u128 {
        if self.0 == 128 {
            u128::MAX
        } else {
            (1u128 << self.0) - 1
        }
    }

    /// A mask with only the most-significant (sign) bit of this width set.
    pub fn sign_bit(self) -> u128 {
        1u128 << (self.0 - 1)
    }
}

/// How a value's bits are interpreted as a number for decimal display and for
/// the distinction between logical and arithmetic right shift.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Signedness {
    Unsigned,
    Signed,
}

/// A width-bounded integer value, stored as a raw bit pattern masked to width.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Value {
    raw: u128,
    width: Width,
}

impl Value {
    /// Build a value from a raw bit pattern, masking it to `width`.
    pub fn new(raw: u128, width: Width) -> Value {
        Value {
            raw: raw & width.mask(),
            width,
        }
    }

    /// The raw, width-masked bit pattern.
    pub fn raw(self) -> u128 {
        self.raw
    }

    pub fn width(self) -> Width {
        self.width
    }

    /// Same bits, re-masked to a new width (truncates when narrowing).
    pub fn with_width(self, width: Width) -> Value {
        Value::new(self.raw, width)
    }

    /// Same width, new raw bits (re-masked).
    pub fn with_raw(self, raw: u128) -> Value {
        Value::new(raw, self.width)
    }

    /// Interpret the bits as a signed two's-complement integer.
    pub fn as_signed(self) -> i128 {
        let w = self.width.bits();
        if w == 128 {
            // The full 128-bit pattern is already a valid i128 two's complement.
            self.raw as i128
        } else if self.raw & self.width.sign_bit() != 0 {
            // Sign bit set: value = raw - 2^width.
            (self.raw as i128) - (1i128 << w)
        } else {
            self.raw as i128
        }
    }

    /// Interpret the bits as an unsigned integer.
    pub fn as_unsigned(self) -> u128 {
        self.raw
    }

    /// Hexadecimal, zero-padded to the full width and grouped in 4-digit
    /// chunks, e.g. a 32-bit `0xDEADBEEF` renders as `DEAD_BEEF`.
    pub fn to_hex(self) -> String {
        let digits = self.width.bits().div_ceil(4) as usize;
        let s = format!("{:0width$X}", self.raw, width = digits);
        group(&s, '_', 4)
    }

    /// Binary, zero-padded to the full width and grouped in 4-bit nibbles.
    pub fn to_bin(self) -> String {
        let digits = self.width.bits() as usize;
        let s = format!("{:0width$b}", self.raw, width = digits);
        group(&s, '_', 4)
    }

    /// Octal, zero-padded to cover the width and grouped in 3-digit chunks.
    pub fn to_oct(self) -> String {
        let digits = self.width.bits().div_ceil(3) as usize;
        let s = format!("{:0width$o}", self.raw, width = digits);
        group(&s, '_', 3)
    }

    /// Decimal. Unsigned shows the raw magnitude; signed shows the
    /// two's-complement interpretation. Not zero-padded; apostrophe thousands
    /// separators are inserted for readability (e.g. `1'000'000`).
    pub fn to_dec(self, sign: Signedness) -> String {
        let raw = match sign {
            Signedness::Unsigned => self.raw.to_string(),
            Signedness::Signed => self.as_signed().to_string(),
        };
        // Insert ' every 3 digits from the right, skipping a leading '-'.
        let (sign_char, digits) = if raw.starts_with('-') {
            ("-", &raw[1..])
        } else {
            ("", raw.as_str())
        };
        format!("{}{}", sign_char, group(digits, '\'', 3))
    }
}

/// Insert `sep` every `group_size` characters counting from the right.
/// `group("DEADBEEF", '_', 4)` -> `"DEAD_BEEF"`.
fn group(s: &str, sep: char, group_size: usize) -> String {
    let len = s.len();
    let mut out = String::with_capacity(len + len / group_size);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(group_size) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(bits: u32) -> Width {
        Width::new(bits).unwrap()
    }

    #[test]
    fn width_bounds() {
        assert!(Width::new(0).is_none());
        assert!(Width::new(129).is_none());
        assert_eq!(Width::new(1).unwrap().bits(), 1);
        assert_eq!(Width::new(128).unwrap().bits(), 128);
        assert_eq!(Width::clamped(0).bits(), 1);
        assert_eq!(Width::clamped(999).bits(), 128);
    }

    #[test]
    fn masks() {
        assert_eq!(w(8).mask(), 0xFF);
        assert_eq!(w(1).mask(), 0x1);
        assert_eq!(w(128).mask(), u128::MAX);
        assert_eq!(w(8).sign_bit(), 0x80);
        assert_eq!(w(32).sign_bit(), 1u128 << 31);
    }

    #[test]
    fn new_masks_to_width() {
        // 0x1FF in an 8-bit value keeps only the low byte.
        assert_eq!(Value::new(0x1FF, w(8)).raw(), 0xFF);
        assert_eq!(Value::new(u128::MAX, w(4)).raw(), 0xF);
    }

    #[test]
    fn signed_vs_unsigned_decimal_same_bits() {
        // 0xFF at width 8 is 255 unsigned, -1 signed. No separator needed.
        let v = Value::new(0xFF, w(8));
        assert_eq!(v.to_dec(Signedness::Unsigned), "255");
        assert_eq!(v.to_dec(Signedness::Signed), "-1");

        // 0x80 at width 8 is the most negative value.
        let v = Value::new(0x80, w(8));
        assert_eq!(v.to_dec(Signedness::Unsigned), "128");
        assert_eq!(v.to_dec(Signedness::Signed), "-128");

        // 0x7F is positive in both.
        let v = Value::new(0x7F, w(8));
        assert_eq!(v.to_dec(Signedness::Unsigned), "127");
        assert_eq!(v.to_dec(Signedness::Signed), "127");
    }

    #[test]
    fn decimal_thousands_separator() {
        // 1_000 — exactly 4 digits, gets one separator.
        let v = Value::new(1_000, w(16));
        assert_eq!(v.to_dec(Signedness::Unsigned), "1'000");

        // 1_000_000 — 7 digits.
        let v = Value::new(1_000_000, w(32));
        assert_eq!(v.to_dec(Signedness::Unsigned), "1'000'000");

        // Signed negative: separator appears only in the digit run, not before '-'.
        let v = Value::new(0xFFFF_D8F0, w(32)); // -10000 signed
        assert_eq!(v.to_dec(Signedness::Signed), "-10'000");

        // Values under 1000 need no separator.
        let v = Value::new(999, w(16));
        assert_eq!(v.to_dec(Signedness::Unsigned), "999");
    }

    #[test]
    fn signed_decimal_width_128() {
        let v = Value::new(u128::MAX, w(128));
        assert_eq!(v.as_signed(), -1);
        assert_eq!(v.to_dec(Signedness::Signed), "-1");
        // u128::MAX has 39 digits; just check apostrophes appear.
        let s = v.to_dec(Signedness::Unsigned);
        assert!(s.contains('\''));
    }

    #[test]
    fn two_complement_round_trip() {
        // For every width, signed(-k) bits re-read as -k.
        for bits in [4u32, 8, 16, 32, 64] {
            let width = w(bits);
            for k in [1i128, 2, 7, 100] {
                if k < (1i128 << (bits - 1)) {
                    let raw = ((-k) as u128) & width.mask();
                    assert_eq!(Value::new(raw, width).as_signed(), -k, "bits={bits} k={k}");
                }
            }
        }
    }

    #[test]
    fn hex_padding_and_grouping() {
        assert_eq!(Value::new(0xDEADBEEF, w(32)).to_hex(), "DEAD_BEEF");
        assert_eq!(Value::new(0x5, w(8)).to_hex(), "05");
        assert_eq!(Value::new(0xF, w(4)).to_hex(), "F");
        assert_eq!(Value::new(0x1, w(16)).to_hex(), "0001");
    }

    #[test]
    fn bin_padding_and_grouping() {
        assert_eq!(Value::new(0b1101, w(8)).to_bin(), "0000_1101");
        assert_eq!(Value::new(0b1, w(4)).to_bin(), "0001");
        assert_eq!(Value::new(0xFF, w(8)).to_bin(), "1111_1111");
    }

    #[test]
    fn oct_padding_and_grouping() {
        assert_eq!(Value::new(0o377, w(8)).to_oct(), "377");
        assert_eq!(Value::new(0o1, w(8)).to_oct(), "001");
    }

    #[test]
    fn with_width_truncates() {
        let v = Value::new(0xDEADBEEF, w(32));
        assert_eq!(v.with_width(w(8)).raw(), 0xEF);
        assert_eq!(v.with_width(w(16)).raw(), 0xBEEF);
    }
}
