# API Contracts — `powercalc-core`

_Generated: 2026-06-16 · Updated: 2026-06-23 (full rescan) · Scan level: exhaustive_

This is the public Rust API of the `powercalc-core` library — the contract the GUI
(and any future consumer) depends on. There are no network/HTTP endpoints; the
"API" is the crate's exported types and functions.

## Re-exports (`lib.rs`)

```rust
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
```

## `value` module

### `Width` — bit width in `1..=128`
```rust
pub struct Width(/* private */ u32);

impl Width {
    pub const MIN: u32 = 1;
    pub const MAX: u32 = 128;
    pub fn new(bits: u32) -> Option<Width>;   // None if out of 1..=128
    pub fn clamped(bits: u32) -> Width;       // clamps into 1..=128
    pub fn bits(self) -> u32;
    pub fn mask(self) -> u128;                // low-`bits` mask (handles 128)
    pub fn sign_bit(self) -> u128;            // MSB-of-width mask
}
// derives: Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash
```

### `Signedness` — interpretation selector
```rust
pub enum Signedness { Unsigned, Signed }
// derives: Clone, Copy, PartialEq, Eq, Debug, Hash
// NOT stored on Value; passed in for decimal render, >>, and / %.
```

### `Value` — width-masked raw bit pattern
```rust
pub struct Value { /* private */ raw: u128, width: Width }

impl Value {
    pub fn new(raw: u128, width: Width) -> Value;  // masks raw to width
    pub fn raw(self) -> u128;                       // width-masked bits
    pub fn width(self) -> Width;
    pub fn with_width(self, width: Width) -> Value; // re-mask to new width
    pub fn with_raw(self, raw: u128) -> Value;      // new bits, same width
    pub fn as_signed(self) -> i128;                 // two's-complement
    pub fn as_unsigned(self) -> u128;
    pub fn to_hex(self) -> String;                  // padded, 4-grouped "DEAD_BEEF"
    pub fn to_bin(self) -> String;                  // padded, nibble-grouped
    pub fn to_oct(self) -> String;                  // padded, 3-grouped
    pub fn to_dec(self, sign: Signedness) -> String;// signed honours two's comp
}
// derives: Clone, Copy, PartialEq, Eq, Debug, Hash
```

## `ops` module — methods on `Value`

All results are re-masked to the left operand's width; the right operand is
re-masked to that width before use.

```rust
impl Value {
    // Bitwise
    pub fn and(self, rhs: Value) -> Value;
    pub fn or(self, rhs: Value) -> Value;
    pub fn xor(self, rhs: Value) -> Value;
    pub fn nand(self, rhs: Value) -> Value;
    pub fn nor(self, rhs: Value) -> Value;
    pub fn xnor(self, rhs: Value) -> Value;
    pub fn not(self) -> Value;                 // within width

    // Arithmetic (wrapping)
    pub fn add(self, rhs: Value) -> Value;
    pub fn sub(self, rhs: Value) -> Value;
    pub fn mul(self, rhs: Value) -> Value;
    pub fn neg(self) -> Value;                 // two's-complement negate

    // Division / remainder — None on divide-by-zero
    pub fn div(self, rhs: Value, sign: Signedness) -> Option<Value>;
    pub fn rem(self, rhs: Value, sign: Signedness) -> Option<Value>;

    // Shifts / rotates
    pub fn shl(self, amount: u32) -> Value;                 // logical; 0 if amount>=width
    pub fn shr(self, amount: u32, sign: Signedness) -> Value; // logical/arithmetic
    pub fn rotl(self, amount: u32) -> Value;               // wraps mod width
    pub fn rotr(self, amount: u32) -> Value;               // wraps mod width
}
```

## `parse` module

```rust
pub fn parse_literal(s: &str) -> Result<u128, ParseError>;

pub enum ParseError { Empty, InvalidDigit(char), Overflow }
// impl Display + std::error::Error
```
Accepted: `0x..` / `0b..` / `0o..` prefixes (case-insensitive) or bare decimal;
`_` separators ignored. Bare hex letters are NOT numbers (no prefix → identifier).

## `expr` module

```rust
pub fn eval(
    input: &str,
    width: Width,
    sign: Signedness,
    ans: Value,
) -> Result<Value, EvalError>;

pub enum EvalError {
    Parse(ParseError),
    BadChar(char),
    LoneAngle(char),       // `<`/`>` not doubled into a shift
    UnexpectedEof,
    UnexpectedToken,
    UnbalancedParen,
    UnknownIdent(String),  // identifier other than `ans`
    DivByZero,
    BitwiseInFloatMode(char), // `& | ^ ~ << >>` used in float-mode eval
}
// impl Display + std::error::Error + From<ParseError>
// Shared by both `eval` and `eval_float`.
```

**Operators** (loosest→tightest): `|` < `^` < `&` < `<< >>` < `+ -` < `* / %` <
unary `- ~` < primary. Primaries: literals, `ans` (current value, case-insensitive),
`(` … `)`. Width-masks every literal; `sign` drives `>>` and `/ %`.

Example: `eval("0xFF & (1 << 3)", Width::new(32)?, Signedness::Unsigned, ans)` → `0x08`.

## `float` module

```rust
pub fn eval_float(input: &str, ans: f64) -> Result<f64, EvalError>;
pub fn f64_to_value(x: f64) -> Value;   // f64 IEEE-754 bits as a 64-bit Value
```

The calculator's **float mode**: a full-precision `f64` sibling to `eval`. Same
surface grammar minus bitwise/shift: `+ - * / %`, unary `-`, parentheses, and the
`ans` identifier. Literals accept decimals and scientific notation (`1.5`, `1e6`,
`1.5e-3`) plus base-prefixed/plain integers widened to `f64`. Bitwise/shift
operators raise `EvalError::BitwiseInFloatMode(c)`. IEEE specials are allowed
(`1/0` → `inf`, `0/0` → `nan`). `f64_to_value` is for rendering a float result in
hex/bin/oct via its 64-bit encoding (the active integer `Width` does not apply).

Example: `eval_float("1500e6 * 8 / 1.024e6", 0.0)` → `11718.75`.

## `fixed` module

```rust
pub fn to_real(value: Value, frac_bits: u32, sign: Signedness) -> f64; // int / 2^n
pub fn from_real(real: f64, width: Width, frac_bits: u32) -> Value;    // round, mask
```
Qm.n: low `frac_bits` are the fraction. Conversion goes through `f64`
(precision-limited for wide values); the `Value` bits remain authoritative.

## Stability note

All types are `0.1.0` and pre-1.0 — no stability guarantees yet. `Width` and
`Value` fields are private; construct via `Width::new`/`clamped` and `Value::new`.
