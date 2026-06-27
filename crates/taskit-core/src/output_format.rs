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
    fn all_variants_are_distinct() {
        let variants = [
            OutputFormat::Human,
            OutputFormat::Json,
            OutputFormat::Github,
            OutputFormat::Junit,
            OutputFormat::Diagnostic,
        ];
        // Verify each variant has a distinct debug representation
        let names: Vec<String> = variants.iter().map(|v| format!("{v:?}")).collect();
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "all variants should be distinct");
    }
}
