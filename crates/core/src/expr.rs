//! Expression evaluation: tokenizer + Pratt parser + evaluator.
//!
//! Grammar (C-like precedence, loosest to tightest):
//! `|` < `^` < `&` < `<< >>` < `+ -` < `* / %` < `**` (right-assoc) ≈ unary `- ~`
//! < primary.
//! Primaries are numeric literals (see [`crate::parse`]), the identifier `ans`
//! (the current value), parenthesised sub-expressions, and named function calls
//! such as `sqrt(255)`, `log2(1024)`, `clog2(1000)`, or `gcd(54, 24)` (see
//! [`Parser::dispatch`] for the full set).
//!
//! Evaluation is width- and sign-aware: every literal is masked to the active
//! [`Width`], and the two interpretation-dependent operations — right shift and
//! division/remainder — use the active [`Signedness`].

use crate::parse::{parse_literal, ParseError};
use crate::value::{Signedness, Value, Width};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A bad numeric literal.
    Parse(ParseError),
    /// An unexpected character while tokenizing.
    BadChar(char),
    /// `<` or `>` not paired into a shift operator.
    LoneAngle(char),
    /// Parser hit the end of input while expecting more.
    UnexpectedEof,
    /// A token appeared where it wasn't expected.
    UnexpectedToken,
    /// Unbalanced parentheses.
    UnbalancedParen,
    /// An identifier other than `ans` (or a known constant).
    UnknownIdent(String),
    /// A function name that isn't recognised, e.g. `foo(1)`.
    UnknownFunction(String),
    /// A known function called with the wrong number of arguments.
    ArgCount { func: String, got: usize },
    /// A function argument outside its domain in integer mode, e.g. `log2(0)`.
    /// (Float mode returns NaN instead, mirroring its IEEE-specials policy.)
    DomainError(String),
    /// Division or remainder by zero.
    DivByZero,
    /// A bitwise/shift operator was used in float mode, where it has no meaning.
    BitwiseInFloatMode(char),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::Parse(e) => write!(f, "{e}"),
            EvalError::BadChar(c) => write!(f, "unexpected character '{c}'"),
            EvalError::LoneAngle(c) => write!(f, "expected '{c}{c}' for a shift"),
            EvalError::UnexpectedEof => write!(f, "unexpected end of expression"),
            EvalError::UnexpectedToken => write!(f, "unexpected token"),
            EvalError::UnbalancedParen => write!(f, "unbalanced parentheses"),
            EvalError::UnknownIdent(s) => write!(f, "unknown name '{s}'"),
            EvalError::UnknownFunction(s) => write!(f, "unknown function '{s}'"),
            EvalError::ArgCount { func, got } => {
                write!(f, "wrong number of arguments to '{func}' (got {got})")
            }
            EvalError::DomainError(s) => write!(f, "{s}"),
            EvalError::DivByZero => write!(f, "division by zero"),
            EvalError::BitwiseInFloatMode(c) => {
                write!(f, "'{c}' is not available in float mode")
            }
        }
    }
}

impl std::error::Error for EvalError {}

impl From<ParseError> for EvalError {
    fn from(e: ParseError) -> Self {
        EvalError::Parse(e)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Num(u128),
    Ident(String),
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    Percent,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Shl,
    Shr,
    LParen,
    RParen,
    Comma,
}

fn tokenize(input: &str) -> Result<Vec<Token>, EvalError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            c if c.is_whitespace() => i += 1,
            // A number starts with a digit; consume following alphanumerics and
            // underscores (this captures `0xDEAD`, `0b10`, `1_000`, etc).
            c if c.is_ascii_digit() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Num(parse_literal(&text)?));
            }
            // An identifier starts with a letter (e.g. `ans`).
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Ident(text));
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
            '&' => {
                tokens.push(Token::Amp);
                i += 1;
            }
            '|' => {
                tokens.push(Token::Pipe);
                i += 1;
            }
            '^' => {
                tokens.push(Token::Caret);
                i += 1;
            }
            '~' => {
                tokens.push(Token::Tilde);
                i += 1;
            }
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
            '<' | '>' => {
                if i + 1 < chars.len() && chars[i + 1] == c {
                    tokens.push(if c == '<' { Token::Shl } else { Token::Shr });
                    i += 2;
                } else {
                    return Err(EvalError::LoneAngle(c));
                }
            }
            other => return Err(EvalError::BadChar(other)),
        }
    }
    Ok(tokens)
}

/// Left binding power for an infix operator, or `None` if the token isn't one.
/// Higher binds tighter. C-like ordering.
fn infix_bp(tok: &Token) -> Option<u8> {
    Some(match tok {
        Token::Pipe => 10,
        Token::Caret => 20,
        Token::Amp => 30,
        Token::Shl | Token::Shr => 40,
        Token::Plus | Token::Minus => 50,
        Token::Star | Token::Slash | Token::Percent => 60,
        // `**` binds tighter than `* /` and as tightly as the unary prefixes,
        // and is right-associative (handled in `expr`).
        Token::StarStar => 70,
        _ => return None,
    })
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    width: Width,
    sign: Signedness,
    ans: Value,
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

    fn lit(&self, raw: u128) -> Value {
        Value::new(raw, self.width)
    }

    /// Pratt loop: parse an expression whose operators bind at least `min_bp`.
    fn expr(&mut self, min_bp: u8) -> Result<Value, EvalError> {
        let mut lhs = self.prefix()?;

        while let Some(bp) = self.peek().and_then(infix_bp) {
            if bp < min_bp {
                break;
            }
            let op = self.next().unwrap();
            // `**` is right-associative (recurse at `bp`); everything else is
            // left-associative (recurse at `bp + 1`).
            let next_bp = if op == Token::StarStar { bp } else { bp + 1 };
            let rhs = self.expr(next_bp)?;
            lhs = self.apply_infix(&op, lhs, rhs)?;
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Value, EvalError> {
        match self.next().ok_or(EvalError::UnexpectedEof)? {
            Token::Num(n) => Ok(self.lit(n)),
            Token::Ident(name) => {
                if self.peek() == Some(&Token::LParen) {
                    self.call(&name)
                } else if name.eq_ignore_ascii_case("ans") {
                    Ok(Value::new(self.ans.raw(), self.width))
                } else {
                    Err(EvalError::UnknownIdent(name))
                }
            }
            Token::Minus => Ok(-self.expr(70)?),
            Token::Tilde => Ok(!self.expr(70)?),
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

    fn apply_infix(&self, op: &Token, lhs: Value, rhs: Value) -> Result<Value, EvalError> {
        Ok(match op {
            Token::Plus => lhs + rhs,
            Token::Minus => lhs - rhs,
            Token::Star => lhs * rhs,
            Token::StarStar => lhs.pow(rhs),
            Token::Slash => lhs.div(rhs, self.sign).ok_or(EvalError::DivByZero)?,
            Token::Percent => lhs.rem(rhs, self.sign).ok_or(EvalError::DivByZero)?,
            Token::Amp => lhs.and(rhs),
            Token::Pipe => lhs.or(rhs),
            Token::Caret => lhs.xor(rhs),
            // Shift amount is the rhs interpreted as a plain count.
            Token::Shl => lhs << (rhs.raw() as u32),
            Token::Shr => lhs.shr(rhs.raw() as u32, self.sign),
            _ => return Err(EvalError::UnexpectedToken),
        })
    }

    /// Parse a function call. The current token is the opening `(`; consume the
    /// comma-separated argument list and the closing `)`, then dispatch by name.
    fn call(&mut self, name: &str) -> Result<Value, EvalError> {
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
        self.dispatch(name, &args)
    }

    /// Apply a named integer function to its evaluated arguments. Width/sign are
    /// taken from the parser; results are re-masked to width like every op.
    fn dispatch(&self, name: &str, args: &[Value]) -> Result<Value, EvalError> {
        let lower = name.to_ascii_lowercase();
        let arg_count = |n: usize| -> Result<(), EvalError> {
            if args.len() == n {
                Ok(())
            } else {
                Err(EvalError::ArgCount {
                    func: lower.clone(),
                    got: args.len(),
                })
            }
        };
        let unary = |f: fn(Value) -> Value| -> Result<Value, EvalError> {
            arg_count(1)?;
            Ok(f(args[0]))
        };

        match lower.as_str() {
            "pow" => {
                arg_count(2)?;
                Ok(args[0].pow(args[1]))
            }
            "sqrt" => unary(Value::isqrt),
            "log2" => {
                arg_count(1)?;
                args[0]
                    .ilog2()
                    .ok_or_else(|| EvalError::DomainError("log2 of zero is undefined".into()))
            }
            "clog2" => unary(Value::clog2),
            "popcount" => unary(Value::popcount),
            "abs" => {
                arg_count(1)?;
                Ok(args[0].abs(self.sign))
            }
            "sign" => {
                arg_count(1)?;
                Ok(args[0].signum(self.sign))
            }
            "fact" => unary(Value::factorial),
            "gcd" => {
                arg_count(2)?;
                Ok(args[0].gcd(args[1]))
            }
            "lcm" => {
                arg_count(2)?;
                Ok(args[0].lcm(args[1]))
            }
            "min" => {
                arg_count(2)?;
                Ok(args[0].min(args[1], self.sign))
            }
            "max" => {
                arg_count(2)?;
                Ok(args[0].max(args[1], self.sign))
            }
            "mod" => {
                arg_count(2)?;
                args[0].rem(args[1], self.sign).ok_or(EvalError::DivByZero)
            }
            // Rounding helpers are identities on integers; accept them so an
            // expression carried over from float mode still evaluates.
            "floor" | "ceil" | "round" | "trunc" => unary(|v| v),
            _ => Err(EvalError::UnknownFunction(lower)),
        }
    }
}

/// Evaluate `input` to a [`Value`] of the given width, using `sign` for
/// decimal/shift interpretation and `ans` as the value of the `ans` identifier.
pub fn eval(input: &str, width: Width, sign: Signedness, ans: Value) -> Result<Value, EvalError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(EvalError::UnexpectedEof);
    }
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        width,
        sign,
        ans,
    };
    let value = parser.expr(0)?;
    if parser.pos != tokens.len() {
        // Leftover tokens mean a malformed expression (e.g. "1 2").
        return Err(EvalError::UnexpectedToken);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval_float;

    fn w(bits: u32) -> Width {
        Width::new(bits).unwrap()
    }

    fn eval32(input: &str) -> Result<u128, EvalError> {
        eval(input, w(32), Signedness::Unsigned, Value::new(0, w(32))).map(|v| v.raw())
    }

    #[test]
    fn literals_and_bases() {
        assert_eq!(eval32("0xFF").unwrap(), 255);
        assert_eq!(eval32("0b1010").unwrap(), 10);
        assert_eq!(eval32("0o17").unwrap(), 15);
        assert_eq!(eval32("42").unwrap(), 42);
    }

    #[test]
    fn precedence() {
        // * binds tighter than +.
        assert_eq!(eval32("2 + 3 * 4").unwrap(), 14);
        // shift binds looser than +, tighter than &.
        assert_eq!(eval32("1 + 1 << 2").unwrap(), 8); // (1+1)<<2
        assert_eq!(eval32("0xF0 & 0x0F | 0x01").unwrap(), 0x01); // (&) before (|)
        assert_eq!(eval32("1 ^ 1 & 0").unwrap(), 1); // & before ^
    }

    #[test]
    fn parentheses_override() {
        assert_eq!(eval32("(2 + 3) * 4").unwrap(), 20);
        assert_eq!(eval32("0xFF & (1 << 3)").unwrap(), 0x08);
    }

    #[test]
    fn unary_ops() {
        // ~0 in 32 bits is 0xFFFF_FFFF.
        assert_eq!(eval32("~0").unwrap(), 0xFFFF_FFFF);
        // -1 in 32 bits is 0xFFFF_FFFF.
        assert_eq!(eval32("-1").unwrap(), 0xFFFF_FFFF);
        assert_eq!(eval32("~0xFF & 0xFF").unwrap(), 0x00);
    }

    #[test]
    fn ans_substitution() {
        let ans = Value::new(0x10, w(32));
        let r = eval("ans + 1", w(32), Signedness::Unsigned, ans).unwrap();
        assert_eq!(r.raw(), 0x11);
        let r = eval("ans << 4", w(32), Signedness::Unsigned, ans).unwrap();
        assert_eq!(r.raw(), 0x100);
    }

    #[test]
    fn width_masks_result() {
        // 0xFF + 1 in an 8-bit width wraps to 0.
        let r = eval("0xFF + 1", w(8), Signedness::Unsigned, Value::new(0, w(8))).unwrap();
        assert_eq!(r.raw(), 0x00);
    }

    #[test]
    fn signed_shift_in_expr() {
        // 0x80 >> 1 arithmetic (signed) sign-extends to 0xC0 at width 8.
        let r = eval("0x80 >> 1", w(8), Signedness::Signed, Value::new(0, w(8))).unwrap();
        assert_eq!(r.raw(), 0xC0);
        // Same expression unsigned is a logical shift -> 0x40.
        let r = eval("0x80 >> 1", w(8), Signedness::Unsigned, Value::new(0, w(8))).unwrap();
        assert_eq!(r.raw(), 0x40);
    }

    #[test]
    fn errors() {
        assert_eq!(eval32("1 / 0"), Err(EvalError::DivByZero));
        assert_eq!(eval32(""), Err(EvalError::UnexpectedEof));
        assert_eq!(eval32("1 +"), Err(EvalError::UnexpectedEof));
        assert_eq!(eval32("1 2"), Err(EvalError::UnexpectedToken));
        assert_eq!(eval32("(1 + 2"), Err(EvalError::UnbalancedParen));
        assert!(matches!(eval32("foo"), Err(EvalError::UnknownIdent(_))));
        assert_eq!(eval32("1 < 2"), Err(EvalError::LoneAngle('<')));
        assert!(matches!(eval32("1 @ 2"), Err(EvalError::BadChar('@'))));
        assert!(matches!(eval32("0xZZ"), Err(EvalError::Parse(_))));
    }

    #[test]
    fn power_operator() {
        assert_eq!(eval32("2 ** 8").unwrap(), 256);
        assert_eq!(eval32("2 ** 10").unwrap(), 1024);
        // ** binds tighter than * and is right-associative.
        assert_eq!(eval32("2 * 3 ** 2").unwrap(), 18);
        assert_eq!(eval32("2 ** 3 ** 2").unwrap(), 512);
        // ** binds tighter than unary minus: -(2**2) = -4 -> 0xFFFF_FFFC.
        assert_eq!(eval32("-2 ** 2").unwrap(), 0xFFFF_FFFC);
        // exponent can carry its own unary minus.
        assert_eq!(eval32("2 ** -1").unwrap(), 0); // 2^(4294967295) wraps; just ensure it parses
    }

    #[test]
    fn power_masks_to_width() {
        // 2**8 = 256 truncates to 0 at width 8.
        let r = eval("2 ** 8", w(8), Signedness::Unsigned, Value::new(0, w(8))).unwrap();
        assert_eq!(r.raw(), 0);
    }

    #[test]
    fn named_functions() {
        assert_eq!(eval32("sqrt(255)").unwrap(), 15);
        assert_eq!(eval32("sqrt(256)").unwrap(), 16);
        assert_eq!(eval32("log2(1024)").unwrap(), 10);
        assert_eq!(eval32("clog2(1000)").unwrap(), 10);
        assert_eq!(eval32("popcount(0xFF)").unwrap(), 8);
        assert_eq!(eval32("gcd(54, 24)").unwrap(), 6);
        assert_eq!(eval32("lcm(4, 6)").unwrap(), 12);
        assert_eq!(eval32("min(3, 9)").unwrap(), 3);
        assert_eq!(eval32("max(3, 9)").unwrap(), 9);
        assert_eq!(eval32("mod(17, 5)").unwrap(), 2);
        assert_eq!(eval32("fact(5)").unwrap(), 120);
        assert_eq!(eval32("pow(2, 8)").unwrap(), 256);
        // functions compose with operators and ans.
        assert_eq!(eval32("log2(1 << 10)").unwrap(), 10);
    }

    #[test]
    fn signed_functions_use_sign() {
        // abs(-1) with a signed 8-bit width is 1.
        let signed8 =
            |s: &str| eval(s, w(8), Signedness::Signed, Value::new(0, w(8))).map(|v| v.raw());
        assert_eq!(signed8("abs(-1)").unwrap(), 1);
        assert_eq!(signed8("sign(-5)").unwrap(), 0xFF); // -1
        assert_eq!(signed8("min(-1, 1)").unwrap(), 0xFF); // -1 is smaller
    }

    #[test]
    fn function_errors() {
        assert!(matches!(
            eval32("foo(1)"),
            Err(EvalError::UnknownFunction(_))
        ));
        assert!(matches!(eval32("pow(1)"), Err(EvalError::ArgCount { .. })));
        assert!(matches!(
            eval32("sqrt(1, 2)"),
            Err(EvalError::ArgCount { .. })
        ));
        assert!(matches!(eval32("log2(0)"), Err(EvalError::DomainError(_))));
        // a stray comma outside a call is malformed.
        assert_eq!(eval32("1, 2"), Err(EvalError::UnexpectedToken));
    }

    // --- Bitwise operations -----------------------------------------------

    #[test]
    fn bitwise_and() {
        assert_eq!(eval32("0xFF & 0x0F").unwrap(), 0x0F);
        assert_eq!(eval32("0xAA & 0x55").unwrap(), 0x00);
        assert_eq!(eval32("0xFFFF & 0xFFFF").unwrap(), 0xFFFF);
        // AND with zero is zero.
        assert_eq!(eval32("0x12345678 & 0").unwrap(), 0);
    }

    #[test]
    fn bitwise_or() {
        assert_eq!(eval32("0x0F | 0xF0").unwrap(), 0xFF);
        assert_eq!(eval32("0xAA | 0x55").unwrap(), 0xFF);
        // OR with zero is identity.
        assert_eq!(eval32("0x12345678 | 0").unwrap(), 0x12345678);
    }

    #[test]
    fn bitwise_xor() {
        assert_eq!(eval32("0xFF ^ 0xFF").unwrap(), 0x00);
        assert_eq!(eval32("0xAA ^ 0x55").unwrap(), 0xFF);
        assert_eq!(eval32("0x12 ^ 0x34").unwrap(), 0x26);
        // XOR with zero is identity.
        assert_eq!(eval32("0x12345678 ^ 0").unwrap(), 0x12345678);
    }

    #[test]
    fn bitwise_not() {
        assert_eq!(eval32("~0x00000000").unwrap(), 0xFFFFFFFF);
        assert_eq!(eval32("~0xFFFFFFFF").unwrap(), 0x00000000);
        assert_eq!(eval32("~0x55555555").unwrap(), 0xAAAAAAAA);
    }

    #[test]
    fn left_shift() {
        assert_eq!(eval32("1 << 0").unwrap(), 1);
        assert_eq!(eval32("1 << 8").unwrap(), 0x100);
        assert_eq!(eval32("0xFF << 8").unwrap(), 0xFF00);
        // Shift beyond width wraps to zero.
        assert_eq!(eval32("1 << 32").unwrap(), 0);
        assert_eq!(eval32("1 << 40").unwrap(), 0);
    }

    #[test]
    fn right_shift_unsigned() {
        assert_eq!(eval32("0xFF >> 1").unwrap(), 0x7F);
        assert_eq!(eval32("0x100 >> 8").unwrap(), 0x01);
        assert_eq!(eval32("0x80000000 >> 1").unwrap(), 0x40000000);
        // Shift beyond width yields zero.
        assert_eq!(eval32("1 >> 32").unwrap(), 0);
    }

    #[test]
    fn right_shift_signed_extends() {
        let signed8 =
            |s: &str| eval(s, w(8), Signedness::Signed, Value::new(0, w(8))).map(|v| v.raw());
        // 0x80 is -128 signed; right shift fills with 1s.
        assert_eq!(signed8("0x80 >> 1").unwrap(), 0xC0);
        assert_eq!(signed8("0x80 >> 2").unwrap(), 0xE0);
        // 0x40 is positive; right shift fills with 0s.
        assert_eq!(signed8("0x40 >> 1").unwrap(), 0x20);
        assert_eq!(signed8("0x40 >> 2").unwrap(), 0x10);
    }

    #[test]
    fn arithmetic_add_and_wrap() {
        assert_eq!(eval32("0 + 0").unwrap(), 0);
        assert_eq!(eval32("1 + 1").unwrap(), 2);
        assert_eq!(eval32("0xFFFFFFFF + 1").unwrap(), 0x00000000);
        assert_eq!(eval32("0x7FFFFFFF + 0x7FFFFFFF").unwrap(), 0xFFFFFFFE);
    }

    #[test]
    fn arithmetic_sub_and_wrap() {
        assert_eq!(eval32("5 - 3").unwrap(), 2);
        assert_eq!(eval32("0 - 1").unwrap(), 0xFFFFFFFF);
        assert_eq!(eval32("0x80000000 - 1").unwrap(), 0x7FFFFFFF);
    }

    #[test]
    fn arithmetic_mul_and_wrap() {
        assert_eq!(eval32("3 * 4").unwrap(), 12);
        assert_eq!(eval32("256 * 256").unwrap(), 0x10000);
        // 0x80000000 * 2 = 0x100000000, which wraps to 0 at 32-bit width.
        let r = eval(
            "0x80000000 * 2",
            w(32),
            Signedness::Unsigned,
            Value::new(0, w(32)),
        )
        .unwrap();
        assert_eq!(r.raw(), 0);
    }

    #[test]
    fn arithmetic_div_unsigned() {
        assert_eq!(eval32("10 / 2").unwrap(), 5);
        assert_eq!(eval32("10 / 3").unwrap(), 3); // truncates
        assert_eq!(eval32("1 / 2").unwrap(), 0);  // truncates to zero
    }

    #[test]
    fn arithmetic_div_signed() {
        let signed32 = |s: &str| {
            eval(
                s,
                w(32),
                Signedness::Signed,
                Value::new(0, w(32)),
            )
            .map(|v| v.raw() as i32)
        };
        assert_eq!(signed32("10 / 2").unwrap(), 5);
        assert_eq!(signed32("-10 / 2").unwrap(), -5);
        assert_eq!(signed32("10 / -2").unwrap(), -5);
        assert_eq!(signed32("-10 / -2").unwrap(), 5);
    }

    #[test]
    fn arithmetic_rem_unsigned() {
        assert_eq!(eval32("17 % 5").unwrap(), 2);
        assert_eq!(eval32("10 % 3").unwrap(), 1);
        assert_eq!(eval32("5 % 10").unwrap(), 5);
    }

    #[test]
    fn arithmetic_rem_signed() {
        let signed32 = |s: &str| {
            eval(
                s,
                w(32),
                Signedness::Signed,
                Value::new(0, w(32)),
            )
            .map(|v| v.raw() as i32)
        };
        // Remainder follows dividend sign in Rust.
        assert_eq!(signed32("17 % 5").unwrap(), 2);
        assert_eq!(signed32("-17 % 5").unwrap(), -2);
        assert_eq!(signed32("17 % -5").unwrap(), 2);
        assert_eq!(signed32("-17 % -5").unwrap(), -2);
    }

    #[test]
    fn function_sqrt_edge_cases() {
        assert_eq!(eval32("sqrt(0)").unwrap(), 0);
        assert_eq!(eval32("sqrt(1)").unwrap(), 1);
        assert_eq!(eval32("sqrt(4)").unwrap(), 2);
        assert_eq!(eval32("sqrt(9)").unwrap(), 3);
        assert_eq!(eval32("sqrt(100)").unwrap(), 10);
        // sqrt(255) = 15 (15^2 = 225 < 255, 16^2 = 256 > 255)
        assert_eq!(eval32("sqrt(255)").unwrap(), 15);
        assert_eq!(eval32("sqrt(256)").unwrap(), 16);
    }

    #[test]
    fn function_log2_edge_cases() {
        assert_eq!(eval32("log2(1)").unwrap(), 0);
        assert_eq!(eval32("log2(2)").unwrap(), 1);
        assert_eq!(eval32("log2(4)").unwrap(), 2);
        assert_eq!(eval32("log2(8)").unwrap(), 3);
        assert_eq!(eval32("log2(1024)").unwrap(), 10);
        // log2(1023) = 9 (2^9 = 512 < 1023 < 1024 = 2^10)
        assert_eq!(eval32("log2(1023)").unwrap(), 9);
        assert_eq!(eval32("log2(1025)").unwrap(), 10);
    }

    #[test]
    fn function_clog2_edge_cases() {
        // clog2(n) = ceil(log2(n)), or the bit position of the MSB + 1.
        assert_eq!(eval32("clog2(0)").unwrap(), 0);
        assert_eq!(eval32("clog2(1)").unwrap(), 0); // needs 0 bits
        assert_eq!(eval32("clog2(2)").unwrap(), 1);
        assert_eq!(eval32("clog2(3)").unwrap(), 2);
        assert_eq!(eval32("clog2(4)").unwrap(), 2);
        assert_eq!(eval32("clog2(5)").unwrap(), 3);
        assert_eq!(eval32("clog2(1024)").unwrap(), 10);
        assert_eq!(eval32("clog2(1000)").unwrap(), 10);
        assert_eq!(eval32("clog2(1025)").unwrap(), 11);
    }

    #[test]
    fn function_popcount() {
        assert_eq!(eval32("popcount(0)").unwrap(), 0);
        assert_eq!(eval32("popcount(1)").unwrap(), 1);
        assert_eq!(eval32("popcount(3)").unwrap(), 2);
        assert_eq!(eval32("popcount(0x0F)").unwrap(), 4);
        assert_eq!(eval32("popcount(0xFF)").unwrap(), 8);
        assert_eq!(eval32("popcount(0xFFFF)").unwrap(), 16);
        assert_eq!(eval32("popcount(0xFFFFFFFF)").unwrap(), 32);
    }

    #[test]
    fn function_gcd() {
        assert_eq!(eval32("gcd(0, 5)").unwrap(), 5);
        assert_eq!(eval32("gcd(5, 0)").unwrap(), 5);
        assert_eq!(eval32("gcd(12, 8)").unwrap(), 4);
        assert_eq!(eval32("gcd(54, 24)").unwrap(), 6);
        assert_eq!(eval32("gcd(17, 19)").unwrap(), 1);
        assert_eq!(eval32("gcd(100, 50)").unwrap(), 50);
    }

    #[test]
    fn function_lcm() {
        assert_eq!(eval32("lcm(0, 5)").unwrap(), 0);
        assert_eq!(eval32("lcm(4, 6)").unwrap(), 12);
        assert_eq!(eval32("lcm(12, 8)").unwrap(), 24);
        assert_eq!(eval32("lcm(7, 11)").unwrap(), 77);
    }

    #[test]
    fn function_min_max() {
        assert_eq!(eval32("min(3, 9)").unwrap(), 3);
        assert_eq!(eval32("min(9, 3)").unwrap(), 3);
        assert_eq!(eval32("max(3, 9)").unwrap(), 9);
        assert_eq!(eval32("max(9, 3)").unwrap(), 9);
        assert_eq!(eval32("min(0, 0)").unwrap(), 0);
        assert_eq!(eval32("max(0, 0)").unwrap(), 0);
    }

    #[test]
    fn function_factorial() {
        assert_eq!(eval32("fact(0)").unwrap(), 1);
        assert_eq!(eval32("fact(1)").unwrap(), 1);
        assert_eq!(eval32("fact(2)").unwrap(), 2);
        assert_eq!(eval32("fact(3)").unwrap(), 6);
        assert_eq!(eval32("fact(5)").unwrap(), 120);
        assert_eq!(eval32("fact(10)").unwrap(), 3628800);
    }

    #[test]
    fn function_pow() {
        assert_eq!(eval32("pow(2, 0)").unwrap(), 1);
        assert_eq!(eval32("pow(2, 1)").unwrap(), 2);
        assert_eq!(eval32("pow(2, 8)").unwrap(), 256);
        assert_eq!(eval32("pow(3, 3)").unwrap(), 27);
        assert_eq!(eval32("pow(10, 3)").unwrap(), 1000);
        // pow with negative exponent (in u32 mode) wraps.
        assert_eq!(eval32("pow(2, -1)").unwrap(), 0);
    }

    #[test]
    fn function_abs() {
        let signed32 = |s: &str| {
            eval(
                s,
                w(32),
                Signedness::Signed,
                Value::new(0, w(32)),
            )
            .map(|v| v.raw())
        };
        assert_eq!(signed32("abs(0)").unwrap(), 0);
        assert_eq!(signed32("abs(42)").unwrap(), 42);
        assert_eq!(signed32("abs(-42)").unwrap(), 42);
        // abs of minimum i32 (-2147483648) wraps to itself in two's complement.
        assert_eq!(signed32("abs(-2147483648)").unwrap(), 0x80000000);
    }

    #[test]
    fn function_sign() {
        let signed32 = |s: &str| {
            eval(
                s,
                w(32),
                Signedness::Signed,
                Value::new(0, w(32)),
            )
            .map(|v| v.raw() as i32)
        };
        assert_eq!(signed32("sign(0)").unwrap(), 0);
        assert_eq!(signed32("sign(42)").unwrap(), 1);
        assert_eq!(signed32("sign(-42)").unwrap(), -1);
    }

    // --- Operator combinations -------------------------------------------

    #[test]
    fn mixed_bitwise_arithmetic() {
        // (1 | 2) * 4 = 3 * 4 = 12
        assert_eq!(eval32("(1 | 2) * 4").unwrap(), 12);
        // 0xFF & (1 << 3) = 0x08
        assert_eq!(eval32("0xFF & (1 << 3)").unwrap(), 0x08);
        // 10 + 5 << 1 = 15 << 1 = 30 (shift binds looser than +)
        assert_eq!(eval32("10 + 5 << 1").unwrap(), 30);
    }

    #[test]
    fn complex_precedence_chain() {
        // 2 + 3 * 4 - 5 << 1 = 2 + 12 - 5 << 1 = 9 << 1 = 18
        assert_eq!(eval32("2 + 3 * 4 - 5 << 1").unwrap(), 18);
        // (0xFF & 0x0F) | (1 << 4) = 0x0F | 0x10 = 0x1F
        assert_eq!(eval32("(0xFF & 0x0F) | (1 << 4)").unwrap(), 0x1F);
    }

    #[test]
    fn functions_in_expressions() {
        // sqrt(256) * 2 = 16 * 2 = 32
        assert_eq!(eval32("sqrt(256) * 2").unwrap(), 32);
        // log2(1024) + log2(2) = 10 + 1 = 11
        assert_eq!(eval32("log2(1024) + log2(2)").unwrap(), 11);
        // popcount(0xFF) * 2 = 8 * 2 = 16
        assert_eq!(eval32("popcount(0xFF) * 2").unwrap(), 16);
        // gcd(54, 24) + gcd(12, 8) = 6 + 4 = 10
        assert_eq!(eval32("gcd(54, 24) + gcd(12, 8)").unwrap(), 10);
    }

    #[test]
    fn nested_function_calls() {
        // sqrt(pow(2, 8)) = sqrt(256) = 16
        assert_eq!(eval32("sqrt(pow(2, 8))").unwrap(), 16);
        // log2(pow(2, 10)) = 10
        assert_eq!(eval32("log2(pow(2, 10))").unwrap(), 10);
        // min(gcd(54, 24), gcd(12, 8)) = min(6, 4) = 4
        assert_eq!(eval32("min(gcd(54, 24), gcd(12, 8))").unwrap(), 4);
    }

    #[test]
    fn functions_with_expressions() {
        // sqrt(100 + 156) = sqrt(256) = 16
        assert_eq!(eval32("sqrt(100 + 156)").unwrap(), 16);
        // popcount(0xAA | 0x55) = popcount(0xFF) = 8
        assert_eq!(eval32("popcount(0xAA | 0x55)").unwrap(), 8);
        // gcd(10 * 5, 20 / 2) = gcd(50, 10) = 10
        assert_eq!(eval32("gcd(10 * 5, 20 / 2)").unwrap(), 10);
    }

    #[test]
    fn power_right_associativity() {
        // 2 ** 3 ** 2 = 2 ** 9 = 512 (not (2**3)**2 = 64)
        assert_eq!(eval32("2 ** 3 ** 2").unwrap(), 512);
        // 2 ** 2 ** 3 = 2 ** 8 = 256
        assert_eq!(eval32("2 ** 2 ** 3").unwrap(), 256);
    }

    #[test]
    fn floatmode_bitwise_rejected() {
        assert!(matches!(
            eval_float("0xFF & 0x0F", 0.0),
            Err(EvalError::BitwiseInFloatMode('&'))
        ));
        assert!(matches!(
            eval_float("1 | 2", 0.0),
            Err(EvalError::BitwiseInFloatMode('|'))
        ));
        assert!(matches!(
            eval_float("~0xFF", 0.0),
            Err(EvalError::BitwiseInFloatMode('~'))
        ));
        assert!(matches!(
            eval_float("1 << 2", 0.0),
            Err(EvalError::BitwiseInFloatMode('<'))
        ));
    }
}
