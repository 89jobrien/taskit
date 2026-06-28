use anyhow::Result;
use taskit_types::config::PropagationEntry;

/// Intermediate representation of what taskit init will generate.
#[derive(Debug, Clone)]
pub struct InitPlan {
    pub crates: Vec<CratePlan>,
    pub propagation: Vec<PropagationEntry>,
    pub surfaces: Vec<SurfacePlan>,
    pub coverage: Option<CoveragePlan>,
    pub ci_steps: Vec<CiStepPlan>,
    pub offline_skip: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CratePlan {
    pub dir: String,
    pub pkg: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SurfacePlan {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct CoveragePlan {
    pub crate_name: String,
    pub threshold: f64,
}

#[derive(Debug, Clone)]
pub struct CiStepPlan {
    pub name: String,
    pub cmd: String,
    pub gate: bool,
}

impl InitPlan {
    /// The default CI steps matching the built-in pipeline.
    pub fn default_steps() -> Vec<CiStepPlan> {
        vec![
            CiStepPlan {
                name: "self-check".into(),
                cmd: "self-check".into(),
                gate: true,
            },
            CiStepPlan {
                name: "fmt --check".into(),
                cmd: "fmt --check".into(),
                gate: false,
            },
            CiStepPlan {
                name: "lint".into(),
                cmd: "lint".into(),
                gate: false,
            },
            CiStepPlan {
                name: "compile-tests".into(),
                cmd: "compile-tests".into(),
                gate: false,
            },
            CiStepPlan {
                name: "test".into(),
                cmd: "test".into(),
                gate: false,
            },
            CiStepPlan {
                name: "check-deps".into(),
                cmd: "check-deps".into(),
                gate: false,
            },
            CiStepPlan {
                name: "check-protocol-drift".into(),
                cmd: "check-protocol-drift".into(),
                gate: false,
            },
        ]
    }
}

/// Build an InitPlan from cargo metadata discovery.
pub fn plan_from_discovery() -> Result<InitPlan> {
    let metadata = cargo_metadata_members()?;

    let crates: Vec<CratePlan> = metadata
        .iter()
        .map(|m| CratePlan {
            dir: m.dir.clone(),
            // TODO: for root crates (dir="."), pkg never equals dir so pkg is
            // always Some — consider special-casing root crates
            pkg: if m.pkg == m.dir {
                None
            } else {
                Some(m.pkg.clone())
            },
        })
        .collect();

    Ok(InitPlan {
        crates,
        propagation: vec![],
        surfaces: vec![],
        coverage: None,
        ci_steps: InitPlan::default_steps(),
        offline_skip: None,
    })
}

/// Build an InitPlan interactively using dialoguer prompts.
pub fn plan_interactive() -> Result<InitPlan> {
    use dialoguer::{Confirm, Input};

    let mut plan = plan_from_discovery()?;

    let add_coverage = Confirm::new()
        .with_prompt("Add coverage configuration?")
        .default(false)
        .interact()?;

    if add_coverage {
        let crate_name: String = Input::new()
            .with_prompt("Coverage crate name")
            .interact_text()?;
        let threshold: f64 = Input::new()
            .with_prompt("Coverage threshold (%)")
            .default(80.0)
            .interact_text()?;
        plan.coverage = Some(CoveragePlan {
            crate_name,
            threshold,
        });
    }

    let offline_skip: String = Input::new()
        .with_prompt("Offline skip expression (empty for none)")
        .default(String::new())
        .interact_text()?;
    if !offline_skip.is_empty() {
        plan.offline_skip = Some(offline_skip);
    }

    Ok(plan)
}

struct DiscoveredMember {
    dir: String,
    pkg: String,
}

fn cargo_metadata_members() -> Result<Vec<DiscoveredMember>> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|e| anyhow::anyhow!("cargo metadata failed: {e}"))?;

    let ws_root = metadata.workspace_root.as_std_path();

    Ok(metadata
        .workspace_members
        .iter()
        .filter_map(|id| {
            metadata.packages.iter().find(|p| &p.id == id).map(|p| {
                let manifest = p.manifest_path.as_std_path();
                let pkg_dir = manifest.parent().unwrap_or(manifest);
                let dir = pkg_dir
                    .strip_prefix(ws_root)
                    .unwrap_or(pkg_dir)
                    .to_string_lossy()
                    .into_owned();
                DiscoveredMember {
                    dir,
                    pkg: p.name.clone(),
                }
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ci_steps_match_builtin_pipeline() {
        let plan = InitPlan::default_steps();
        let names: Vec<&str> = plan.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"self-check"));
        assert!(names.contains(&"fmt --check"));
        assert!(names.contains(&"lint"));
        assert!(names.contains(&"test"));
        assert!(names.contains(&"check-deps"));
    }

    #[test]
    fn default_ci_steps_first_is_gate() {
        let plan = InitPlan::default_steps();
        assert!(plan[0].gate);
    }

    #[test]
    fn default_ci_steps_only_first_is_gate() {
        let plan = InitPlan::default_steps();
        for step in &plan[1..] {
            assert!(!step.gate, "{} should not be a gate", step.name);
        }
    }

    #[test]
    fn plan_from_discovery_returns_crates() {
        // This test runs in the taskit workspace, so it should find crates
        let plan = plan_from_discovery().unwrap();
        assert!(!plan.crates.is_empty());
    }

    #[test]
    fn plan_from_discovery_has_default_steps() {
        let plan = plan_from_discovery().unwrap();
        assert!(!plan.ci_steps.is_empty());
        assert_eq!(plan.ci_steps[0].name, "self-check");
    }
}
