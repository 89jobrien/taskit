#![no_main]
use libfuzzer_sys::fuzz_target;
use taskit_init::plan::{CiStepPlan, CratePlan, InitPlan, SurfacePlan};
use taskit_types::config::PropagationEntry;

// Fuzz the render_toml round-trip in two ways:
//
// 1. Arbitrary bytes -> UTF-8 -> `toml::from_str::<Config>`: must never panic.
// 2. Construct an `InitPlan` from slices of the fuzz input, call `render_toml`,
//    and verify the output is valid TOML syntax (not necessarily a valid Config).

fuzz_target!(|data: &[u8]| {
    // --- Approach 1: fuzz the TOML parser with arbitrary strings ---
    if let Ok(s) = std::str::from_utf8(data) {
        let _: Result<taskit_types::config::Config, _> = toml::from_str(s);
    }

    // --- Approach 2: fuzz render_toml with a hand-constructed InitPlan ---
    let plan = build_plan_from_bytes(data);
    let rendered = taskit_init::render_toml::render_toml(&plan);

    // The output must always be syntactically valid TOML.
    let parsed: Result<toml::Value, _> = toml::from_str(&rendered);
    assert!(
        parsed.is_ok(),
        "render_toml produced invalid TOML: {:?}\n---\n{}",
        parsed.err(),
        rendered
    );
});

/// Derive an `InitPlan` from raw bytes without panicking.
///
/// Strings are extracted as 0-32 byte windows from `data`.
/// Any byte outside `[A-Za-z0-9_./-]` is replaced with `x`.
/// All string fields are sanitised so they contain no TOML-breaking characters
/// (double-quotes, backslashes, newlines) before being embedded into key-value
/// pairs by `render_toml`.
fn build_plan_from_bytes(data: &[u8]) -> InitPlan {
    // Build a safe ASCII string from the input by keeping only printable
    // non-special bytes. This is intentionally lossy — we want render_toml
    // to handle strings it receives without panicking, not to test that it
    // handles TOML injection (that is a property of the caller's contract).
    let safe = |start: usize, len: usize| -> String {
        data.iter()
            .cycle()
            .skip(start)
            .take(len.min(32))
            .map(|&b| {
                if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'/' || b == b'.' {
                    b as char
                } else {
                    'x'
                }
            })
            .collect::<String>()
            // Ensure non-empty so render_toml always has something to write
            .trim_end_matches('x')
            .to_string()
            + "a"
    };

    let byte = |idx: usize| -> u8 { data.get(idx % data.len().max(1)).copied().unwrap_or(0) };

    let num_crates = (byte(0) % 4) as usize + 1;
    let crates: Vec<CratePlan> = (0..num_crates)
        .map(|i| CratePlan {
            dir: safe(i * 3, 8),
            pkg: if byte(i + 1) % 2 == 0 {
                Some(safe(i * 5 + 2, 8))
            } else {
                None
            },
        })
        .collect();

    let num_surfaces = (byte(1) % 3) as usize;
    let surfaces: Vec<SurfacePlan> = (0..num_surfaces)
        .map(|i| SurfacePlan {
            name: safe(i * 7, 8),
            path: safe(i * 9 + 1, 12),
        })
        .collect();

    let num_propagation = (byte(2) % 3) as usize;
    let propagation: Vec<PropagationEntry> = (0..num_propagation)
        .map(|i| PropagationEntry {
            source: safe(i * 11, 8),
            dependents: (0..(byte(i + 3) % 3) as usize + 1)
                .map(|j| safe(i * 13 + j, 8))
                .collect(),
        })
        .collect();

    let num_steps = (byte(3) % 4) as usize;
    let ci_steps: Vec<CiStepPlan> = (0..num_steps)
        .map(|i| CiStepPlan {
            name: safe(i * 17, 8),
            cmd: safe(i * 19 + 1, 8),
            gate: byte(i + 4) % 2 == 0,
        })
        .collect();

    let offline_skip = if byte(5) % 4 == 0 {
        Some(safe(10, 16))
    } else {
        None
    };

    let flow = if byte(6) % 3 == 0 {
        Some(taskit_init::plan::FlowPlan {
            main: safe(20, 8),
            staging: safe(21, 8),
            release: safe(22, 8),
        })
    } else {
        None
    };

    let release = if byte(7) % 3 == 0 {
        Some(taskit_init::plan::ReleasePlan {
            github_repo: if byte(8) % 2 == 0 {
                Some(safe(30, 16))
            } else {
                None
            },
            publish_order: (0..(byte(9) % 3) as usize)
                .map(|i| safe(i * 23 + 5, 8))
                .collect(),
        })
    } else {
        None
    };

    InitPlan {
        crates,
        propagation,
        surfaces,
        coverage: None,
        ci_steps,
        offline_skip,
        flow,
        release,
        git_hooks: false,
        github_ci: false,
        deny_toml: false,
        ctx_scaffold: false,
        mdbook: false,
    }
}
