//! Demonstrates `plan_from_discovery` — inspect what `taskit init` would
//! generate for the current workspace without writing any files.
//!
//! Run from the taskit workspace root:
//!
//!   cargo run -p taskit-init --example discover_plan

use taskit_init::plan::plan_from_discovery;

fn main() {
    let plan = match plan_from_discovery() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("discovery failed: {e}");
            std::process::exit(1);
        }
    };

    println!("Discovered workspace plan");
    println!("{}", "─".repeat(50));

    println!("\nCrates ({}):", plan.crates.len());
    for c in &plan.crates {
        let pkg = c.pkg.as_deref().unwrap_or("(unnamed)");
        println!("  {pkg}  ({})", c.dir);
    }

    if !plan.propagation.is_empty() {
        println!("\nPropagation rules:");
        for p in &plan.propagation {
            println!("  {} → {:?}", p.source, p.dependents);
        }
    }

    if !plan.surfaces.is_empty() {
        println!("\nProtocol surfaces:");
        for s in &plan.surfaces {
            println!("  {} @ {}", s.name, s.path);
        }
    }

    if !plan.ci_steps.is_empty() {
        println!("\nCI steps:");
        for s in &plan.ci_steps {
            let gate = if s.gate { " [gate]" } else { "" };
            println!("  {}{gate}  →  {}", s.name, s.cmd);
        }
    }

    if let Some(ref cov) = plan.coverage {
        println!(
            "\nCoverage: {} (threshold: {}%)",
            cov.crate_name, cov.threshold
        );
    }

    if let Some(ref flow) = plan.flow {
        println!(
            "\nFlow branches: main={} staging={} release={}",
            flow.main, flow.staging, flow.release
        );
    }

    println!("\nScaffold flags:");
    println!("  git_hooks:   {}", plan.git_hooks);
    println!("  github_ci:   {}", plan.github_ci);
    println!("  deny_toml:   {}", plan.deny_toml);
    println!("  ctx_scaffold:{}", plan.ctx_scaffold);
    println!("  mdbook:      {}", plan.mdbook);
}
