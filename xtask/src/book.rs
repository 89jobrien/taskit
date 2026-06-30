use clap::Args as ClapArgs;
use std::process::Command;

#[derive(ClapArgs)]
pub struct Args {
    /// Serve the book locally with live reload.
    #[arg(long)]
    serve: bool,
    /// Port for the dev server.
    #[arg(long, default_value = "3000")]
    port: u16,
}

pub fn run(args: Args) -> i32 {
    let subcmd = if args.serve { "serve" } else { "build" };
    let mut cmd = Command::new("mdbook");
    cmd.arg(subcmd).arg("docs/");
    if args.serve {
        cmd.args(["--port", &args.port.to_string()]);
    }
    match cmd.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("mdbook not found — install with: cargo install mdbook");
            1
        }
        Err(e) => {
            eprintln!("failed to run mdbook: {e}");
            1
        }
    }
}
