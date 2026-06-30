mod book;
mod ci;
mod fmt;
mod lint;
mod pre_commit;
mod pre_push;
mod test;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Workspace task runner (delegates to taskit)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build or serve the mdBook documentation.
    Book(book::Args),
    /// Run the full CI pipeline.
    Ci(ci::Args),
    /// Format all Rust code.
    Fmt(fmt::Args),
    /// Run clippy lints.
    Lint(lint::Args),
    /// Run tests via nextest.
    Test(test::Args),
    /// Pre-commit hook delegate.
    PreCommit,
    /// Pre-push hook delegate.
    PrePush,
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Command::Book(args) => book::run(args),
        Command::Ci(args) => ci::run(args),
        Command::Fmt(args) => fmt::run(args),
        Command::Lint(args) => lint::run(args),
        Command::Test(args) => test::run(args),
        Command::PreCommit => pre_commit::run(),
        Command::PrePush => pre_push::run(),
    };
    std::process::exit(code);
}

/// Run `taskit` with the given args. Installs it automatically if missing.
pub fn taskit(args: &[&str]) -> i32 {
    use std::process::Command;

    match Command::new("taskit").args(args).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("taskit not found, installing via cargo install...");
            let install = Command::new("cargo")
                .args(["install", "taskit"])
                .status()
                .expect("failed to run cargo install");
            if !install.success() {
                eprintln!("failed to install taskit");
                return 1;
            }
            Command::new("taskit")
                .args(args)
                .status()
                .map(|s| s.code().unwrap_or(1))
                .unwrap_or(1)
        }
        Err(e) => {
            eprintln!("failed to run taskit: {e}");
            1
        }
    }
}
