use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Test a specific crate.
    #[arg(long)]
    crate_name: Option<String>,
    /// Only test affected crates.
    #[arg(long)]
    affected: bool,
    /// Skip network-dependent tests.
    #[arg(long)]
    offline: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["test"];
    let crate_flag;
    if let Some(ref name) = args.crate_name {
        cmd.push("--crate-name");
        crate_flag = name.as_str();
        cmd.push(crate_flag);
    }
    if args.affected {
        cmd.push("--affected");
    }
    if args.offline {
        cmd.push("--offline");
    }
    crate::taskit(&cmd)
}
