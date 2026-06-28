pub mod affected;
pub mod audit;
pub mod cache;
pub mod check_deps;
pub mod check_freshness;
pub mod ci;
pub mod clean;
pub mod config;
pub mod dev_setup;
pub mod discovery;
pub mod fmt;
pub mod health;
pub mod hooks;
pub mod inspect;
pub mod lint;
pub mod output;
pub mod pipeline_runner;
pub mod progress;
pub mod protocol;
pub mod publish;
pub mod quick;
pub mod runner;
pub mod step;
pub mod testing;
pub mod update_claude;
pub mod util;
pub mod version;

/// Resolved workspace root and parsed config.
#[derive(Debug)]
pub struct Workspace {
    pub root: std::path::PathBuf,
    pub config: taskit_types::config::Config,
}

#[cfg(test)]
mod tests {
    #[test]
    fn engine_crate_compiles() {
        // Verify the crate's public surface is accessible
        let _ws = super::Workspace {
            root: std::path::PathBuf::from("."),
            config: taskit_types::config::Config::default(),
        };
    }
}
