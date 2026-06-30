use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Check formatting without writing.
    #[arg(long)]
    check: bool,
    /// Only format affected crates.
    #[arg(long)]
    affected: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["fmt"];
    if args.check {
        cmd.push("--check");
    }
    if args.affected {
        cmd.push("--affected");
    }
    crate::taskit(&cmd)
}
