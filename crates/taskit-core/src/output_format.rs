/// Output format for pipeline results.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Github,
    Junit,
    Diagnostic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_human() {
        assert!(matches!(OutputFormat::default(), OutputFormat::Human));
    }

    #[test]
    fn all_variants_exist() {
        let _variants = [
            OutputFormat::Human,
            OutputFormat::Json,
            OutputFormat::Github,
            OutputFormat::Junit,
            OutputFormat::Diagnostic,
        ];
    }
}
