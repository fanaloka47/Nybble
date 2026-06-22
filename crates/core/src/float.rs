//! Floating-point evaluation: a full-precision `f64` sibling to [`crate::expr`].
//!
//! This is the calculator's *float mode*. It reuses the same surface grammar as
//! the integer evaluator — `+ - * / %`, unary `-`, parentheses, and the `ans`
//! identifier — but every value is an `f64`, so results keep full floating-point
//! precision regardless of the active [`Width`]. Bitwise and shift operators
//! have no meaning here and are rejected with [`EvalError::BitwiseInFloatMode`].
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
    Slash,
    Percent,
    LParen,
    RParen,
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
        cleaned.parse::<f64>().map_err(|_| {
            EvalError::Parse(ParseError::InvalidDigit(first_bad_float_char(&cleaned)))
        })
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
                let hex = chars[i] == '0' && i + 1 < chars.len() && matches!(chars[i + 1], 'x' | 'X');
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
            '+' => { tokens.push(Token::Plus); i += 1; }
            '-' => { tokens.push(Token::Minus); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '/' => { tokens.push(Token::Slash); i += 1; }
            '%' => { tokens.push(Token::Percent); i += 1; }
            // Bitwise/shift operators are meaningless on floats.
            '&' | '|' | '^' | '~' | '<' | '>' => return Err(EvalError::BitwiseInFloatMode(c)),
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
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
            let rhs = self.expr(bp + 1)?;
            lhs = match op {
                Token::Plus => lhs + rhs,
                Token::Minus => lhs - rhs,
                Token::Star => lhs * rhs,
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
                if name.eq_ignore_ascii_case("ans") {
                    Ok(self.ans)
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
        assert_eq!(eval_float("1.0 & 2", 0.0), Err(EvalError::BitwiseInFloatMode('&')));
        assert_eq!(eval_float("1 << 2", 0.0), Err(EvalError::BitwiseInFloatMode('<')));
        assert_eq!(eval_float("~1", 0.0), Err(EvalError::BitwiseInFloatMode('~')));
    }

    #[test]
    fn errors() {
        assert_eq!(eval_float("", 0.0), Err(EvalError::UnexpectedEof));
        assert_eq!(eval_float("1 +", 0.0), Err(EvalError::UnexpectedEof));
        assert_eq!(eval_float("1 2", 0.0), Err(EvalError::UnexpectedToken));
        assert_eq!(eval_float("(1 + 2", 0.0), Err(EvalError::UnbalancedParen));
        assert!(matches!(eval_float("foo", 0.0), Err(EvalError::UnknownIdent(_))));
    }

    #[test]
    fn f64_bit_pattern() {
        assert_eq!(f64_to_value(1.0).to_hex(), "3FF0_0000_0000_0000");
        assert_eq!(f64_to_value(0.0).to_hex(), "0000_0000_0000_0000");
    }
}
