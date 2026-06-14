//! Width-correct operations on [`Value`].
//!
//! Every result is re-masked to the operand's width, mirroring how a fixed-width
//! hardware register truncates and wraps. Binary operations take the width of
//! the left-hand operand and re-mask the right-hand operand to it. The two
//! operations that depend on interpretation — right shift (logical vs
//! arithmetic) and division/remainder (unsigned vs signed) — take a
//! [`Signedness`].

use crate::value::{Signedness, Value};

impl Value {
    /// Right-hand raw bits, re-masked to this value's width.
    fn rhs_raw(self, rhs: Value) -> u128 {
        rhs.raw() & self.width().mask()
    }

    // --- Bitwise ---------------------------------------------------------

    pub fn and(self, rhs: Value) -> Value {
        self.with_raw(self.raw() & self.rhs_raw(rhs))
    }

    pub fn or(self, rhs: Value) -> Value {
        self.with_raw(self.raw() | self.rhs_raw(rhs))
    }

    pub fn xor(self, rhs: Value) -> Value {
        self.with_raw(self.raw() ^ self.rhs_raw(rhs))
    }

    pub fn nand(self, rhs: Value) -> Value {
        self.with_raw(!(self.raw() & self.rhs_raw(rhs)))
    }

    pub fn nor(self, rhs: Value) -> Value {
        self.with_raw(!(self.raw() | self.rhs_raw(rhs)))
    }

    pub fn xnor(self, rhs: Value) -> Value {
        self.with_raw(!(self.raw() ^ self.rhs_raw(rhs)))
    }

    /// Bitwise NOT, within the width (so the high bits beyond the width stay 0).
    pub fn not(self) -> Value {
        self.with_raw(!self.raw())
    }

    // --- Arithmetic ------------------------------------------------------

    pub fn add(self, rhs: Value) -> Value {
        self.with_raw(self.raw().wrapping_add(self.rhs_raw(rhs)))
    }

    pub fn sub(self, rhs: Value) -> Value {
        self.with_raw(self.raw().wrapping_sub(self.rhs_raw(rhs)))
    }

    pub fn mul(self, rhs: Value) -> Value {
        self.with_raw(self.raw().wrapping_mul(self.rhs_raw(rhs)))
    }

    /// Arithmetic negation (two's complement), within the width.
    pub fn neg(self) -> Value {
        self.with_raw(self.raw().wrapping_neg())
    }

    /// Division. Returns `None` on divide-by-zero. Unsigned uses raw bits;
    /// signed uses the two's-complement interpretation.
    pub fn div(self, rhs: Value, sign: Signedness) -> Option<Value> {
        let divisor = self.rhs_raw(rhs);
        if divisor == 0 {
            return None;
        }
        match sign {
            Signedness::Unsigned => Some(self.with_raw(self.raw() / divisor)),
            Signedness::Signed => {
                let q = self.as_signed().wrapping_div(self.with_raw(divisor).as_signed());
                Some(self.with_raw(q as u128))
            }
        }
    }

    /// Remainder. Returns `None` on divide-by-zero.
    pub fn rem(self, rhs: Value, sign: Signedness) -> Option<Value> {
        let divisor = self.rhs_raw(rhs);
        if divisor == 0 {
            return None;
        }
        match sign {
            Signedness::Unsigned => Some(self.with_raw(self.raw() % divisor)),
            Signedness::Signed => {
                let r = self.as_signed().wrapping_rem(self.with_raw(divisor).as_signed());
                Some(self.with_raw(r as u128))
            }
        }
    }

    // --- Shifts and rotates ---------------------------------------------

    /// Logical left shift. Bits shifted past the width are dropped.
    pub fn shl(self, amount: u32) -> Value {
        let w = self.width().bits();
        if amount >= w {
            self.with_raw(0)
        } else {
            self.with_raw(self.raw() << amount)
        }
    }

    /// Right shift. Unsigned/logical fills with zeros; signed/arithmetic
    /// sign-extends (fills with copies of the sign bit).
    pub fn shr(self, amount: u32, sign: Signedness) -> Value {
        let w = self.width().bits();
        match sign {
            Signedness::Unsigned => {
                if amount >= w {
                    self.with_raw(0)
                } else {
                    self.with_raw(self.raw() >> amount)
                }
            }
            Signedness::Signed => {
                let negative = self.raw() & self.width().sign_bit() != 0;
                if amount >= w {
                    self.with_raw(if negative { self.width().mask() } else { 0 })
                } else {
                    let logical = self.raw() >> amount;
                    if negative {
                        // Set the top `amount` bits (within width) back to 1.
                        let fill = self.width().mask() & !(self.width().mask() >> amount);
                        self.with_raw(logical | fill)
                    } else {
                        self.with_raw(logical)
                    }
                }
            }
        }
    }

    /// Rotate left within the width.
    pub fn rotl(self, amount: u32) -> Value {
        let w = self.width().bits();
        let a = amount % w;
        if a == 0 {
            return self;
        }
        let raw = self.raw();
        self.with_raw((raw << a) | (raw >> (w - a)))
    }

    /// Rotate right within the width.
    pub fn rotr(self, amount: u32) -> Value {
        let w = self.width().bits();
        let a = amount % w;
        if a == 0 {
            return self;
        }
        let raw = self.raw();
        self.with_raw((raw >> a) | (raw << (w - a)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Width;

    fn v(raw: u128, bits: u32) -> Value {
        Value::new(raw, Width::new(bits).unwrap())
    }

    #[test]
    fn bitwise_within_width() {
        assert_eq!(v(0b1100, 4).and(v(0b1010, 4)).raw(), 0b1000);
        assert_eq!(v(0b1100, 4).or(v(0b1010, 4)).raw(), 0b1110);
        assert_eq!(v(0b1100, 4).xor(v(0b1010, 4)).raw(), 0b0110);
        // NOT respects the width: !0xF0 in 8 bits is 0x0F, not 0xFFFF_FF0F.
        assert_eq!(v(0xF0, 8).not().raw(), 0x0F);
        assert_eq!(v(0b1100, 4).nand(v(0b1010, 4)).raw(), 0b0111);
        assert_eq!(v(0b1100, 4).nor(v(0b1010, 4)).raw(), 0b0001);
        assert_eq!(v(0b1100, 4).xnor(v(0b1010, 4)).raw(), 0b1001);
    }

    #[test]
    fn arithmetic_overflow_truncates() {
        // 0xFF + 1 in 8 bits wraps to 0x00.
        assert_eq!(v(0xFF, 8).add(v(1, 8)).raw(), 0x00);
        // 200 * 2 = 400 = 0x190, truncated to 8 bits = 0x90.
        assert_eq!(v(200, 8).mul(v(2, 8)).raw(), 0x90);
        // 0 - 1 wraps to all ones.
        assert_eq!(v(0, 8).sub(v(1, 8)).raw(), 0xFF);
        // neg of 1 in 8 bits is 0xFF (-1).
        assert_eq!(v(1, 8).neg().raw(), 0xFF);
    }

    #[test]
    fn logical_vs_arithmetic_shift() {
        // 0x80 (=-128 signed) >> 1.
        let x = v(0x80, 8);
        assert_eq!(x.shr(1, Signedness::Unsigned).raw(), 0x40);
        assert_eq!(x.shr(1, Signedness::Signed).raw(), 0xC0); // sign-extended
        // arithmetic shift of a positive value behaves like logical.
        assert_eq!(v(0x40, 8).shr(1, Signedness::Signed).raw(), 0x20);
    }

    #[test]
    fn shift_by_width_or_more() {
        assert_eq!(v(0xFF, 8).shl(8).raw(), 0x00);
        assert_eq!(v(0xFF, 8).shl(100).raw(), 0x00);
        assert_eq!(v(0xFF, 8).shr(8, Signedness::Unsigned).raw(), 0x00);
        // signed shift by >= width saturates to all sign bits.
        assert_eq!(v(0x80, 8).shr(8, Signedness::Signed).raw(), 0xFF);
        assert_eq!(v(0x7F, 8).shr(8, Signedness::Signed).raw(), 0x00);
    }

    #[test]
    fn shl_truncates_into_width() {
        // 0x0F << 4 = 0xF0 in 8 bits; the high nibble that would overflow is gone.
        assert_eq!(v(0x0F, 8).shl(4).raw(), 0xF0);
        assert_eq!(v(0xFF, 8).shl(4).raw(), 0xF0);
    }

    #[test]
    fn rotates_wrap_within_width() {
        // 0b1000_0001 rol 1 -> 0b0000_0011.
        assert_eq!(v(0x81, 8).rotl(1).raw(), 0x03);
        // 0b1000_0001 ror 1 -> 0b1100_0000.
        assert_eq!(v(0x81, 8).rotr(1).raw(), 0xC0);
        // rotating by the full width is a no-op.
        assert_eq!(v(0xA5, 8).rotl(8).raw(), 0xA5);
        assert_eq!(v(0xA5, 8).rotr(8).raw(), 0xA5);
        // rotating by more than the width wraps modulo width.
        assert_eq!(v(0x81, 8).rotl(9).raw(), v(0x81, 8).rotl(1).raw());
    }

    #[test]
    fn division_signed_and_unsigned() {
        // -8 / 2 signed = -4 (0xFC); unsigned 0xF8 / 2 = 0x7C.
        let x = v(0xF8, 8); // 248 unsigned, -8 signed
        assert_eq!(x.div(v(2, 8), Signedness::Signed).unwrap().raw(), 0xFC);
        assert_eq!(x.div(v(2, 8), Signedness::Unsigned).unwrap().raw(), 0x7C);
        // divide by zero -> None.
        assert!(x.div(v(0, 8), Signedness::Unsigned).is_none());
        assert!(x.rem(v(0, 8), Signedness::Signed).is_none());
        // signed MIN / -1 wraps instead of panicking.
        assert_eq!(v(0x80, 8).div(v(0xFF, 8), Signedness::Signed).unwrap().raw(), 0x80);
    }
}
