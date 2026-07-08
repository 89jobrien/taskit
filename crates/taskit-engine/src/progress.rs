use std::time::Duration;

/// Format an elapsed duration as a human-readable string.
pub fn fmt_elapsed(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        let m = d.as_secs() / 60;
        let s = d.as_secs() % 60;
        format!("{m}m{s:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_elapsed_sub_minute() {
        assert_eq!(fmt_elapsed(Duration::from_millis(1234)), "1.2s");
    }

    #[test]
    fn fmt_elapsed_exactly_one_minute() {
        assert_eq!(fmt_elapsed(Duration::from_secs(60)), "1m00s");
    }

    #[test]
    fn fmt_elapsed_over_one_minute() {
        assert_eq!(fmt_elapsed(Duration::from_secs(75)), "1m15s");
    }

    #[test]
    fn fmt_elapsed_zero() {
        let s = fmt_elapsed(Duration::ZERO);
        assert!(s.ends_with('s'), "expected seconds suffix, got {s}");
    }
}
