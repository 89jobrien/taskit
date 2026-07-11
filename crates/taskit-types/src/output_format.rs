/// Output format for pipeline results.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OutputFormat {
    #[default]
    Human,
    /// One line per step; expands failed steps when verbose_on_failure is set.
    Compact,
    Json,
    Github,
    Junit,
    Diagnostic,
    Sarif,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;

    #[test]
    fn default_is_human() {
        assert!(matches!(OutputFormat::default(), OutputFormat::Human));
    }

    #[test]
    fn all_variants_are_distinct() {
        let variants = [
            OutputFormat::Human,
            OutputFormat::Compact,
            OutputFormat::Json,
            OutputFormat::Github,
            OutputFormat::Junit,
            OutputFormat::Diagnostic,
            OutputFormat::Sarif,
        ];
        // Verify each variant has a distinct debug representation
        let names: Vec<String> = variants.iter().map(|v| format!("{v:?}")).collect();
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "all variants should be distinct");
    }

    #[test]
    fn value_enum_string_round_trips() {
        for name in [
            "human",
            "compact",
            "json",
            "github",
            "junit",
            "diagnostic",
            "sarif",
        ] {
            let parsed =
                OutputFormat::from_str(name, true).expect("known output format should parse");
            let possible = parsed
                .to_possible_value()
                .expect("output format should have a clap possible value");
            assert_eq!(possible.get_name(), name);
        }
    }
}
