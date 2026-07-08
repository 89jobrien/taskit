use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Look up `name` in a cargo metadata `packages` array and return its version string,
/// or `"unknown"` if not found.
fn find_package_version<'a>(packages: &'a [serde_json::Value], name: &str) -> &'a str {
    packages
        .iter()
        .find(|p| p["name"].as_str() == Some(name))
        .and_then(|p| p["version"].as_str())
        .unwrap_or("unknown")
}

pub fn run(ctx: &Ctx) -> Result<(), TaskitError> {
    let sh = &ctx.sh;
    let ws = ctx.ws();
    if ctx.dry_run {
        taskit_output::taskit_dry!("cargo metadata --no-deps --format-version 1");
        taskit_output::taskit_dry!("rustc --version");
        return Ok(());
    }

    let meta_json = cmd!(sh, "cargo metadata --no-deps --format-version 1")
        .read()
        .map_err(TaskitError::other)?;
    let meta: serde_json::Value = serde_json::from_str(&meta_json).map_err(TaskitError::other)?;
    let packages = meta["packages"]
        .as_array()
        .ok_or_else(|| TaskitError::other("no packages in cargo metadata output"))?;

    taskit_output::taskit_progress!("Workspace Versions:");
    if ws.crates.is_empty() {
        // Zero-config: show all packages from cargo metadata
        for pkg in packages {
            if let (Some(name), Some(ver)) = (pkg["name"].as_str(), pkg["version"].as_str()) {
                taskit_output::taskit_progress!("{name}: {ver}");
            }
        }
    } else {
        for entry in &ws.crates {
            let pkg_name = entry.pkg_name();
            taskit_output::taskit_progress!(
                "{}: {}",
                entry.dir,
                find_package_version(packages, pkg_name)
            );
        }
    }
    taskit_output::taskit_progress!("");
    let rustc = cmd!(sh, "rustc --version")
        .read()
        .map_err(TaskitError::other)?;
    taskit_output::taskit_progress!("rustc: {rustc}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(name: &str, version: &str) -> serde_json::Value {
        serde_json::json!({ "name": name, "version": version })
    }

    #[test]
    fn find_package_version_returns_version_when_found() {
        let pkgs = vec![pkg("my-api", "1.2.3")];
        assert_eq!(find_package_version(&pkgs, "my-api"), "1.2.3");
    }

    #[test]
    fn find_package_version_returns_unknown_when_not_found() {
        let pkgs = vec![pkg("other-crate", "0.1.0")];
        assert_eq!(find_package_version(&pkgs, "my-api"), "unknown");
    }

    #[test]
    fn find_package_version_returns_unknown_for_empty_list() {
        assert_eq!(find_package_version(&[], "my-api"), "unknown");
    }

    #[test]
    fn find_package_version_returns_first_match_when_multiple() {
        let pkgs = vec![pkg("my-api", "1.0.0"), pkg("my-api", "2.0.0")];
        assert_eq!(find_package_version(&pkgs, "my-api"), "1.0.0");
    }

    #[test]
    fn find_package_version_returns_unknown_when_version_field_missing() {
        let pkgs = vec![serde_json::json!({ "name": "my-api" })];
        assert_eq!(find_package_version(&pkgs, "my-api"), "unknown");
    }
}
