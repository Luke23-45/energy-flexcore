// P16.07/P16.09/P16.10 — The fixed-decimal record type for settlement-
// adjacent money and metered quantities.
//
// Shared by the Phase 16 Agent B modules (price records, settlement
// statements, meter data): every settlement-adjacent numeric record is a
// SCALING-FACTOR DECIMAL (i128 mantissa + u8 scale), never a float.
//
// WHY THIS EXISTS (decided once):
//
// - **No money arithmetic in floats** (phase-16 plan §4 non-goals; the
//   hard loop discipline): settlement amounts, prices, and metered
//   settlement quantities are financial-adjacent records. Binary floats
//   cannot represent decimal currency amounts exactly, so this module
//   represents every value as mantissa × 10^−scale — exact for decimal
//   inputs, deterministic in comparison, and never derived from an f64.
//   There is no `From<f64>` anywhere in this module: a float-derived
//   decimal is UNCONSTRUCTABLE by design.
// - **Currency is data, never a unit** (the contracts crate carries no
//   currency unit; plan §4): a [`MoneyAmount`] pairs a [`FixedDecimal`]
//   with the DECLARED currency code as a plain string. No default
//   currency exists; a currency-less money amount is unconstructable.
// - **Declared scale, exact comparison.** Equality and ordering rescale
//   both operands to the common (larger) scale with overflow-checked
//   multiplication; two decimals of different scales carrying the same
//   value (`1.50` and `1.5`) are EQUAL. Subtraction is alignment-first
//   and checked — an overflow is a named refusal, never a wrapped value.
// - **Parsing is the only entry path.** Wire-faithful decimal text (as
//   published by a market or metering source) is parsed at the adapter
//   boundary; the record layer never converts through a float. The
//   parser accepts an optional sign, at most one radix point, digits
//   otherwise, scale ≤ [`MAX_SCALE`]; everything else is a named refusal.
//
// Control status: pure value types for the simulation-only 0.x line. No
// I/O, no store, no arithmetic beyond alignment-checked add/sub/compare —
// no aggregation, tax, tariff, or billing computation is performed here
// (that belongs to jurisdiction-specific record packs, ADR-011).

use std::cmp::Ordering;

/// The maximum supported decimal scale (digits after the radix point).
/// 18 covers any published market/metering precision with headroom; a
/// source exceeding it is a named refusal, never a silent rounding.
pub const MAX_SCALE: u8 = 18;

/// Why a decimal was refused or an operation failed. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedDecimalError {
    /// The input text was empty or whitespace.
    EmptyInput,
    /// The input contained a character outside `[0-9.+-]` (or a sign was
    /// not leading).
    InvalidCharacter { position: usize, character: char },
    /// The input contained more than one radix point.
    MultipleRadixPoints,
    /// The input carried no digit at all (e.g. `"."`, `"-."`).
    NoDigits,
    /// The declared scale exceeded [`MAX_SCALE`].
    ScaleTooLarge { scale: u8, max: u8 },
    /// A rescale/subtract multiplication overflowed the i128 mantissa.
    MantissaOverflow,
    /// A currency code was empty, too long, or not ASCII (the code set is
    /// the SOURCE's, carried verbatim — validated for shape only).
    InvalidCurrencyCode { len: usize },
}

impl FixedDecimalError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            FixedDecimalError::EmptyInput => "FIXED_DECIMAL_EMPTY_INPUT",
            FixedDecimalError::InvalidCharacter { .. } => "FIXED_DECIMAL_INVALID_CHARACTER",
            FixedDecimalError::MultipleRadixPoints => "FIXED_DECIMAL_MULTIPLE_RADIX_POINTS",
            FixedDecimalError::NoDigits => "FIXED_DECIMAL_NO_DIGITS",
            FixedDecimalError::ScaleTooLarge { .. } => "FIXED_DECIMAL_SCALE_TOO_LARGE",
            FixedDecimalError::MantissaOverflow => "FIXED_DECIMAL_MANTISSA_OVERFLOW",
            FixedDecimalError::InvalidCurrencyCode { .. } => "FIXED_DECIMAL_INVALID_CURRENCY",
        }
    }
}

impl std::fmt::Display for FixedDecimalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FixedDecimalError::EmptyInput => {
                write!(f, "the decimal text is empty or whitespace")
            }
            FixedDecimalError::InvalidCharacter { position, character } => write!(
                f,
                "the decimal text carries the invalid character '{character}' at position \
                 {position} (allowed: leading sign, digits, at most one radix point)"
            ),
            FixedDecimalError::MultipleRadixPoints => {
                write!(f, "the decimal text carries more than one radix point")
            }
            FixedDecimalError::NoDigits => {
                write!(f, "the decimal text carries no digit")
            }
            FixedDecimalError::ScaleTooLarge { scale, max } => write!(
                f,
                "the decimal scale {scale} exceeds the maximum supported scale {max}; \
                 the value is refused, never silently rounded"
            ),
            FixedDecimalError::MantissaOverflow => write!(
                f,
                "aligning the decimals overflows the 128-bit mantissa; the operation is \
                 refused, never wrapped"
            ),
            FixedDecimalError::InvalidCurrencyCode { len } => write!(
                f,
                "the currency code has length {len}; it must be 1–8 ASCII characters, \
                 carried verbatim from the declaring source"
            ),
        }
    }
}

impl std::error::Error for FixedDecimalError {}

/// One exact decimal value: `mantissa × 10^−scale` (i128 mantissa, u8
/// scale ≤ [`MAX_SCALE`]). Constructed ONLY by [`FixedDecimal::parse`]
/// (from wire-faithful text) or [`FixedDecimal::from_parts`] (from
/// already-parsed parts); there is no float conversion path anywhere.
/// The type is a plain value pair and is `Copy` — copying a decimal is
/// exact and side-effect-free (unlike any float conversion, which does
/// not exist).
#[derive(Debug, Clone, Copy)]
pub struct FixedDecimal {
    mantissa: i128,
    scale: u8,
}

impl FixedDecimal {
    /// Parses wire-faithful decimal text (optional leading `+`/`-`, digits,
    /// at most one `.`). The scale is the digit count after the radix
    /// point; trailing zeros are preserved verbatim (`1.50` keeps scale 2
    /// — the published rendering is part of the record's provenance, and
    /// equality handles scale differences explicitly).
    pub fn parse(text: &str) -> Result<Self, FixedDecimalError> {
        if text.trim().is_empty() {
            return Err(FixedDecimalError::EmptyInput);
        }
        let body = text.trim();
        let bytes = body.as_bytes();
        let mut index = 0usize;
        let mut negative = false;
        if bytes[0] == b'+' || bytes[0] == b'-' {
            negative = bytes[0] == b'-';
            index = 1;
        }
        let mut mantissa: i128 = 0;
        let mut scale: u8 = 0;
        let mut seen_radix = false;
        let mut seen_digit = false;
        while index < bytes.len() {
            let byte = bytes[index];
            match byte {
                b'.' => {
                    if seen_radix {
                        return Err(FixedDecimalError::MultipleRadixPoints);
                    }
                    seen_radix = true;
                }
                b'0'..=b'9' => {
                    seen_digit = true;
                    let digit = i128::from(byte - b'0');
                    mantissa = mantissa
                        .checked_mul(10)
                        .and_then(|m| m.checked_add(digit))
                        .ok_or(FixedDecimalError::MantissaOverflow)?;
                    if seen_radix {
                        scale += 1;
                    }
                }
                _ => {
                    // Non-ASCII bytes report as their raw byte value.
                    let character = body[index..].chars().next().unwrap_or('?');
                    return Err(FixedDecimalError::InvalidCharacter {
                        position: index,
                        character,
                    });
                }
            }
            index += 1;
        }
        if !seen_digit {
            return Err(FixedDecimalError::NoDigits);
        }
        if scale > MAX_SCALE {
            return Err(FixedDecimalError::ScaleTooLarge {
                scale,
                max: MAX_SCALE,
            });
        }
        if negative {
            mantissa = -mantissa;
        }
        Ok(Self { mantissa, scale })
    }

    /// Constructs from already-parsed parts (the programmatic path for
    /// fixtures). The scale is validated against [`MAX_SCALE`]; both parts
    /// are stored verbatim.
    pub fn from_parts(mantissa: i128, scale: u8) -> Result<Self, FixedDecimalError> {
        if scale > MAX_SCALE {
            return Err(FixedDecimalError::ScaleTooLarge {
                scale,
                max: MAX_SCALE,
            });
        }
        Ok(Self { mantissa, scale })
    }

    /// The mantissa (value = mantissa × 10^−scale).
    pub fn mantissa(self) -> i128 {
        self.mantissa
    }

    /// The scale (digits after the radix point).
    pub fn scale(self) -> u8 {
        self.scale
    }

    /// True when the value is exactly zero (any scale).
    pub fn is_zero(self) -> bool {
        self.mantissa == 0
    }

    /// The canonical rendering (`mantissa` with the radix point `scale`
    /// digits from the right; `-0` normalizes to `0`).
    pub fn canonical_string(self) -> String {
        let negative = self.mantissa < 0;
        let magnitude = self.mantissa.unsigned_abs().to_string();
        let scale = self.scale as usize;
        let body = if scale == 0 {
            magnitude
        } else if magnitude.len() <= scale {
            // Zero-pad the integer side: 0.05 renders from mantissa 5.
            let padded = format!("{}{}", "0".repeat(scale - magnitude.len() + 1), magnitude);
            let split = padded.len() - scale;
            format!("{}.{}", &padded[..split], &padded[split..])
        } else {
            let split = magnitude.len() - scale;
            format!("{}.{}", &magnitude[..split], &magnitude[split..])
        };
        if negative && self.mantissa != 0 {
            format!("-{body}")
        } else {
            body
        }
    }

    /// The value rescaled to `target_scale` (mantissa multiplied by
    /// 10^(target − scale), overflow-checked). Downscales REJECT rather
    /// than round — an inexact rescale is a named refusal.
    pub fn rescaled_to(self, target_scale: u8) -> Result<Self, FixedDecimalError> {
        if target_scale > MAX_SCALE {
            return Err(FixedDecimalError::ScaleTooLarge {
                scale: target_scale,
                max: MAX_SCALE,
            });
        }
        if target_scale == self.scale {
            return Ok(self);
        }
        if target_scale < self.scale {
            // Downsizing would require division: refuse, never round.
            return Err(FixedDecimalError::MantissaOverflow);
        }
        let factor = 10_i128.checked_pow(u32::from(target_scale - self.scale));
        let Some(factor) = factor else {
            return Err(FixedDecimalError::MantissaOverflow);
        };
        let mantissa = self
            .mantissa
            .checked_mul(factor)
            .ok_or(FixedDecimalError::MantissaOverflow)?;
        Ok(Self {
            mantissa,
            scale: target_scale,
        })
    }

    /// The aligned difference `self − other` at the common (larger)
    /// scale, overflow-checked. Both operands keep their own scale; the
    /// result carries the common scale.
    pub fn checked_sub(self, other: Self) -> Result<Self, FixedDecimalError> {
        let target = self.scale.max(other.scale);
        let a = self.rescaled_to(target)?;
        let b = other.rescaled_to(target)?;
        let mantissa = a
            .mantissa
            .checked_sub(b.mantissa)
            .ok_or(FixedDecimalError::MantissaOverflow)?;
        Ok(Self {
            mantissa,
            scale: target,
        })
    }
}

// Equality and ordering compare VALUES, not renderings: operands are
// rescaled to the common (larger) scale first. An unalignable operand
// (impossible under MAX_SCALE with i128 headroom for realistic market
// values, but still checked) falls back to refusing comparison by
// panicking only where the checker cannot — instead, comparison treats an
// overflow operand pair as NOT EQUAL and unordered; the reconciliation
// layer surfaces mismatches as records, never as panics.
impl PartialEq for FixedDecimal {
    fn eq(&self, other: &Self) -> bool {
        let target = self.scale.max(other.scale);
        match (self.rescaled_to(target), other.rescaled_to(target)) {
            (Ok(a), Ok(b)) => a.mantissa == b.mantissa,
            _ => false,
        }
    }
}

impl Eq for FixedDecimal {}

impl PartialOrd for FixedDecimal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let target = self.scale.max(other.scale);
        let a = self.rescaled_to(target).ok()?;
        let b = other.rescaled_to(target).ok()?;
        a.mantissa.partial_cmp(&b.mantissa)
    }
}

/// One money amount: an exact [`FixedDecimal`] plus the DECLARED currency
/// code as data (plan §4 non-goals: currency is data; the contracts crate
/// has no currency unit). No default currency exists — a currency-less
/// money amount is unconstructable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct MoneyAmount {
    value: FixedDecimal,
    currency_code: String,
}

impl MoneyAmount {
    /// The canonical spelling of the money-record rule (pinned by tests).
    pub const NO_FLOAT_MONEY_STATEMENT: &'static str = "Every settlement-adjacent money \
         record is an exact scaling-factor decimal (i128 mantissa x 10^-scale) carrying \
         its currency code as DATA. No money value is ever stored in, or derived from, a \
         binary float: there is no From<f64> path anywhere in this module, a \
         float-derived amount is unconstructable by design, and no default currency \
         exists (phase-16 plan section 4 non-goals).";

    /// Validates the currency-code SHAPE (1–8 ASCII alphanumeric
    /// characters, no whitespace) and constructs. The code SET is the
    /// declaring source's and is carried verbatim (UNVERIFIED per source
    /// until the adapter documents the actual code set — the P15.04
    /// posture mirrored).
    pub fn new(value: FixedDecimal, currency_code: String) -> Result<Self, FixedDecimalError> {
        let len = currency_code.len();
        if len == 0
            || len > 8
            || !currency_code
                .chars()
                .all(|c| c.is_ascii_alphanumeric())
        {
            return Err(FixedDecimalError::InvalidCurrencyCode { len });
        }
        Ok(Self {
            value,
            currency_code,
        })
    }

    /// Parses wire-faithful decimal text and attaches the currency.
    pub fn parse(text: &str, currency_code: String) -> Result<Self, FixedDecimalError> {
        Self::new(FixedDecimal::parse(text)?, currency_code)
    }

    /// The exact value.
    pub fn value(&self) -> FixedDecimal {
        self.value
    }

    /// The declared currency code (data, verbatim from the source).
    pub fn currency_code(&self) -> &str {
        &self.currency_code
    }

    /// The canonical rendering including the currency, for records and
    /// audit text (never a float rendering).
    pub fn canonical_string(&self) -> String {
        format!("{} {}", self.value.canonical_string(), self.currency_code)
    }

    /// The aligned difference `self − other`, refused (never wrapped) when
    /// the currencies differ — money of two currencies is never silently
    /// mixed.
    pub fn checked_sub(self, other: Self) -> Result<MoneyAmount, FixedDecimalError> {
        if self.currency_code != other.currency_code {
            return Err(FixedDecimalError::InvalidCurrencyCode {
                len: other.currency_code.len(),
            });
        }
        let value = self.value.checked_sub(other.value)?;
        Ok(Self {
            value,
            currency_code: self.currency_code,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the parsing/rescaling rules above. Nothing is
// fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_is_exact_and_scale_preserving() {
        let d = FixedDecimal::parse("-12.50").expect("valid decimal");
        assert_eq!(d.mantissa(), -1250);
        assert_eq!(d.scale(), 2);
        assert_eq!(d.canonical_string(), "-12.50");
        // Trailing zeros are preserved: the published rendering stays.
        let d = FixedDecimal::parse("1.5").expect("valid decimal");
        assert_eq!(d.scale(), 1);
        let d = FixedDecimal::parse("1.50").expect("valid decimal");
        assert_eq!(d.scale(), 2);
        assert_eq!(d.canonical_string(), "1.50");
        // Zero-padding on the integer side.
        let d = FixedDecimal::parse("0.05").expect("valid decimal");
        assert_eq!(d.mantissa(), 5);
        assert_eq!(d.scale(), 2);
        assert_eq!(d.canonical_string(), "0.05");
    }

    #[test]
    fn value_equality_ignores_scale_differences() {
        let a = FixedDecimal::parse("1.50").expect("a");
        let b = FixedDecimal::parse("1.5").expect("b");
        assert_eq!(a, b);
        let c = FixedDecimal::parse("1.51").expect("c");
        assert_ne!(a, c);
        // Rendering still preserves the original scale.
        assert_eq!(a.canonical_string(), "1.50");
        assert_eq!(b.canonical_string(), "1.5");
    }

    #[test]
    fn negative_prices_and_zero_parse_exactly() {
        let d = FixedDecimal::parse("-0.00").expect("negative zero");
        assert!(d.is_zero());
        assert_eq!(d.canonical_string(), "0.00");
        let d = FixedDecimal::parse("+7").expect("plus sign accepted");
        assert_eq!(d.mantissa(), 7);
        assert_eq!(d.scale(), 0);
    }

    #[test]
    fn malformed_text_is_named_refusal() {
        assert_eq!(
            FixedDecimal::parse("   ").expect_err("empty refused").name(),
            "FIXED_DECIMAL_EMPTY_INPUT"
        );
        assert_eq!(
            FixedDecimal::parse(".")
                .expect_err("no digits refused")
                .name(),
            "FIXED_DECIMAL_NO_DIGITS"
        );
        assert_eq!(
            FixedDecimal::parse("1.2.3")
                .expect_err("two radix points refused")
                .name(),
            "FIXED_DECIMAL_MULTIPLE_RADIX_POINTS"
        );
        let err = FixedDecimal::parse("12x5").expect_err("letter refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_INVALID_CHARACTER");
        // The sign must be leading.
        let err = FixedDecimal::parse("1-2").expect_err("inner sign refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_INVALID_CHARACTER");
    }

    #[test]
    fn scale_over_eighteen_is_refused_never_rounded() {
        let err = FixedDecimal::parse("0.1234567890123456789")
            .expect_err("19-digit scale refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_SCALE_TOO_LARGE");
        assert!(matches!(
            err,
            FixedDecimalError::ScaleTooLarge { scale: 19, max: 18 }
        ));
    }

    #[test]
    fn rescale_up_is_exact_and_down_is_refused() {
        let d = FixedDecimal::parse("1.5").expect("d");
        let up = d.rescaled_to(3).expect("upscale");
        assert_eq!(up.mantissa(), 1500);
        assert_eq!(up.scale(), 3);
        // Downscale would need division: refuse, never round.
        let err = d.rescaled_to(0).expect_err("downscale refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_MANTISSA_OVERFLOW");
    }

    #[test]
    fn checked_sub_aligns_scales_exactly() {
        let a = FixedDecimal::parse("10.00").expect("a");
        let b = FixedDecimal::parse("7.5").expect("b");
        let diff = a.checked_sub(b).expect("aligned sub");
        assert_eq!(diff.scale(), 2);
        assert_eq!(diff.mantissa(), 250);
        assert_eq!(diff.canonical_string(), "2.50");
        // Negative deviations are exact (statement under expectation).
        let diff = b.checked_sub(a).expect("aligned sub reversed");
        assert_eq!(diff.canonical_string(), "-2.50");
    }

    #[test]
    fn money_requires_a_currency_and_carries_it_verbatim() {
        let amount = MoneyAmount::parse("125.75", "EUR".to_string()).expect("amount");
        assert_eq!(amount.canonical_string(), "125.75 EUR");
        assert_eq!(amount.currency_code(), "EUR");
        let err =
            MoneyAmount::new(FixedDecimal::parse("1.0").expect("d"), String::new())
                .expect_err("empty currency refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_INVALID_CURRENCY");
        let err = MoneyAmount::new(
            FixedDecimal::parse("1.0").expect("d"),
            "EU R".to_string(),
        )
        .expect_err("whitespace currency refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_INVALID_CURRENCY");
    }

    #[test]
    fn money_sub_refuses_mixed_currencies() {
        let eur = MoneyAmount::parse("10.00", "EUR".to_string()).expect("eur");
        let usd = MoneyAmount::parse("10.00", "USD".to_string()).expect("usd");
        let err = eur.checked_sub(usd).expect_err("mixed currencies refused");
        assert_eq!(err.name(), "FIXED_DECIMAL_INVALID_CURRENCY");
    }

    #[test]
    fn there_is_no_float_path_and_the_statement_is_pinned() {
        // The statement is the recorded rule; the structural proof is that
        // this module defines no From<f64> / TryFrom<f64> implementation
        // (a source-level invariant enforced by review) and every entry
        // path is parse/from_parts.
        assert!(MoneyAmount::NO_FLOAT_MONEY_STATEMENT.contains("no From<f64> path"));
        assert!(MoneyAmount::NO_FLOAT_MONEY_STATEMENT.contains("currency code as DATA"));
        // Ordering compares values across scales.
        let a = FixedDecimal::parse("2.5").expect("a");
        let b = FixedDecimal::parse("2.50").expect("b");
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Equal));
        let c = FixedDecimal::parse("2.6").expect("c");
        assert_eq!(a.partial_cmp(&c), Some(Ordering::Less));
    }
}
