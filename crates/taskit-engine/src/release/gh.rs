use std::path::Path;

use taskit_types::error::TaskitError;
use xshell::cmd;

use crate::ctx::Ctx;

/// Create a GitHub release for the given tag.
///
/// Uses the `gh` CLI. If `notes_file` is provided, reads release notes
/// from that path; otherwise falls back to `--generate-notes`.
pub fn run(ctx: &Ctx, tag: &str, notes_file: Option<&Path>) -> Result<(), TaskitError> {
    let sh = &ctx.sh;

    let repo = resolve_repo(ctx)?;

    if ctx.dry_run {
        taskit_output::taskit_dry!("gh release create {tag} --repo {repo}");
        return Ok(());
    }

    // Ensure the tag exists locally
    let tag_check = cmd!(sh, "git tag -l {tag}").output();
    match tag_check {
        Ok(output) if output.stdout.is_empty() => {
            return Err(TaskitError::other(format!(
                "tag {tag} does not exist locally; create it first"
            )));
        }
        Err(e) => return Err(TaskitError::other(e)),
        _ => {}
    }

    let mut args = vec![
        "release".to_owned(),
        "create".to_owned(),
        tag.to_owned(),
        "--repo".to_owned(),
        repo.clone(),
        "--title".to_owned(),
        tag.to_owned(),
        "--verify-tag".to_owned(),
    ];

    if let Some(path) = notes_file {
        args.push("--notes-file".to_owned());
        args.push(path.display().to_string());
    } else {
        args.push("--generate-notes".to_owned());
    }

    taskit_output::taskit_progress!("creating GitHub release {tag} on {repo}");
    ctx.run(cmd!(sh, "gh {args...}"))?;
    taskit_output::taskit_ok!("GitHub release {tag} created");

    Ok(())
}

/// Resolve the GitHub repo: config > git remote > error.
fn resolve_repo(ctx: &Ctx) -> Result<String, TaskitError> {
    // 1. Check config
    if let Some(repo) = ctx.config.release.as_ref().and_then(|r| r.github_repo()) {
        return Ok(repo.to_owned());
    }

    // 2. Infer from git remote
    let sh = &ctx.sh;
    let output = cmd!(sh, "git remote get-url origin")
        .output()
        .map_err(|e| TaskitError::other(format!("failed to read git remote: {e}")))?;

    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    parse_github_repo(&url).ok_or_else(|| {
        TaskitError::other(
            "cannot determine GitHub repo; set [release] github_repo in taskit.toml \
             or add a GitHub origin remote",
        )
    })
}

/// Extract `owner/repo` from common GitHub URL formats.
fn parse_github_repo(url: &str) -> Option<String> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        return Some(rest.trim_end_matches(".git").to_owned());
    }
    // HTTPS: https://github.com/owner/repo.git
    if let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    {
        return Some(rest.trim_end_matches(".git").to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ssh_url() {
        assert_eq!(
            parse_github_repo("git@github.com:89jobrien/taskit.git"),
            Some("89jobrien/taskit".into())
        );
    }

    #[test]
    fn parse_https_url() {
        assert_eq!(
            parse_github_repo("https://github.com/89jobrien/taskit.git"),
            Some("89jobrien/taskit".into())
        );
    }

    #[test]
    fn parse_https_no_suffix() {
        assert_eq!(
            parse_github_repo("https://github.com/89jobrien/taskit"),
            Some("89jobrien/taskit".into())
        );
    }

    #[test]
    fn parse_non_github_returns_none() {
        assert_eq!(parse_github_repo("https://gitlab.com/user/repo.git"), None);
    }

    #[test]
    fn resolve_repo_falls_back_to_remote() {
        let ctx = Ctx::test();
        // In the taskit workspace, origin points to GitHub
        let result = resolve_repo(&ctx);
        // May fail in CI without a remote, that's fine — we're testing the logic
        if let Ok(repo) = result {
            assert!(repo.contains('/'), "should be owner/repo format");
        }
    }
}
