use std::fmt;
use std::str::FromStr;

/// Output format for pipeline results.
///
/// This is a plain domain enum: CLI parsing goes through [`FromStr`] so the
/// leaf types crate stays free of CLI-framework dependencies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Github,
    Junit,
    Diagnostic,
    Sarif,
}

impl OutputFormat {
    /// All variants with their canonical names, for parsing and help text.
    pub const ALL: &[(Self, &str)] = &[
        (Self::Human, "human"),
        (Self::Json, "json"),
        (Self::Github, "github"),
        (Self::Junit, "junit"),
        (Self::Diagnostic, "diagnostic"),
        (Self::Sarif, "sarif"),
    ];

    pub fn as_str(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(v, _)| *v == self)
            .map(|(_, name)| *name)
            .unwrap_or("human")
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|(_, name)| name.eq_ignore_ascii_case(s))
            .map(|(v, _)| *v)
            .ok_or_else(|| {
                let expected: Vec<&str> = Self::ALL.iter().map(|(_, n)| *n).collect();
                format!(
                    "invalid output format {s:?} (expected one of: {})",
                    expected.join(", ")
                )
            })
    }
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
        let names: Vec<&str> = OutputFormat::ALL.iter().map(|(_, n)| *n).collect();
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "all variants should be distinct");
    }

    #[test]
    fn from_str_round_trips_every_variant() {
        for (variant, name) in OutputFormat::ALL {
            assert_eq!(name.parse::<OutputFormat>().unwrap(), *variant);
            assert_eq!(variant.to_string(), *name);
        }
    }

    #[test]
    fn from_str_is_case_insensitive() {
        assert_eq!("JSON".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    }

    #[test]
    fn from_str_rejects_unknown() {
        let err = "xml".parse::<OutputFormat>().unwrap_err();
        assert!(err.contains("invalid output format"));
        assert!(err.contains("human"), "error should list valid formats");
    }
}
