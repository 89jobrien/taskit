use taskit_types::step::PipelineOutcome;

/// Port: formats pipeline results for different output targets.
///
/// Adapters: `HumanFormatter`, `JsonFormatter`, `GithubFormatter`,
/// `JunitFormatter`, `DiagnosticFormatter`, `SarifFormatter` (taskit-engine).
pub trait OutputFormatter {
    fn render(&self, outcome: &PipelineOutcome) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskit_types::step::{StepResult, StepStatus};

    struct NameListFormatter;

    impl OutputFormatter for NameListFormatter {
        fn render(&self, outcome: &PipelineOutcome) -> String {
            outcome
                .results
                .iter()
                .map(|r| r.name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    #[test]
    fn custom_formatter_satisfies_trait() {
        let outcome = PipelineOutcome {
            results: vec![StepResult {
                name: "fmt".into(),
                status: StepStatus::Pass,
                duration: std::time::Duration::ZERO,
                error: None,
                gate: false,
                diagnostics: vec![],
            }],
            total: std::time::Duration::ZERO,
            passed: true,
        };
        assert_eq!(NameListFormatter.render(&outcome), "fmt");
    }
}
