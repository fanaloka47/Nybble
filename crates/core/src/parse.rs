//! Parsing of a single numeric literal into a raw `u128` magnitude.
//!
//! Accepted forms (case-insensitive prefixes, optional `_` separators):
//! `0x1A`, `0b1010`, `0o17`, `255`, `DE_AD` only with a `0x` prefix. A bare
//! string of hex letters is *not* a number here — the expression layer treats
//! letters as identifiers (e.g. `ans`), so hex needs the `0x` prefix.

use std::fmt;

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
        assert_eq!(parse_literal("0xFFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF_FFFF").unwrap(), u128::MAX);
    }
}
