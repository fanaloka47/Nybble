# Architecture — `powercalc-core`

_Generated: 2026-06-16 · Part type: library · Scan level: exhaustive_

## Executive summary

`powercalc-core` is a dependency-free Rust library that holds **all** of
PowerCalc's numeric logic. It is designed to be fully exercised without a GUI and
carries the entire automated test suite (35 unit tests across the modules). The
GUI is a thin consumer of this crate.

## Technology stack

| Category   | Detail |
|------------|--------|
| Language   | Rust, edition 2021 |
| Crate type | `lib` (`name = "powercalc_core"`, `path = src/lib.rs`) |
| Dependencies | **None** (std only) |
| Version    | 0.1.0 |

## Architecture pattern

A small **domain model + pure functions** design. There is one central data type,
`Value`, and every operation is a method or free function that takes values and
returns new values — no interior mutability, no I/O, no globals. `Value`, `Width`
and `Signedness` are all `Copy`.

### The canonical value model (`value.rs`)

- **`Width(u32)`** — a bit width constrained to `1..=128`.
  - `new(bits) -> Option<Width>` (rejects out-of-range), `clamped(bits)` (clamps).
  - `mask()` — low-`n`-bits mask, special-casing 128 to avoid `1 << 128` overflow.
  - `sign_bit()` — mask with only the MSB of the width set.
- **`Signedness { Unsigned, Signed }`** — *not* stored on a value. It only matters
  for decimal rendering (two's complement) and for the logical-vs-arithmetic right
  shift and unsigned-vs-signed division/remainder. It is passed in at those points.
- **`Value { raw: u128, width: Width }`** — a raw bit pattern that is **always
  masked to `width`** (enforced in `Value::new`, the only constructor path).
  - Constructors/derivations: `new`, `with_width` (re-mask to a new width,
    truncating when narrowing), `with_raw` (new bits, same width).
  - Interpretation: `raw()`, `as_unsigned()`, `as_signed()` (two's complement,
    with a dedicated width-128 path).
  - Formatters: `to_hex()` (4-digit groups, e.g. `DEAD_BEEF`), `to_bin()`
    (nibble groups), `to_oct()` (3-digit groups), `to_dec(sign)`. Hex/bin/oct are
    zero-padded to the full width and grouped with `_`; decimal honours signedness
    and is neither padded nor grouped.
  - A private `group(s, n)` helper inserts `_` separators counting from the right.

### Operations (`ops.rs`)

Methods on `Value`. Every result is re-masked to the **left-hand operand's
width**, mirroring fixed-width hardware truncation/wraparound. Binary ops re-mask
the right-hand operand to the left width via a private `rhs_raw` helper.

- **Bitwise:** `and`, `or`, `xor`, `nand`, `nor`, `xnor`, `not` (width-respecting).
- **Arithmetic:** `add`, `sub`, `mul` (all `wrapping_*`), `neg` (two's complement).
- **Division:** `div(rhs, sign) -> Option<Value>`, `rem(rhs, sign) -> Option<Value>`
  — return `None` on divide-by-zero; signed paths use `wrapping_div`/`wrapping_rem`
  so `MIN / -1` wraps instead of panicking.
- **Shifts:** `shl(amount)` (drops bits past width, zero when `amount >= width`);
  `shr(amount, sign)` — logical (zero-fill) when unsigned, arithmetic
  (sign-extend) when signed, saturating to all-zeros/all-sign-bits when
  `amount >= width`.
- **Rotates:** `rotl(amount)`, `rotr(amount)` — wrap within the width
  (`amount % width`).

### Literal parsing (`parse.rs`)

`parse_literal(s) -> Result<u128, ParseError>` parses a single numeric token:

- Prefixes (case-insensitive): `0x` (hex), `0b` (binary), `0o` (octal); otherwise
  decimal. `_` separators are skipped.
- A bare run of hex letters is **not** a number — hex requires the `0x` prefix —
  because the expression layer treats letters as identifiers (e.g. `ans`).
- `ParseError`: `Empty` (no digits / lone prefix), `InvalidDigit(char)`,
  `Overflow` (exceeds `u128`). Implements `Display` + `std::error::Error`.

### Expression evaluation (`expr.rs`)

A self-contained, dependency-free **tokenizer + Pratt parser + evaluator** (an
intentional Rust learning artifact).

- `tokenize(input) -> Result<Vec<Token>, EvalError>` — numbers (delegating to
  `parse_literal`), identifiers, the operators `+ - * / % & | ^ ~ << >>`, and
  parentheses. `<`/`>` must be doubled into a shift or it errors (`LoneAngle`).
- **Precedence** (loosest→tightest), C-like, via `infix_bp`:
  `|`(10) < `^`(20) < `&`(30) < `<< >>`(40) < `+ -`(50) < `* / %`(60) <
  unary `- ~`(binds at 70) < primary. All infix operators are left-associative.
- Primaries: numeric literals, the identifier **`ans`** (case-insensitive → the
  current value), and parenthesised sub-expressions.
- `eval(input, width, sign, ans) -> Result<Value, EvalError>` — masks every
  literal to `width` and applies `sign` to the interpretation-dependent ops.
  Leftover tokens after a complete parse → `UnexpectedToken` (e.g. `1 2`).
- `EvalError` variants: `Parse(ParseError)`, `BadChar`, `LoneAngle`,
  `UnexpectedEof`, `UnexpectedToken`, `UnbalancedParen`, `UnknownIdent`,
  `DivByZero`. Implements `Display` + `Error`, with `From<ParseError>`.

### Fixed-point (`fixed.rs`)

Qm.n interpretation of a `Value` (low `n` bits are the fraction; `real = int / 2^n`).

- `to_real(value, frac_bits, sign) -> f64` — integer interpretation (unsigned or
  signed) divided by `2^frac_bits`.
- `from_real(real, width, frac_bits) -> Value` — multiply by `2^frac_bits`, round
  to nearest representable step, mask to `width` (negatives land on their two's-
  complement pattern).
- Conversion goes through `f64` (53-bit mantissa) — a display/entry convenience;
  the raw `Value` bits remain the source of truth.

## Data architecture

There is no database. The "data model" is the in-memory `Value`/`Width`/
`Signedness` triad described above. See
[API Contracts — Core](./api-contracts-core.md) for exact signatures.

## Testing strategy

Every module ends with a `#[cfg(test)] mod tests`. Coverage by file: `value.rs`
(10), `expr.rs` (8), `ops.rs` (7), `fixed.rs` (6), `parse.rs` (4) — 35 total.
Tests assert hardware-style behaviours directly: masking/truncation, two's-
complement round-trips, signed vs unsigned decimal for identical bits, logical vs
arithmetic shift, rotate wraparound, operator precedence, mixed-base literals,
`ans` substitution, error cases, and fixed-point round-trips. Run with
`cargo test -p powercalc-core` (or `cargo test`).

## Public API surface

Re-exported from `lib.rs`: `Value`, `Width`, `Signedness`, `eval`, `EvalError`,
`parse_literal`, `ParseError`; plus the modules `expr`, `fixed`, `ops`, `parse`,
`value`. Full catalog in [API Contracts — Core](./api-contracts-core.md).
