use taskit_types::config::PropagationEntry;
use taskit_types::error::TaskitError;

/// Intermediate representation of what taskit init will generate.
#[derive(Debug, Clone)]
pub struct InitPlan {
    pub crates: Vec<CratePlan>,
    pub propagation: Vec<PropagationEntry>,
    pub surfaces: Vec<SurfacePlan>,
    pub coverage: Option<CoveragePlan>,
    pub ci_steps: Vec<CiStepPlan>,
    pub offline_skip: Option<String>,
    pub flow: Option<FlowPlan>,
    pub git_hooks: bool,
    pub github_ci: bool,
    pub deny_toml: bool,
    pub ctx_scaffold: bool,
    pub mdbook: bool,
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

#[derive(Debug, Clone)]
pub struct FlowPlan {
    pub main: String,
    pub staging: String,
    pub release: String,
}

impl Default for FlowPlan {
    fn default() -> Self {
        Self {
            main: "main".into(),
            staging: "staging".into(),
            release: "release".into(),
        }
    }
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
pub fn plan_from_discovery() -> Result<InitPlan, TaskitError> {
    let metadata = cargo_metadata_members()?;

    let crates: Vec<CratePlan> = metadata
        .iter()
        .map(|m| CratePlan {
            dir: m.dir.clone(),
            pkg: if m.pkg == m.dir {
                None
            } else {
                Some(m.pkg.clone())
            },
        })
        .collect();

    let propagation = infer_propagation(&metadata);
    let surfaces = detect_surfaces(&metadata);

    Ok(InitPlan {
        crates,
        propagation,
        surfaces,
        coverage: None,
        ci_steps: InitPlan::default_steps(),
        offline_skip: None,
        flow: Some(FlowPlan::default()),
        git_hooks: true,
        github_ci: true,
        deny_toml: true,
        ctx_scaffold: true,
        mdbook: true,
    })
}

/// Build an InitPlan interactively using dialoguer prompts.
pub fn plan_interactive() -> Result<InitPlan, TaskitError> {
    use dialoguer::{Confirm, Input};

    let mut plan = plan_from_discovery()?;

    let add_coverage = Confirm::new()
        .with_prompt("Add coverage configuration?")
        .default(false)
        .interact()
        .map_err(TaskitError::other)?;

    if add_coverage {
        let crate_name: String = Input::new()
            .with_prompt("Coverage crate name")
            .interact_text()
            .map_err(TaskitError::other)?;
        let threshold: f64 = Input::new()
            .with_prompt("Coverage threshold (%)")
            .default(80.0)
            .interact_text()
            .map_err(TaskitError::other)?;
        plan.coverage = Some(CoveragePlan {
            crate_name,
            threshold,
        });
    }

    let offline_skip: String = Input::new()
        .with_prompt("Offline skip expression (empty for none)")
        .default(String::new())
        .interact_text()
        .map_err(TaskitError::other)?;
    if !offline_skip.is_empty() {
        plan.offline_skip = Some(offline_skip);
    }

    plan.flow = if Confirm::new()
        .with_prompt("Configure git flow branches?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?
    {
        let main: String = Input::new()
            .with_prompt("Main branch")
            .default("main".into())
            .interact_text()
            .map_err(TaskitError::other)?;
        let staging: String = Input::new()
            .with_prompt("Staging branch")
            .default("staging".into())
            .interact_text()
            .map_err(TaskitError::other)?;
        let release: String = Input::new()
            .with_prompt("Release branch")
            .default("release".into())
            .interact_text()
            .map_err(TaskitError::other)?;
        Some(FlowPlan {
            main,
            staging,
            release,
        })
    } else {
        None
    };

    plan.git_hooks = Confirm::new()
        .with_prompt("Generate git hooks (.githooks/)?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?;

    plan.github_ci = Confirm::new()
        .with_prompt("Generate GitHub Actions CI workflow?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?;

    plan.deny_toml = Confirm::new()
        .with_prompt("Generate deny.toml for cargo-deny?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?;

    plan.ctx_scaffold = Confirm::new()
        .with_prompt("Generate .ctx/ project context scaffold?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?;

    plan.mdbook = Confirm::new()
        .with_prompt("Generate docs/ mdBook scaffold?")
        .default(true)
        .interact()
        .map_err(TaskitError::other)?;

    Ok(plan)
}

struct DiscoveredMember {
    dir: String,
    pkg: String,
    /// Workspace-local dependency names (only packages in the workspace).
    deps: Vec<String>,
}

fn cargo_metadata_members() -> Result<Vec<DiscoveredMember>, TaskitError> {
    let metadata = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|e| TaskitError::other(format!("cargo metadata failed: {e}")))?;

    let ws_root = metadata.workspace_root.as_std_path();
    let ws_pkg_names: Vec<String> = metadata
        .workspace_members
        .iter()
        .filter_map(|id| {
            metadata
                .packages
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.name.clone())
        })
        .collect();

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
                let deps: Vec<String> = p
                    .dependencies
                    .iter()
                    .filter(|d| ws_pkg_names.contains(&d.name))
                    .map(|d| d.name.clone())
                    .collect();
                DiscoveredMember {
                    dir,
                    pkg: p.name.clone(),
                    deps,
                }
            })
        })
        .collect())
}

/// Infer propagation entries from workspace dependency graph.
///
/// If crate B depends on crate A (both in workspace), then A is a "source"
/// and B is a "dependent" — changes to A should propagate to B.
fn infer_propagation(members: &[DiscoveredMember]) -> Vec<PropagationEntry> {
    use std::collections::BTreeMap;
    let mut source_to_dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for member in members {
        for dep in &member.deps {
            source_to_dependents
                .entry(dep.clone())
                .or_default()
                .push(member.pkg.clone());
        }
    }

    source_to_dependents
        .into_iter()
        .filter(|(_, deps)| !deps.is_empty())
        .map(|(source, dependents)| PropagationEntry { source, dependents })
        .collect()
}

/// Detect candidate protocol surfaces by scanning for pub trait files.
fn detect_surfaces(members: &[DiscoveredMember]) -> Vec<SurfacePlan> {
    let mut surfaces = Vec::new();
    let ws_root = std::env::current_dir().unwrap_or_default();

    for member in members {
        let member_dir = if member.dir.is_empty() || member.dir == "." {
            ws_root.join("src")
        } else {
            ws_root.join(&member.dir).join("src")
        };

        if !member_dir.exists() {
            continue;
        }

        let Ok(entries) = std::fs::read_dir(&member_dir) else {
            continue;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "rs") {
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if content.contains("pub trait ") {
                    let rel = path
                        .strip_prefix(&ws_root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned();
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    surfaces.push(SurfacePlan {
                        name: format!("{}-{}", member.pkg, name),
                        path: rel,
                    });
                }
            }
        }
    }

    surfaces
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
        let plan = plan_from_discovery().unwrap();
        assert!(!plan.crates.is_empty());
    }

    #[test]
    fn plan_from_discovery_has_default_steps() {
        let plan = plan_from_discovery().unwrap();
        assert!(!plan.ci_steps.is_empty());
        assert_eq!(plan.ci_steps[0].name, "self-check");
    }

    #[test]
    fn plan_from_discovery_infers_propagation() {
        let plan = plan_from_discovery().unwrap();
        // taskit workspace has cross-crate deps, so propagation should be non-empty
        assert!(
            !plan.propagation.is_empty(),
            "should infer propagation from workspace deps"
        );
    }

    #[test]
    fn plan_from_discovery_detects_surfaces() {
        let plan = plan_from_discovery().unwrap();
        // taskit-core has PipelineRunner trait, should be detected
        assert!(
            !plan.surfaces.is_empty(),
            "should detect pub trait surfaces"
        );
    }

    #[test]
    fn plan_from_discovery_has_flow() {
        let plan = plan_from_discovery().unwrap();
        assert!(plan.flow.is_some());
        let flow = plan.flow.unwrap();
        assert_eq!(flow.main, "main");
        assert_eq!(flow.staging, "staging");
        assert_eq!(flow.release, "release");
    }

    #[test]
    fn flow_plan_default() {
        let f = FlowPlan::default();
        assert_eq!(f.main, "main");
        assert_eq!(f.staging, "staging");
        assert_eq!(f.release, "release");
    }

    #[test]
    fn infer_propagation_from_deps() {
        let members = vec![
            DiscoveredMember {
                dir: "crates/core".into(),
                pkg: "core".into(),
                deps: vec![],
            },
            DiscoveredMember {
                dir: "crates/engine".into(),
                pkg: "engine".into(),
                deps: vec!["core".into()],
            },
            DiscoveredMember {
                dir: "crates/cli".into(),
                pkg: "cli".into(),
                deps: vec!["core".into(), "engine".into()],
            },
        ];
        let prop = infer_propagation(&members);
        assert_eq!(prop.len(), 2);
        assert_eq!(prop[0].source, "core");
        assert!(prop[0].dependents.contains(&"engine".to_string()));
        assert!(prop[0].dependents.contains(&"cli".to_string()));
        assert_eq!(prop[1].source, "engine");
        assert!(prop[1].dependents.contains(&"cli".to_string()));
    }
}
