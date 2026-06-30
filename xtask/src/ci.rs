use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Stop on first failure.
    #[arg(long)]
    fail_fast: bool,
    /// Include network-dependent tests.
    #[arg(long)]
    include_network: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["ci"];
    if args.fail_fast {
        cmd.push("--fail-fast");
    }
    if args.include_network {
        cmd.push("--include-network");
    }
    crate::taskit(&cmd)
}
