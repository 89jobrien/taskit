use std::time::{Duration, Instant};

use indicatif::{ProgressBar, ProgressStyle};

const TICK_STRINGS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""];
const TICK_INTERVAL: Duration = Duration::from_millis(80);

/// A spinner that wraps a single labeled operation.
///
/// Call [`finish_ok`] or [`finish_err`] to complete it.  On non-TTY output
/// (e.g. CI logs) `indicatif` degrades to a single status line printed on
/// completion, so no interleaved carriage-return noise appears in log files.
pub struct Spinner {
    bar: ProgressBar,
    label: String,
    started: Instant,
}

impl Spinner {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("valid template")
                .tick_strings(TICK_STRINGS),
        );
        bar.enable_steady_tick(TICK_INTERVAL);
        bar.set_message(label.clone());
        Self {
            bar,
            label,
            started: Instant::now(),
        }
    }

    pub fn finish_ok(self) {
        let elapsed = fmt_elapsed(self.started.elapsed());
        self.bar
            .finish_with_message(format!("✓ {} [{elapsed}]", self.label));
    }

    pub fn finish_err(self) {
        let elapsed = fmt_elapsed(self.started.elapsed());
        self.bar
            .finish_with_message(format!("✗ {} [{elapsed}]", self.label));
    }
}

/// Run `f`, wrapping it with a spinner labeled `label`.
///
/// Returns the result of `f` unchanged; the spinner finishes with ✓ or ✗.
pub fn with_spinner<T, E, F>(label: impl Into<String>, f: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let sp = Spinner::new(label);
    match f() {
        Ok(v) => {
            sp.finish_ok();
            Ok(v)
        }
        Err(e) => {
            sp.finish_err();
            Err(e)
        }
    }
}

fn fmt_elapsed(d: Duration) -> String {
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

    #[test]
    fn with_spinner_propagates_ok() {
        let result = with_spinner("test-ok", || Ok::<i32, anyhow::Error>(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn with_spinner_propagates_err() {
        let result: Result<(), anyhow::Error> =
            with_spinner("test-err", || Err(anyhow::anyhow!("boom")));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "boom");
    }
}
