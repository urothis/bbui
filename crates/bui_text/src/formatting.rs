//! Stable significant-digit formatting for numeric controls.

/// Format a finite float with the requested number of significant digits.
/// Small values stay visible, trailing fractional zeroes are removed, and very
/// large or very small magnitudes use compact scientific notation.
pub fn format_float(value: f64, significant_digits: usize) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "∞".into();
    }
    if value == f64::NEG_INFINITY {
        return "−∞".into();
    }
    if value == 0.0 {
        return "0".into();
    }

    let digits = significant_digits.clamp(1, 17);
    for precision in digits..=17 {
        let formatted = format_finite_candidate(value, precision);
        if formatted
            .parse::<f64>()
            .is_ok_and(|parsed| parsed.is_finite() && parsed != 0.0)
        {
            return formatted;
        }
    }

    // The shortest round-trip representation of every finite f64 is finite. This is a defensive
    // fallback if formatting behavior changes or the candidate loop cannot produce one.
    value.to_string()
}

fn format_finite_candidate(value: f64, significant_digits: usize) -> String {
    let exponent = value.abs().log10().floor() as i32;
    if !(-4..9).contains(&exponent) {
        return scientific(value, significant_digits);
    }

    let decimal_places = (significant_digits as i32 - 1 - exponent).max(0) as usize;
    trim_fraction(format!("{value:.decimal_places$}"))
}

fn scientific(value: f64, significant_digits: usize) -> String {
    let scientific = format!("{:.*e}", significant_digits.saturating_sub(1), value);
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let mantissa = trim_fraction(mantissa.to_owned());
    let exponent: i32 = exponent.parse().unwrap_or(0);
    format!("{mantissa}e{exponent}")
}

fn trim_fraction(mut value: String) -> String {
    if value.contains('.') {
        while value.ends_with('0') {
            value.pop();
        }
        if value.ends_with('.') {
            value.pop();
        }
    }
    if value == "-0" { "0".into() } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_significant_digits_without_collapsing_small_values() {
        assert_eq!(format_float(1.234567, 5), "1.2346");
        assert_eq!(format_float(12.0, 6), "12");
        assert_eq!(format_float(0.00001234, 4), "1.234e-5");
        assert_eq!(format_float(123_456_789_000.0, 4), "1.235e11");
        assert_eq!(format_float(-0.00000002, 3), "-2e-8");
        assert_eq!(format_float(-0.0, 6), "0");
    }

    #[test]
    fn formats_special_values_and_clamps_precision() {
        assert_eq!(format_float(f64::INFINITY, 6), "∞");
        assert_eq!(format_float(f64::NEG_INFINITY, 6), "−∞");
        assert_eq!(format_float(f64::NAN, 6), "NaN");
        assert_eq!(format_float(1.234, 0), "1");
        assert_eq!(format_float(1.234, 99), "1.234");
    }

    #[test]
    fn significant_display_round_trips_across_magnitudes() {
        for exponent in (-300..=300).step_by(7) {
            for mantissa in [1.000_001, 1.234_567_89, 5.432_198_76, 9.999_999] {
                let value = mantissa * 10f64.powi(exponent);
                if !value.is_finite() || value == 0.0 {
                    continue;
                }
                for signed in [value, -value] {
                    let shown = format_float(signed, 6);
                    let parsed: f64 = shown.parse().expect("display must parse as a float");
                    let relative_error = ((parsed - signed) / signed).abs();
                    assert!(
                        relative_error <= 0.000_005_1,
                        "{signed} -> {shown} -> {parsed}: relative error {relative_error}"
                    );
                }
            }
        }
    }

    #[test]
    fn finite_extremes_always_format_as_finite_expression_values() {
        let smallest_subnormal = f64::from_bits(1);
        for value in [
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            smallest_subnormal,
            -f64::MIN_POSITIVE,
            -smallest_subnormal,
        ] {
            for digits in [1, 2, 6, 17] {
                let shown = format_float(value, digits);
                let parsed = shown.parse::<f64>().expect("formatted value parses");
                assert!(parsed.is_finite(), "{value} -> {shown} -> {parsed}");
                assert_ne!(parsed, 0.0, "nonzero {value} was erased by {shown}");
                let evaluated = crate::expression::evaluate(&shown, &Default::default())
                    .expect("formatted value remains accepted by numeric expressions");
                assert!(evaluated.is_finite(), "{value} -> {shown} -> {evaluated}");
            }
        }
    }
}
