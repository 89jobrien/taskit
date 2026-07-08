/// Run a block inside a temporary directory with automatic cleanup.
///
/// Two forms:
/// ```ignore
/// in_temp_dir! { /* body */ }
/// in_temp_dir! { dir => /* body using dir: &Path */ }
/// ```
#[macro_export]
macro_rules! in_temp_dir {
    { $dir:ident => $($body:tt)* } => {{
        let _guard = $crate::TempDirGuard::new();
        let $dir = _guard.path();
        $($body)*
    }};
    { $($body:tt)* } => {{
        let _guard = $crate::TempDirGuard::new();
        $($body)*
    }};
}

/// Construct a `StepResult` with sensible defaults.
///
/// ```ignore
/// step_result!("lint", Pass)
/// step_result!("test", Fail, error: "assertion failed")
/// step_result!("gate", Pass, gate: true)
/// step_result!("slow", Pass, duration: Duration::from_secs(5))
/// step_result!("full", Fail, error: "err", gate: true, duration: Duration::from_secs(2))
/// ```
#[macro_export]
macro_rules! step_result {
    ($name:expr, $status:ident) => {
        $crate::__step_result_inner!($name, $status, None, false, ::std::time::Duration::ZERO)
    };
    ($name:expr, $status:ident, error: $err:expr) => {
        $crate::__step_result_inner!(
            $name,
            $status,
            Some($err.to_string()),
            false,
            ::std::time::Duration::ZERO
        )
    };
    ($name:expr, $status:ident, gate: $gate:expr) => {
        $crate::__step_result_inner!($name, $status, None, $gate, ::std::time::Duration::ZERO)
    };
    ($name:expr, $status:ident, duration: $dur:expr) => {
        $crate::__step_result_inner!($name, $status, None, false, $dur)
    };
    ($name:expr, $status:ident, error: $err:expr, gate: $gate:expr) => {
        $crate::__step_result_inner!(
            $name,
            $status,
            Some($err.to_string()),
            $gate,
            ::std::time::Duration::ZERO
        )
    };
    ($name:expr, $status:ident, error: $err:expr, gate: $gate:expr, duration: $dur:expr) => {
        $crate::__step_result_inner!($name, $status, Some($err.to_string()), $gate, $dur)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __step_result_inner {
    ($name:expr, $status:ident, $error:expr, $gate:expr, $dur:expr) => {
        ::taskit_types::step::StepResult {
            name: $name.to_string(),
            status: ::taskit_types::step::StepStatus::$status,
            duration: $dur,
            error: $error,
            gate: $gate,
            diagnostics: vec![],
            context: Default::default(),
        }
    };
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use taskit_types::step::{StepResult, StepStatus};

    #[test]
    fn step_result_basic() {
        let r: StepResult = step_result!("lint", Pass);
        assert_eq!(r.name, "lint");
        assert_eq!(r.status, StepStatus::Pass);
        assert!(r.error.is_none());
        assert!(!r.gate);
        assert_eq!(r.duration, Duration::ZERO);
    }

    #[test]
    fn step_result_with_error() {
        let r = step_result!("test", Fail, error: "assertion failed");
        assert_eq!(r.status, StepStatus::Fail);
        assert_eq!(r.error.as_deref(), Some("assertion failed"));
    }

    #[test]
    fn step_result_with_gate() {
        let r = step_result!("preflight", Pass, gate: true);
        assert!(r.gate);
    }

    #[test]
    fn step_result_with_duration() {
        let r = step_result!("slow", Pass, duration: Duration::from_secs(5));
        assert_eq!(r.duration, Duration::from_secs(5));
    }

    #[test]
    fn step_result_with_error_and_gate() {
        let r = step_result!("gate", Fail, error: "bad", gate: true);
        assert!(r.gate);
        assert_eq!(r.error.as_deref(), Some("bad"));
    }

    #[test]
    fn step_result_full() {
        let r =
            step_result!("full", Fail, error: "err", gate: true, duration: Duration::from_secs(2));
        assert_eq!(r.name, "full");
        assert_eq!(r.status, StepStatus::Fail);
        assert_eq!(r.error.as_deref(), Some("err"));
        assert!(r.gate);
        assert_eq!(r.duration, Duration::from_secs(2));
    }

    #[test]
    fn in_temp_dir_basic() {
        let before = std::env::current_dir().unwrap();
        in_temp_dir! {
            std::fs::write("test.txt", "hello").unwrap();
            assert!(std::path::Path::new("test.txt").exists());
        }
        let after = std::env::current_dir().unwrap();
        assert_eq!(after, before);
    }

    #[test]
    fn in_temp_dir_with_binding() {
        in_temp_dir! { dir =>
            assert!(dir.exists());
            assert!(dir.is_dir());
        }
    }
}
