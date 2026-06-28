//! Parsing of a single numeric literal into a raw `u128` magnitude, and
//! parsing of typed base-field strings (hex/bin/oct/dec) into [`Value`].

use std::fmt;

use crate::value::{Signedness, Value, Width};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No digits present (e.g. empty string or a lone `0x`).
    Empty,
    /// A character that isn't a valid digit for the radix.
    InvalidDigit(char),
    /// The magnitude doesn't fit in `u128`.
    Overflow,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "no digits in number"),
            ParseError::InvalidDigit(c) => write!(f, "invalid digit '{c}'"),
            ParseError::Overflow => write!(f, "number too large (exceeds 128 bits)"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Strip a 2-character ASCII prefix case-insensitively, returning the rest.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

/// Parse a single numeric literal token into its raw `u128` magnitude.
pub fn parse_literal(s: &str) -> Result<u128, ParseError> {
    let (radix, digits) = if let Some(rest) = strip_prefix_ci(s, "0x") {
        (16u32, rest)
    } else if let Some(rest) = strip_prefix_ci(s, "0b") {
        (2, rest)
    } else if let Some(rest) = strip_prefix_ci(s, "0o") {
        (8, rest)
    } else {
        (10, s)
    };

    let mut acc: u128 = 0;
    let mut saw_digit = false;
    for c in digits.chars() {
        if c == '_' {
            continue;
        }
        let d = c.to_digit(radix).ok_or(ParseError::InvalidDigit(c))?;
        acc = acc
            .checked_mul(radix as u128)
            .and_then(|a| a.checked_add(d as u128))
            .ok_or(ParseError::Overflow)?;
        saw_digit = true;
    }

    if saw_digit {
        Ok(acc)
    } else {
        Err(ParseError::Empty)
    }
}

/// Strip the radix prefix (`0x`, `0b`, `0o`) from `s`, case-insensitively.
/// Returns `s` unchanged for unsupported radix values or when no prefix is present.
fn strip_radix_prefix(s: &str, radix: u32) -> &str {
    let prefix = match radix {
        16 => "0x",
        2 => "0b",
        8 => "0o",
        _ => return s,
    };
    strip_prefix_ci(s, prefix).unwrap_or(s)
}

/// Parse a base-field string (hex, dec, bin, or oct) into a width-masked [`Value`].
///
/// Whitespace and `_` separators are stripped before parsing. Each radix optionally
/// accepts a `0x`/`0b`/`0o` prefix. Decimal accepts a leading `-` in signed mode.
/// An empty or all-whitespace input produces `Value::new(0, width)`.
pub fn parse_base(text: &str, radix: u32, width: Width, sign: Signedness) -> Result<Value, String> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if cleaned.is_empty() {
        return Ok(Value::new(0, width));
    }

    if radix == 10 {
        if let Some(mag) = cleaned.strip_prefix('-') {
            if sign == Signedness::Unsigned {
                return Err("negative value in unsigned mode".to_owned());
            }
            let n: i128 = mag
                .parse()
                .map_err(|_| "invalid decimal number".to_owned())?;
            return Ok(Value::new((-n) as u128, width));
        }
        let n: u128 = cleaned
            .parse()
            .map_err(|_| "invalid decimal number".to_owned())?;
        return Ok(Value::new(n, width));
    }

    let body = strip_radix_prefix(&cleaned, radix);
    let n =
        u128::from_str_radix(body, radix).map_err(|_| format!("invalid base-{radix} number"))?;
    Ok(Value::new(n, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bases() {
        assert_eq!(parse_literal("255").unwrap(), 255);
        assert_eq!(parse_literal("0xFF").unwrap(), 255);
        assert_eq!(parse_literal("0xff").unwrap(), 255);
        assert_eq!(parse_literal("0Xff").unwrap(), 255);
        assert_eq!(parse_literal("0b1010").unwrap(), 10);
        assert_eq!(parse_literal("0o17").unwrap(), 15);
        assert_eq!(parse_literal("0").unwrap(), 0);
    }

    #[test]
    fn underscores_allowed() {
        assert_eq!(parse_literal("0xDEAD_BEEF").unwrap(), 0xDEADBEEF);
        assert_eq!(parse_literal("1_000").unwrap(), 1000);
        assert_eq!(parse_literal("0b1111_0000").unwrap(), 0xF0);
    }

    #[test]
    fn errors() {
        assert_eq!(parse_literal("0x"), Err(ParseError::Empty));
        assert_eq!(parse_literal(""), Err(ParseError::Empty));
        assert_eq!(parse_literal("0b2"), Err(ParseError::InvalidDigit('2')));
        assert_eq!(parse_literal("12G"), Err(ParseError::InvalidDigit('G')));
        // 2^128 overflows u128.
        assert_eq!(
            parse_literal("0x1_0000_0000_0000_0000_0000_0000_0000_0000"),
            Err(ParseError::Overflow)
        );
    }

    #[test]
    fn max_u128_fits() {
        assert_eq!(
            parse_literal("0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF").unwrap(),
            u128::MAX
        );
    }

    #[test]
    fn parse_base_hex() {
        let w = Width::new(32).unwrap();
        let v = parse_base("DEADBEEF", 16, w, Signedness::Unsigned).unwrap();
        assert_eq!(v.raw(), 0xDEADBEEF);
        // With prefix
        let v = parse_base("0xDEAD_BEEF", 16, w, Signedness::Unsigned).unwrap();
        assert_eq!(v.raw(), 0xDEADBEEF);
    }

    #[test]
    fn parse_base_dec_signed() {
        let w = Width::new(32).unwrap();
        let v = parse_base("-1", 10, w, Signedness::Signed).unwrap();
        assert_eq!(v.raw(), 0xFFFF_FFFF); // two's complement -1 in 32 bits
                                          // Unsigned mode rejects negative
        assert!(parse_base("-1", 10, w, Signedness::Unsigned).is_err());
    }

    #[test]
    fn parse_base_whitespace_and_underscores() {
        let w = Width::new(16).unwrap();
        let v = parse_base("  FF FF  ", 16, w, Signedness::Unsigned).unwrap();
        assert_eq!(v.raw(), 0xFFFF);
    }

    #[test]
    fn parse_base_empty_yields_zero() {
        let w = Width::new(8).unwrap();
        let v = parse_base("", 16, w, Signedness::Unsigned).unwrap();
        assert_eq!(v.raw(), 0);
    }

    #[test]
    fn parse_base_bin_with_prefix() {
        let w = Width::new(8).unwrap();
        let v = parse_base("0b1010_1010", 2, w, Signedness::Unsigned).unwrap();
        assert_eq!(v.raw(), 0xAA);
    }
}
