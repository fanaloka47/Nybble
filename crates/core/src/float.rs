//! Floating-point evaluation: a full-precision `f64` sibling to [`crate::expr`].
//!
//! This is the calculator's *float mode*. It reuses the same surface grammar as
//! the integer evaluator — `+ - * / %`, the `**` power operator, unary `-`,
//! parentheses, and the `ans` identifier — but every value is an `f64`, so
//! results keep full floating-point precision regardless of the active
//! [`Width`]. Bitwise and shift operators have no meaning here and are rejected
//! with [`EvalError::BitwiseInFloatMode`].
//!
//! Float mode also provides the scientific function set — trigonometry
//! (`sin`/`cos`/`tan` in radians, `sind`/`cosd`/`tand` in degrees, plus inverse
//! and hyperbolic variants), logarithms (`ln`, `log2`, `log10`, `log(x, base)`),
//! `exp`/`sqrt`/`cbrt`/`pow`/`root`, rounding (`floor`/`ceil`/`round`/`trunc`),
//! `abs`/`sign`/`hypot`/`min`/`max`/`gcd`/`lcm`/`mod`/`fact`, and the constants
//! `pi`, `e`, and `tau` (see [`call_func`]). Out-of-domain calls yield NaN.
//!
//! Numeric literals accept decimals and scientific notation (`1.5`, `1e6`,
//! `1.5e-3`) as well as plain integer literals in any base (`8`, `0xFF`,
//! `0b1010`), which are widened to `f64`. IEEE specials are allowed: `1/0`
//! yields `inf` rather than an error.

use crate::expr::EvalError;
use crate::parse::{parse_literal, ParseError};
use crate::value::{Value, Width};

/// Reinterpret an `f64`'s IEEE 754 bits as a 64-bit [`Value`] for hex/bin/oct
/// rendering. The integer width in use elsewhere does not apply: a float is
/// always shown through its `f64` encoding.
pub fn f64_to_value(x: f64) -> Value {
    Value::new(x.to_bits() as u128, Width::new(64).unwrap())
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    Percent,
    LParen,
    RParen,
    Comma,
}

/// The first character that can't appear in a decimal float literal, used to
/// build a precise parse error.
fn first_bad_float_char(s: &str) -> char {
    s.chars()
        .find(|c| !matches!(c, '0'..='9' | '.' | 'e' | 'E' | '+' | '-'))
        .unwrap_or('?')
}

/// Parse one numeric token to `f64`. Base-prefixed (`0x`/`0b`/`0o`) and plain
/// integer tokens go through [`parse_literal`]; anything with a `.` or exponent
/// parses as a decimal float.
fn parse_number(text: &str) -> Result<f64, EvalError> {
    let is_prefixed = {
        let mut c = text.chars();
        c.next() == Some('0') && matches!(c.next(), Some('x' | 'X' | 'b' | 'B' | 'o' | 'O'))
    };
    if is_prefixed {
        return Ok(parse_literal(text)? as f64);
    }
    if text.contains(['.', 'e', 'E']) {
        let cleaned: String = text.chars().filter(|&c| c != '_').collect();
        cleaned
            .parse::<f64>()
            .map_err(|_| EvalError::Parse(ParseError::InvalidDigit(first_bad_float_char(&cleaned))))
    } else {
        Ok(parse_literal(text)? as f64)
    }
}

fn tokenize(input: &str) -> Result<Vec<Token>, EvalError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            // A number: digits, '.', '_', base letters, plus a signed exponent.
            c if c.is_ascii_digit() || c == '.' => {
                let start = i;
                let hex =
                    chars[i] == '0' && i + 1 < chars.len() && matches!(chars[i + 1], 'x' | 'X');
                while i < chars.len() {
                    let d = chars[i];
                    if d.is_ascii_alphanumeric() || d == '_' || d == '.' {
                        i += 1;
                    } else if (d == '+' || d == '-') && !hex && matches!(chars[i - 1], 'e' | 'E') {
                        // Exponent sign, e.g. the '-' in `1.5e-3`.
                        i += 1;
                    } else {
                        break;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Num(parse_number(&text)?));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            // `**` is the power operator; a lone `*` is multiplication.
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    tokens.push(Token::StarStar);
                    i += 2;
                } else {
                    tokens.push(Token::Star);
                    i += 1;
                }
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            // Bitwise/shift operators are meaningless on floats.
            '&' | '|' | '^' | '~' | '<' | '>' => return Err(EvalError::BitwiseInFloatMode(c)),
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            other => return Err(EvalError::BadChar(other)),
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    ans: f64,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Left binding power for an infix operator. Higher binds tighter.
    fn infix_bp(tok: &Token) -> Option<u8> {
        Some(match tok {
            Token::Plus | Token::Minus => 10,
            Token::Star | Token::Slash | Token::Percent => 20,
            // `**` binds tighter than `* /` and as tightly as unary minus, and
            // is right-associative (handled in `expr`).
            Token::StarStar => 30,
            _ => return None,
        })
    }

    fn expr(&mut self, min_bp: u8) -> Result<f64, EvalError> {
        let mut lhs = self.prefix()?;
        loop {
            let Some(bp) = self.peek().and_then(Self::infix_bp) else {
                break;
            };
            if bp < min_bp {
                break;
            }
            let op = self.next().unwrap();
            // `**` is right-associative (recurse at `bp`); the rest are
            // left-associative (recurse at `bp + 1`).
            let next_bp = if op == Token::StarStar { bp } else { bp + 1 };
            let rhs = self.expr(next_bp)?;
            lhs = match op {
                Token::Plus => lhs + rhs,
                Token::Minus => lhs - rhs,
                Token::Star => lhs * rhs,
                Token::StarStar => lhs.powf(rhs),
                Token::Slash => lhs / rhs,
                Token::Percent => lhs % rhs,
                _ => return Err(EvalError::UnexpectedToken),
            };
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<f64, EvalError> {
        match self.next().ok_or(EvalError::UnexpectedEof)? {
            Token::Num(n) => Ok(n),
            Token::Ident(name) => {
                if self.peek() == Some(&Token::LParen) {
                    self.call(&name)
                } else if name.eq_ignore_ascii_case("ans") {
                    Ok(self.ans)
                } else if let Some(c) = constant(&name) {
                    Ok(c)
                } else {
                    Err(EvalError::UnknownIdent(name))
                }
            }
            // Unary minus binds tighter than any infix operator.
            Token::Minus => Ok(-self.expr(30)?),
            Token::LParen => {
                let inner = self.expr(0)?;
                match self.next() {
                    Some(Token::RParen) => Ok(inner),
                    _ => Err(EvalError::UnbalancedParen),
                }
            }
            _ => Err(EvalError::UnexpectedToken),
        }
    }

    /// Parse a function call. The current token is the opening `(`; consume the
    /// comma-separated argument list and the closing `)`, then dispatch by name.
    fn call(&mut self, name: &str) -> Result<f64, EvalError> {
        self.next(); // the '(' confirmed by the caller
        let mut args = Vec::new();
        if self.peek() != Some(&Token::RParen) {
            loop {
                args.push(self.expr(0)?);
                match self.peek() {
                    Some(Token::Comma) => {
                        self.next();
                    }
                    _ => break,
                }
            }
        }
        match self.next() {
            Some(Token::RParen) => {}
            _ => return Err(EvalError::UnbalancedParen),
        }
        call_func(name, &args)
    }
}

/// Resolve a bare identifier to a mathematical constant, if it names one.
fn constant(name: &str) -> Option<f64> {
    match name.to_ascii_lowercase().as_str() {
        "pi" => Some(std::f64::consts::PI),
        "e" => Some(std::f64::consts::E),
        "tau" => Some(std::f64::consts::TAU),
        _ => None,
    }
}

/// Apply a named scientific function to its evaluated arguments. Domain errors
/// (e.g. `sqrt(-1)`, `ln(-1)`) fall out as NaN, matching float mode's policy of
/// allowing IEEE specials rather than erroring.
fn call_func(name: &str, args: &[f64]) -> Result<f64, EvalError> {
    let lower = name.to_ascii_lowercase();
    let arity = |n: usize| -> Result<(), EvalError> {
        if args.len() == n {
            Ok(())
        } else {
            Err(EvalError::ArgCount {
                func: lower.clone(),
                got: args.len(),
            })
        }
    };
    // One-argument function: check arity, then apply `f` to the single argument.
    let unary = |f: fn(f64) -> f64| -> Result<f64, EvalError> {
        arity(1)?;
        Ok(f(args[0]))
    };
    // Radians per degree, inlined so the degree closures stay non-capturing
    // (and therefore coercible to `fn` pointers).
    const DEG: f64 = std::f64::consts::PI / 180.0;

    match lower.as_str() {
        // Trigonometric (radians).
        "sin" => unary(f64::sin),
        "cos" => unary(f64::cos),
        "tan" => unary(f64::tan),
        "asin" => unary(f64::asin),
        "acos" => unary(f64::acos),
        "atan" => unary(f64::atan),
        // Trigonometric (degrees).
        "sind" => unary(|x| (x * DEG).sin()),
        "cosd" => unary(|x| (x * DEG).cos()),
        "tand" => unary(|x| (x * DEG).tan()),
        "asind" => unary(|x| x.asin() / DEG),
        "acosd" => unary(|x| x.acos() / DEG),
        "atand" => unary(|x| x.atan() / DEG),
        "atan2" => {
            arity(2)?;
            Ok(args[0].atan2(args[1]))
        }
        // Hyperbolic.
        "sinh" => unary(f64::sinh),
        "cosh" => unary(f64::cosh),
        "tanh" => unary(f64::tanh),
        "asinh" => unary(f64::asinh),
        "acosh" => unary(f64::acosh),
        "atanh" => unary(f64::atanh),
        // Logarithms and exponentials.
        "ln" => unary(f64::ln),
        "log10" => unary(f64::log10),
        "log2" => unary(f64::log2),
        // `log(x)` is base 10; `log(x, base)` is an arbitrary base.
        "log" => match args.len() {
            1 => Ok(args[0].log10()),
            2 => Ok(args[0].log(args[1])),
            _ => Err(EvalError::ArgCount {
                func: lower,
                got: args.len(),
            }),
        },
        "exp" => unary(f64::exp),
        "exp2" => unary(f64::exp2),
        // Powers and roots.
        "sqrt" => unary(f64::sqrt),
        "cbrt" => unary(f64::cbrt),
        "pow" => {
            arity(2)?;
            Ok(args[0].powf(args[1]))
        }
        "root" => {
            arity(2)?;
            Ok(args[0].powf(1.0 / args[1]))
        }
        // Rounding and sign.
        "abs" => unary(f64::abs),
        "floor" => unary(f64::floor),
        "ceil" => unary(f64::ceil),
        "round" => unary(f64::round),
        "trunc" => unary(f64::trunc),
        "sign" => unary(f64::signum),
        // Combinatoric / two-argument helpers.
        "hypot" => {
            arity(2)?;
            Ok(args[0].hypot(args[1]))
        }
        "min" => {
            arity(2)?;
            Ok(args[0].min(args[1]))
        }
        "max" => {
            arity(2)?;
            Ok(args[0].max(args[1]))
        }
        "mod" => {
            arity(2)?;
            Ok(args[0] % args[1])
        }
        "gcd" => {
            arity(2)?;
            Ok(gcd_f64(args[0], args[1]))
        }
        "lcm" => {
            arity(2)?;
            let g = gcd_f64(args[0], args[1]);
            Ok(if g == 0.0 {
                0.0
            } else {
                (args[0] / g * args[1]).abs()
            })
        }
        "fact" => unary(factorial_f64),
        _ => Err(EvalError::UnknownFunction(lower)),
    }
}

/// Greatest common divisor of two reals, rounded to integers. Returns NaN if
/// either input isn't a finite integer value.
fn gcd_f64(a: f64, b: f64) -> f64 {
    if a.fract() != 0.0 || b.fract() != 0.0 || !a.is_finite() || !b.is_finite() {
        return f64::NAN;
    }
    let mut x = a.abs();
    let mut y = b.abs();
    while y != 0.0 {
        let t = x % y;
        x = y;
        y = t;
    }
    x
}

/// Factorial for a non-negative integer argument; NaN otherwise. Large inputs
/// overflow to `inf` naturally.
fn factorial_f64(n: f64) -> f64 {
    if n.fract() != 0.0 || n < 0.0 || !n.is_finite() {
        return f64::NAN;
    }
    let mut acc = 1.0f64;
    let mut i = 2.0f64;
    while i <= n {
        acc *= i;
        i += 1.0;
    }
    acc
}

/// Evaluate `input` as a full-precision `f64` expression, using `ans` as the
/// value of the `ans` identifier.
pub fn eval_float(input: &str, ans: f64) -> Result<f64, EvalError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(EvalError::UnexpectedEof);
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        ans,
    };
    let value = parser.expr(0)?;
    if parser.pos != tokens.len() {
        return Err(EvalError::UnexpectedToken);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_precedence() {
        assert_eq!(eval_float("2 + 3 * 4", 0.0).unwrap(), 14.0);
        assert_eq!(eval_float("(2 + 3) * 4", 0.0).unwrap(), 20.0);
        // A full bitrate-style calculation lands exactly on a dyadic value.
        assert_eq!(eval_float("1500e6 * 8 / 1.024e6", 0.0).unwrap(), 11718.75);
    }

    #[test]
    fn float_literals() {
        assert_eq!(eval_float("1.5", 0.0).unwrap(), 1.5);
        assert_eq!(eval_float("1e6", 0.0).unwrap(), 1_000_000.0);
        assert_eq!(eval_float("1.5e-3", 0.0).unwrap(), 0.0015);
        assert_eq!(eval_float("1_000.5", 0.0).unwrap(), 1000.5);
    }

    #[test]
    fn integer_literals_widen() {
        assert_eq!(eval_float("8 * 2.0", 0.0).unwrap(), 16.0);
        assert_eq!(eval_float("0xFF", 0.0).unwrap(), 255.0);
        assert_eq!(eval_float("0b1010", 0.0).unwrap(), 10.0);
    }

    #[test]
    fn unary_minus_and_ans() {
        assert_eq!(eval_float("-2.5", 0.0).unwrap(), -2.5);
        assert_eq!(eval_float("ans + 1", 10.0).unwrap(), 11.0);
        assert_eq!(eval_float("ans * 2", 1.5).unwrap(), 3.0);
    }

    #[test]
    fn division_by_zero_is_infinity() {
        assert!(eval_float("1.0 / 0.0", 0.0).unwrap().is_infinite());
        assert!(eval_float("0.0 / 0.0", 0.0).unwrap().is_nan());
    }

    #[test]
    fn bitwise_rejected() {
        assert_eq!(
            eval_float("1.0 & 2", 0.0),
            Err(EvalError::BitwiseInFloatMode('&'))
        );
        assert_eq!(
            eval_float("1 << 2", 0.0),
            Err(EvalError::BitwiseInFloatMode('<'))
        );
        assert_eq!(
            eval_float("~1", 0.0),
            Err(EvalError::BitwiseInFloatMode('~'))
        );
    }

    #[test]
    fn errors() {
        assert_eq!(eval_float("", 0.0), Err(EvalError::UnexpectedEof));
        assert_eq!(eval_float("1 +", 0.0), Err(EvalError::UnexpectedEof));
        assert_eq!(eval_float("1 2", 0.0), Err(EvalError::UnexpectedToken));
        assert_eq!(eval_float("(1 + 2", 0.0), Err(EvalError::UnbalancedParen));
        assert!(matches!(
            eval_float("foo", 0.0),
            Err(EvalError::UnknownIdent(_))
        ));
    }

    #[test]
    fn f64_bit_pattern() {
        assert_eq!(f64_to_value(1.0).to_hex(), "3FF0_0000_0000_0000");
        assert_eq!(f64_to_value(0.0).to_hex(), "0000_0000_0000_0000");
    }

    fn ev(s: &str) -> f64 {
        eval_float(s, 0.0).unwrap()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn power_operator() {
        assert_eq!(ev("2 ** 10"), 1024.0);
        assert!(approx(ev("2 ** 0.5"), std::f64::consts::SQRT_2));
        // right-associative and tighter than * and unary minus.
        assert_eq!(ev("2 ** 3 ** 2"), 512.0);
        assert_eq!(ev("2 * 3 ** 2"), 18.0);
        assert_eq!(ev("-2 ** 2"), -4.0);
        assert_eq!(ev("2 ** -1"), 0.5);
    }

    #[test]
    fn constants() {
        assert_eq!(ev("pi"), std::f64::consts::PI);
        assert_eq!(ev("e"), std::f64::consts::E);
        assert_eq!(ev("tau"), std::f64::consts::TAU);
        assert!(approx(ev("2 * pi"), std::f64::consts::TAU));
    }

    #[test]
    fn scientific_functions() {
        assert!(approx(ev("sqrt(2)"), std::f64::consts::SQRT_2));
        assert!(approx(ev("log2(1024)"), 10.0));
        assert!(approx(ev("log10(1000)"), 3.0));
        assert!(approx(ev("ln(e)"), 1.0));
        assert!(approx(ev("log(8, 2)"), 3.0));
        assert!(approx(ev("sin(pi / 2)"), 1.0));
        assert!(approx(ev("sind(90)"), 1.0));
        assert!(approx(ev("cosd(0)"), 1.0));
        assert!(approx(ev("asind(1)"), 90.0));
        assert!(approx(ev("atan2(1, 1)"), std::f64::consts::FRAC_PI_4));
        assert!(approx(ev("pow(2, 8)"), 256.0));
        assert!(approx(ev("cbrt(27)"), 3.0));
        assert!(approx(ev("root(27, 3)"), 3.0));
        assert!(approx(ev("hypot(3, 4)"), 5.0));
        assert!(approx(ev("fact(5)"), 120.0));
        assert!(approx(ev("gcd(54, 24)"), 6.0));
        assert!(approx(ev("lcm(4, 6)"), 12.0));
        assert!(approx(ev("min(3, 9)"), 3.0));
        assert!(approx(ev("max(3, 9)"), 9.0));
        assert!(approx(ev("floor(2.7)"), 2.0));
        assert!(approx(ev("ceil(2.1)"), 3.0));
        assert!(approx(ev("abs(-2.5)"), 2.5));
    }

    #[test]
    fn domain_errors_are_nan() {
        assert!(eval_float("sqrt(-1)", 0.0).unwrap().is_nan());
        assert!(eval_float("ln(-1)", 0.0).unwrap().is_nan());
        assert!(eval_float("fact(-1)", 0.0).unwrap().is_nan());
        assert!(eval_float("fact(2.5)", 0.0).unwrap().is_nan());
    }

    #[test]
    fn function_errors() {
        assert!(matches!(
            eval_float("foo(1)", 0.0),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(matches!(
            eval_float("pow(1)", 0.0),
            Err(EvalError::ArgCount { .. })
        ));
        assert!(matches!(
            eval_float("sin(1, 2)", 0.0),
            Err(EvalError::ArgCount { .. })
        ));
    }
}
