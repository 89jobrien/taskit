pub mod config;
pub mod output_format;
pub mod pipeline_runner;
pub mod step;

#[cfg(test)]
mod tests {
    #[test]
    fn core_crate_compiles() {
        let cfg = crate::config::Config::default();
        assert!(cfg.ci.is_none());
    }
}
