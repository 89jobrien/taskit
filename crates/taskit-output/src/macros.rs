/// Emit a progress message through the active sink.
#[macro_export]
macro_rules! taskit_progress {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::Progress(format!($($arg)*)));
    };
}

/// Emit a skip message through the active sink.
#[macro_export]
macro_rules! taskit_skip {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::Skip(format!($($arg)*)));
    };
}

/// Emit a dry-run message through the active sink.
#[macro_export]
macro_rules! taskit_dry {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::DryRun(format!($($arg)*)));
    };
}

/// Emit a success message through the active sink.
#[macro_export]
macro_rules! taskit_ok {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::Success(format!($($arg)*)));
    };
}

/// Emit an error message through the active sink.
#[macro_export]
macro_rules! taskit_err {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::Error(format!($($arg)*)));
    };
}

/// Emit a warning message through the active sink.
#[macro_export]
macro_rules! taskit_warn {
    ($($arg:tt)*) => {
        $crate::sink().emit(&$crate::Message::Progress(format!("warning: {}", format!($($arg)*))));
    };
}
