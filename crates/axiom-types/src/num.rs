//! Exact numeric representation for the normative Axiom numeric path.
//!
//! Floating point is intentionally absent. All normative arithmetic uses
//! [`Num`] (integer, reduced rational, or fixed-scale decimal) so that the
//! canonical encoding and deterministic replay never depend on IEEE-754
//! rounding behaviour.

use crate::error::NumError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Num {
    /// Signed integer.
    Int(i128),
    /// Reduced rational with `den > 0`.
    Rational { num: i128, den: i128 },
    /// Fixed-scale decimal with value `mantissa * 10^-scale`, `scale <= 38`.
    Decimal { mantissa: i128, scale: u8 },
}

impl Num {
    pub fn rational(num: i128, den: i128) -> Result<Num, NumError> {
        if den == 0 {
            return Err(NumError::DivisionByZero);
        }
        let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd(num.unsigned_abs(), den.unsigned_abs());
        Ok(Num::Rational {
            num: num / g as i128,
            den: den / g as i128,
        })
    }

    pub fn decimal(mantissa: i128, scale: u8) -> Result<Num, NumError> {
        if scale > 38 {
            return Err(NumError::LossOfPrecision);
        }
        Ok(Num::Decimal { mantissa, scale })
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Num::Int(n) => *n == 0,
            Num::Rational { num, .. } => *num == 0,
            Num::Decimal { mantissa, .. } => *mantissa == 0,
        }
    }

    pub fn to_rational(self) -> Result<(i128, i128), NumError> {
        match self {
            Num::Int(n) => Ok((n, 1)),
            Num::Rational { num, den } => Ok((num, den)),
            Num::Decimal { mantissa, scale } => {
                let (factor, ok) = pow10(scale as u32);
                if !ok {
                    return Err(NumError::Overflow);
                }
                let (num, den) = reduce(mantissa, factor as i128);
                Ok((num, den))
            }
        }
    }

    pub fn checked_add(self, other: Num) -> Result<Num, NumError> {
        let (a, b) = self.to_rational()?;
        let (c, d) = other.to_rational()?;
        let den = lcm(b, d);
        let an = a * (den / b);
        let cn = c * (den / d);
        let num = an.checked_add(cn).ok_or(NumError::Overflow)?;
        reduce_to_num(num, den)
    }

    pub fn checked_sub(self, other: Num) -> Result<Num, NumError> {
        other.checked_neg()?.checked_add(self)
    }

    pub fn checked_mul(self, other: Num) -> Result<Num, NumError> {
        let (a, b) = self.to_rational()?;
        let (c, d) = other.to_rational()?;
        let num = a.checked_mul(c).ok_or(NumError::Overflow)?;
        let den = b.checked_mul(d).ok_or(NumError::Overflow)?;
        reduce_to_num(num, den)
    }

    pub fn checked_div(self, other: Num) -> Result<Num, NumError> {
        if other.is_zero() {
            return Err(NumError::DivisionByZero);
        }
        let (a, b) = self.to_rational()?;
        let (c, d) = other.to_rational()?;
        let num = a.checked_mul(d).ok_or(NumError::Overflow)?;
        let den = b.checked_mul(c).ok_or(NumError::Overflow)?;
        reduce_to_num(num, den)
    }

    pub fn checked_neg(self) -> Result<Num, NumError> {
        match self {
            Num::Int(n) => Ok(Num::Int(n.wrapping_neg())),
            Num::Rational { num, den } => Ok(Num::Rational { num: -num, den }),
            Num::Decimal { mantissa, scale } => Ok(Num::Decimal {
                mantissa: -mantissa,
                scale,
            }),
        }
    }

    /// Canonical ordering used for interval endpoints and deterministic sorts.
    pub fn cmp_num(&self, other: &Num) -> Ordering {
        let (a, b) = match (self.to_rational(), other.to_rational()) {
            (Ok(x), Ok(y)) => (x, y),
            _ => return Ordering::Equal,
        };
        let den = lcm(a.1, b.1);
        let an = a.0 * (den / a.1);
        let cn = b.0 * (den / b.1);
        an.cmp(&cn)
    }

    /// Decimal text for display only. Never used in canonical encoding.
    pub fn display(&self) -> String {
        match self {
            Num::Int(n) => n.to_string(),
            Num::Rational { num, den } => format!("{num}/{den}"),
            Num::Decimal { mantissa, scale } => {
                if *scale == 0 {
                    mantissa.to_string()
                } else {
                    let neg = *mantissa < 0;
                    let m = mantissa.unsigned_abs().to_string();
                    let s = *scale as usize;
                    let (int, frac) = if m.len() <= s {
                        let padded = format!("{:0>width$}", m, width = s);
                        let (a, b) = padded.split_at(padded.len() - s);
                        ("0".to_string(), a.to_string() + b)
                    } else {
                        let (a, b) = m.split_at(m.len() - s);
                        (a.to_string(), b.to_string())
                    };
                    format!("{}{}.{}", if neg { "-" } else { "" }, int, frac)
                }
            }
        }
    }

    pub fn parse(s: &str) -> Result<Num, NumError> {
        let s = s.trim();
        if let Some((a, b)) = s.split_once('/') {
            let num: i128 = a.trim().parse().map_err(|_| NumError::Parse(s.into()))?;
            let den: i128 = b.trim().parse().map_err(|_| NumError::Parse(s.into()))?;
            return Num::rational(num, den);
        }
        if let Some(dot) = s.find('.') {
            let int_part = &s[..dot];
            let frac_part = &s[dot + 1..];
            let scale = frac_part.len() as u8;
            let combined = format!("{}{}", int_part, frac_part);
            let mantissa: i128 = combined
                .trim_start_matches('-')
                .parse()
                .map_err(|_| NumError::Parse(s.into()))?;
            let mantissa = if s.starts_with('-') {
                -mantissa
            } else {
                mantissa
            };
            return Num::decimal(mantissa, scale);
        }
        let n: i128 = s.parse().map_err(|_| NumError::Parse(s.into()))?;
        Ok(Num::Int(n))
    }
}

fn reduce_to_num(num: i128, den: i128) -> Result<Num, NumError> {
    if den < 0 {
        return reduce_to_num(-num, -den);
    }
    if den == 0 {
        return Err(NumError::DivisionByZero);
    }
    // If both fit i128 and denominator is 1, emit Int.
    if den == 1 {
        return Ok(Num::Int(num));
    }
    // Prefer decimal when denominator is a power of ten (lossless).
    if let Some(scale) = is_pow10(den) {
        if scale <= 38 {
            return Ok(Num::Decimal {
                mantissa: num,
                scale: scale as u8,
            });
        }
    }
    Ok(Num::Rational { num, den })
}

fn is_pow10(mut n: i128) -> Option<u32> {
    if n <= 0 {
        return None;
    }
    let mut scale = 0u32;
    while n % 10 == 0 {
        n /= 10;
        scale += 1;
    }
    if n == 1 {
        Some(scale)
    } else {
        None
    }
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

fn reduce(mut num: i128, mut den: i128) -> (i128, i128) {
    let g = gcd(num.unsigned_abs(), den.unsigned_abs()) as i128;
    num /= g;
    den /= g;
    if den < 0 {
        (num, -1)
    } else {
        (num, den)
    }
}

fn lcm(a: i128, b: i128) -> i128 {
    a.abs()
        .checked_mul(b.abs())
        .map(|p| p / gcd(a.unsigned_abs(), b.unsigned_abs()) as i128)
        .unwrap_or(i128::MAX)
}

fn pow10(e: u32) -> (u128, bool) {
    let mut v: u128 = 1;
    for _ in 0..e {
        match v.checked_mul(10) {
            Some(x) => v = x,
            None => return (v, false),
        }
    }
    (v, true)
}

impl fmt::Display for Num {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

impl Serialize for Num {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Num::Int(n) => s.serialize_str(&format!("i{n}")),
            Num::Rational { num, den } => s.serialize_str(&format!("r{num}/{den}")),
            Num::Decimal { mantissa, scale } => s.serialize_str(&format!("d{mantissa}/{scale}")),
        }
    }
}

impl<'de> Deserialize<'de> for Num {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        let n = parse_num_str(&s).map_err(serde::de::Error::custom)?;
        Ok(n)
    }
}

fn parse_num_str(s: &str) -> Result<Num, NumError> {
    match s.as_bytes().first() {
        Some(b'i') => Ok(Num::Int(
            s[1..].parse().map_err(|_| NumError::Parse(s.into()))?,
        )),
        Some(b'r') => {
            let inner = &s[1..];
            let (a, b) = inner
                .split_once('/')
                .ok_or_else(|| NumError::Parse(s.into()))?;
            Num::rational(
                a.parse().map_err(|_| NumError::Parse(s.into()))?,
                b.parse().map_err(|_| NumError::Parse(s.into()))?,
            )
        }
        Some(b'd') => {
            let inner = &s[1..];
            let (a, b) = inner
                .split_once('/')
                .ok_or_else(|| NumError::Parse(s.into()))?;
            Num::decimal(
                a.parse().map_err(|_| NumError::Parse(s.into()))?,
                b.parse().map_err(|_| NumError::Parse(s.into()))?,
            )
        }
        _ => Err(NumError::Parse(s.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_is_exact() {
        let a = Num::Int(1);
        let b = Num::Int(3);
        assert_eq!(a.checked_div(b).unwrap(), Num::Rational { num: 1, den: 3 });
        let c = Num::decimal(1, 2).unwrap(); // 0.01
        let d = Num::decimal(2, 2).unwrap(); // 0.02
        assert_eq!(c.checked_add(d).unwrap(), Num::decimal(3, 2).unwrap());
    }

    #[test]
    fn decimal_from_string() {
        assert_eq!(Num::parse("1.5").unwrap(), Num::decimal(15, 1).unwrap());
        assert_eq!(Num::parse("2/4").unwrap(), Num::Rational { num: 1, den: 2 });
    }

    #[test]
    fn ordering() {
        assert_eq!(
            Num::parse("1/2")
                .unwrap()
                .cmp_num(&Num::decimal(5, 1).unwrap()),
            Ordering::Equal
        );
        assert!(Num::Int(3).cmp_num(&Num::Int(2)) == Ordering::Greater);
    }
}
