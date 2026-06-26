# Nybble — Supported expressions

This is the complete reference for what the expression field accepts. It is
derived directly from the evaluators in `crates/core`:

- **Integer mode** — `crates/core/src/expr.rs` (+ numeric methods in `ops.rs`).
  Every value is a width- and sign-bound `Value`; **all results are re-masked to
  the active bit width** (hardware truncation/wrapping).
- **Float mode** — `crates/core/src/float.rs`. Every value is an `f64` at full
  precision regardless of the active width. Out-of-domain calls yield `NaN`,
  and `1/0` yields `inf` (IEEE specials are allowed, never an error).

The two modes share the same surface grammar (literals, `ans`, parentheses,
function-call syntax, the `**` operator) but differ in which operators and
functions are available, as noted per row below.

---

## 1. Literals

| Form | Example | Modes | Notes |
|------|---------|-------|-------|
| Decimal | `42`, `1_000` | both | `_` allowed anywhere as a separator |
| Hex | `0xFF`, `0xDEAD_BEEF` | both | `0x`/`0X` prefix |
| Binary | `0b1010` | both | `0b`/`0B` prefix |
| Octal | `0o17` | both | `0o`/`0O` prefix |
| Decimal fraction | `1.5`, `1_000.5` | float only | |
| Scientific | `1e6`, `1.5e-3` | float only | signed exponent allowed |

In integer mode a literal is masked to the active width immediately (e.g. `0x1FF`
at width 8 becomes `0xFF`). In float mode any integer literal is widened to `f64`.

> Note: a base/constant name must not be glued to a digit — write `2 * pi`, not
> `2pi` (the number scanner would try to read `2pi` as one malformed literal).

---

## 2. Identifiers & constants

| Name | Value | Modes |
|------|-------|-------|
| `ans` | the calculator's current value | both |
| `pi` | π ≈ 3.14159265358979 | float only |
| `e` | Euler's number ≈ 2.71828182845905 | float only |
| `tau` | τ = 2π ≈ 6.28318530717959 | float only |

`ans` is case-insensitive. In integer mode, any other bare identifier is an
`unknown name` error.

---

## 3. Operators

Precedence is listed **loosest → tightest**. All binary operators are
left-associative **except `**`**, which is right-associative and binds as
tightly as the unary prefixes — so `-2 ** 2 == -(2 ** 2)` and
`2 ** 3 ** 2 == 2 ** (3 ** 2) == 512`.

### Integer mode

| Operator | Meaning | Notes |
|----------|---------|-------|
| `\|` | bitwise OR | |
| `^` | bitwise XOR | |
| `&` | bitwise AND | |
| `<<` `>>` | left / right shift | right shift is logical (unsigned) or arithmetic (signed) per the active signedness |
| `+` `-` | add / subtract | wraps at the width |
| `*` `/` `%` | multiply / divide / remainder | `/` and `%` are signedness-aware; divide-by-zero is an error |
| `**` | power (right-assoc) | exponent taken as a raw count; result wraps & masks |
| unary `-` | two's-complement negate | |
| unary `~` | bitwise NOT (within width) | |

### Float mode

| Operator | Meaning | Notes |
|----------|---------|-------|
| `+` `-` | add / subtract | |
| `*` `/` `%` | multiply / divide / remainder | `1/0` → `inf`, `0/0` → `nan` |
| `**` | power (right-assoc) | `2 ** 0.5` ≈ 1.41421356 |
| unary `-` | negate | |

Bitwise and shift operators (`& \| ^ ~ << >>`) are **rejected** in float mode
(`'<' is not available in float mode`).

---

## 4. Functions — integer mode

Function names are case-insensitive. Arguments are separated by commas. Every
result is re-masked to the active width. Signedness-aware functions use the
calculator's current signed/unsigned setting.

| Call | Arity | Description |
|------|-------|-------------|
| `pow(x, y)` | 2 | `x` to the power `y` (same as `x ** y`) |
| `sqrt(x)` | 1 | floor of the integer square root |
| `log2(x)` | 1 | floor of log₂; **error** if `x == 0` |
| `clog2(x)` | 1 | ceiling of log₂ — bits needed to index `x` values; `clog2(0)=clog2(1)=0` |
| `popcount(x)` | 1 | number of set bits |
| `abs(x)` | 1 | absolute value (signedness-aware; no-op when unsigned) |
| `sign(x)` | 1 | `-1` / `0` / `1` (signedness-aware) |
| `fact(x)` | 1 | factorial, wrapped & masked to width |
| `gcd(x, y)` | 2 | greatest common divisor |
| `lcm(x, y)` | 2 | least common multiple (wraps like a product) |
| `min(x, y)` | 2 | smaller value (signedness-aware) |
| `max(x, y)` | 2 | larger value (signedness-aware) |
| `mod(x, y)` | 2 | remainder (signedness-aware; same as `%`) |
| `floor(x)` `ceil(x)` `round(x)` `trunc(x)` | 1 | identity on integers — accepted so a float-mode expression still evaluates |

---

## 5. Functions — float mode

Full `f64` precision. Out-of-domain inputs (e.g. `sqrt(-1)`, `ln(-1)`,
`acos(2)`) return `NaN` rather than erroring.

### Trigonometric (radians)

| Call | Description |
|------|-------------|
| `sin(x)` `cos(x)` `tan(x)` | trig functions, `x` in radians |
| `asin(x)` `acos(x)` `atan(x)` | inverse trig, result in radians |
| `atan2(y, x)` | angle of the point `(x, y)`, in radians |

### Trigonometric (degrees)

| Call | Description |
|------|-------------|
| `sind(x)` `cosd(x)` `tand(x)` | trig functions, `x` in degrees |
| `asind(x)` `acosd(x)` `atand(x)` | inverse trig, result in degrees |

### Hyperbolic

| Call | Description |
|------|-------------|
| `sinh(x)` `cosh(x)` `tanh(x)` | hyperbolic functions |
| `asinh(x)` `acosh(x)` `atanh(x)` | inverse hyperbolic functions |

### Logarithms & exponentials

| Call | Description |
|------|-------------|
| `ln(x)` | natural log (base e) |
| `log10(x)` | base-10 log |
| `log2(x)` | base-2 log |
| `log(x)` | base-10 log (1-argument form) |
| `log(x, base)` | log of `x` in an arbitrary `base` (2-argument form) |
| `exp(x)` | eˣ |
| `exp2(x)` | 2ˣ |

### Powers & roots

| Call | Description |
|------|-------------|
| `sqrt(x)` | square root |
| `cbrt(x)` | cube root |
| `pow(x, y)` | `x` to the power `y` (same as `x ** y`) |
| `root(x, n)` | `n`-th root of `x` (= `x ** (1/n)`) |

### Rounding & sign

| Call | Description |
|------|-------------|
| `abs(x)` | absolute value |
| `floor(x)` `ceil(x)` `round(x)` `trunc(x)` | rounding modes |
| `sign(x)` | `-1.0` / `0.0` / `1.0` (and `NaN` for `NaN`) |

### Combinatoric / two-argument helpers

| Call | Description |
|------|-------------|
| `hypot(a, b)` | `sqrt(a² + b²)` without overflow |
| `min(a, b)` `max(a, b)` | smaller / larger value |
| `mod(a, b)` | remainder (same as `%`) |
| `gcd(a, b)` | greatest common divisor (NaN unless both are finite integers) |
| `lcm(a, b)` | least common multiple |
| `fact(n)` | factorial of a non-negative integer `n` (else `NaN`); large `n` → `inf` |

---

## 6. Errors

Surfaced as a red message under the input. The `EvalError` variants:

| Condition | Message |
|-----------|---------|
| Bad numeric literal | (delegated to the parser, e.g. `invalid digit 'Z'`) |
| Unexpected character | `unexpected character '@'` |
| Lone `<` or `>` | `expected '<<' for a shift` |
| Ran out of input | `unexpected end of expression` |
| Leftover/misplaced token | `unexpected token` |
| Unbalanced parentheses | `unbalanced parentheses` |
| Unknown bare name | `unknown name 'foo'` |
| Unknown function | `unknown function 'foo'` |
| Wrong argument count | `wrong number of arguments to 'pow' (got 1)` |
| Integer-mode domain error | e.g. `log2 of zero is undefined` |
| Divide by zero (integer mode) | `division by zero` |
| Bitwise/shift used in float mode | `'<' is not available in float mode` |

---

## 7. Worked examples

```text
Integer mode (width 32, unsigned):
  0xFF & (1 << 3)   → 8
  log2(4)           → 2
  2 ** 8            → 256
  clog2(1024)       → 10
  sqrt(255)         → 15
  gcd(54, 24)       → 6
  popcount(0xFF)    → 8
  fact(5)           → 120

Float mode:
  sqrt(2)           → 1.4142135623730951
  sin(pi / 2)       → 1
  sind(90)          → 1
  ln(e)             → 1
  log(8, 2)         → 3
  2 ** 0.5          → 1.4142135623730951
  hypot(3, 4)       → 5
  sqrt(-1)          → NaN
```
