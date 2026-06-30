use clap::Args as ClapArgs;

#[derive(ClapArgs)]
pub struct Args {
    /// Lint a specific crate.
    #[arg(long)]
    crate_name: Option<String>,
    /// Only lint affected crates.
    #[arg(long)]
    affected: bool,
}

pub fn run(args: Args) -> i32 {
    let mut cmd = vec!["lint"];
    let crate_flag;
    if let Some(ref name) = args.crate_name {
        cmd.push("--crate-name");
        crate_flag = name.as_str();
        cmd.push(crate_flag);
    }
    if args.affected {
        cmd.push("--affected");
    }
    crate::taskit(&cmd)
}
