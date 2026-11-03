//! xtask — build tasks for this workspace.
//!
//! Run with: `cargo xtask <task>`
//! Tasks are delegated to the `taskit` binary.

// --- taskit-managed module declaration ---
mod taskit;
// --- end taskit-managed module declaration ---

fn main() {
    // --- taskit-managed dispatch ---
    if taskit::dispatch() {
        return;
    }
    // --- end taskit-managed dispatch ---

    let task = std::env::args().nth(1).unwrap_or_default();
    eprintln!("unknown task: {task}");
    eprintln!("available: fmt, fmt-check, lint, test, ci, pre-commit, pre-push");
    std::process::exit(1);
}
