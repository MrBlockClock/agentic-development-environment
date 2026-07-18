use crate::error::AdeError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

/// Exact USD amount stored as integer micro-dollars (`1_000_000` = $1.00).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Self = Self(0);
    pub const MICRO_PER_USD: i64 = 1_000_000;

    pub const fn from_micros(micros: i64) -> Self {
        Self(if micros < 0 { 0 } else { micros })
    }

    pub const fn micros(self) -> i64 {
        self.0
    }

    pub fn from_usd_str(value: &str) -> Result<Self, AdeError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(AdeError::Config("empty USD amount".into()));
        }
        let negative = trimmed.starts_with('-');
        let body = trimmed.trim_start_matches(['+', '-']);
        let (whole, frac) = match body.split_once('.') {
            Some((whole, frac)) => (whole, frac),
            None => (body, ""),
        };
        if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
            return Err(AdeError::Config(format!("invalid USD amount '{value}'")));
        }
        if frac.chars().any(|c| !c.is_ascii_digit()) {
            return Err(AdeError::Config(format!("invalid USD amount '{value}'")));
        }
        if frac.len() > 6 {
            return Err(AdeError::Config(format!(
                "USD amount '{value}' has more than 6 decimal places"
            )));
        }
        let whole_micros = whole
            .parse::<i64>()
            .map_err(|_| AdeError::Config(format!("invalid USD amount '{value}'")))?
            .checked_mul(Self::MICRO_PER_USD)
            .ok_or_else(|| AdeError::Config(format!("USD amount '{value}' overflows")))?;
        let mut frac_buf = frac.to_string();
        while frac_buf.len() < 6 {
            frac_buf.push('0');
        }
        let frac_micros = if frac_buf.is_empty() {
            0
        } else {
            frac_buf
                .parse::<i64>()
                .map_err(|_| AdeError::Config(format!("invalid USD amount '{value}'")))?
        };
        let micros = whole_micros
            .checked_add(frac_micros)
            .ok_or_else(|| AdeError::Config(format!("USD amount '{value}' overflows")))?;
        if negative && micros != 0 {
            return Err(AdeError::Config(
                "money amounts must be non-negative".into(),
            ));
        }
        Ok(Self(micros))
    }

    /// Converts a finite non-negative f64 USD value using banker's rounding to micros.
    pub fn try_from_usd_f64(value: f64) -> Result<Self, AdeError> {
        if !value.is_finite() || value < 0.0 {
            return Err(AdeError::Config(
                "money amounts must be finite and non-negative".into(),
            ));
        }
        let scaled = value * Self::MICRO_PER_USD as f64;
        if scaled > i64::MAX as f64 {
            return Err(AdeError::Config(
                "USD amount overflows micro-dollars".into(),
            ));
        }
        Ok(Self(scaled.round() as i64))
    }

    pub fn to_usd_f64(self) -> f64 {
        self.0 as f64 / Self::MICRO_PER_USD as f64
    }

    pub fn format_usd(self) -> String {
        let sign = if self.0 < 0 { "-" } else { "" };
        let abs = self.0.unsigned_abs();
        let whole = abs / Self::MICRO_PER_USD as u64;
        let frac = abs % Self::MICRO_PER_USD as u64;
        format!("{sign}{whole}.{frac:06}")
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0).max(0))
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0).max(0))
    }

    /// Ceiling cost for `tokens` billed at `rate_per_million_tokens`.
    pub fn cost_for_tokens(tokens: u64, rate_per_million: Self) -> Self {
        if tokens == 0 || rate_per_million.0 == 0 {
            return Self::ZERO;
        }
        let product = (tokens as u128).saturating_mul(rate_per_million.0 as u128);
        let micros = product.div_ceil(1_000_000);
        if micros > i64::MAX as u128 {
            Self(i64::MAX)
        } else {
            Self(micros as i64)
        }
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Add for Money {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl AddAssign for Money {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.saturating_add(rhs);
    }
}

impl Sub for Money {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self.saturating_sub(rhs)
    }
}

impl SubAssign for Money {
    fn sub_assign(&mut self, rhs: Self) {
        *self = self.saturating_sub(rhs);
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}", self.format_usd())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_formats_usd() {
        let money = Money::from_usd_str("1.25").unwrap();
        assert_eq!(money.micros(), 1_250_000);
        assert_eq!(money.format_usd(), "1.250000");
        assert_eq!(Money::from_usd_str("0.000001").unwrap().micros(), 1);
    }

    #[test]
    fn rejects_negative_and_overflow_scale() {
        assert!(Money::from_usd_str("-1.00").is_err());
        assert!(Money::from_usd_str("1.0000001").is_err());
        assert!(Money::try_from_usd_f64(-0.1).is_err());
    }

    #[test]
    fn ceilings_token_costs() {
        let rate = Money::from_usd_str("2.000000").unwrap();
        // 1 token at $2/MTok => 2 micro-dollars exactly.
        assert_eq!(Money::cost_for_tokens(1, rate).micros(), 2);
        // 1 token at $1/MTok => ceil(1) micro-dollar.
        let rate = Money::from_usd_str("1.000000").unwrap();
        assert_eq!(Money::cost_for_tokens(1, rate).micros(), 1);
        assert_eq!(Money::cost_for_tokens(1_000_000, rate).micros(), 1_000_000);
    }
}
