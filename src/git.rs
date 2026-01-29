use crate::error::{Error, Result};
use std::path::PathBuf;
use std::process::Command;

pub fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_worktree() -> Result<bool> {
    let git_dir = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()?;
    let git_common_dir = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()?;

    let git_dir = String::from_utf8_lossy(&git_dir.stdout).trim().to_string();
    let git_common_dir = String::from_utf8_lossy(&git_common_dir.stdout)
        .trim()
        .to_string();

    Ok(git_dir != git_common_dir)
}

pub fn remote_exists(remote: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["remote", "get-url", remote])
        .output()?;
    Ok(output.status.success())
}

pub fn get_remote_url(remote: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", remote])
        .output()?;

    if !output.status.success() {
        return Err(Error::RemoteNotFound(remote.to_string()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn branch_exists(branch: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["show-ref", "--verify", "--quiet", &format!("refs/heads/{}", branch)])
        .output()?;
    Ok(output.status.success())
}

pub fn fetch(remote: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["fetch", remote])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!("fetch failed: {}", stderr)));
    }

    Ok(())
}

pub fn add_worktree(path: &PathBuf, branch: &str, start_point: &str) -> Result<()> {
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            path.to_str().unwrap(),
            "-b",
            branch,
            start_point,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!("worktree add failed: {}", stderr)));
    }

    Ok(())
}

pub fn remove_worktree(path: &PathBuf) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force", path.to_str().unwrap()])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!("worktree remove failed: {}", stderr)));
    }

    Ok(())
}

pub fn delete_branch(branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["branch", "-D", branch])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Git(format!("branch delete failed: {}", stderr)));
    }

    Ok(())
}

pub fn status_short(path: &PathBuf) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", path.to_str().unwrap(), "status", "-s"])
        .output()?;

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn get_project_prefix(remote: &str) -> Result<String> {
    let url = get_remote_url(remote)?;
    let name = parse_repo_name(&url)?;
    Ok(to_acronym(&name))
}

fn parse_repo_name(url: &str) -> Result<String> {
    // Handle SSH format: git@github.com:user/project.git
    // Handle HTTPS format: https://github.com/user/project.git
    let name = url
        .rsplit('/')
        .next()
        .or_else(|| url.rsplit(':').next())
        .ok_or_else(|| Error::Git(format!("Cannot parse remote URL: {}", url)))?;

    // Strip .git suffix
    let name = name.strip_suffix(".git").unwrap_or(name);
    Ok(name.to_string())
}

fn to_acronym(name: &str) -> String {
    // Split on '-' or '_'
    let parts: Vec<&str> = name.split(|c| c == '-' || c == '_').collect();

    if parts.len() == 1 {
        // No separators, return name as-is
        name.to_lowercase()
    } else {
        // Take first char of each part
        parts
            .iter()
            .filter_map(|p| p.chars().next())
            .collect::<String>()
            .to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_acronym_no_separator() {
        assert_eq!(to_acronym("myproject"), "myproject");
    }

    #[test]
    fn test_to_acronym_hyphen() {
        assert_eq!(to_acronym("my-project"), "mp");
    }

    #[test]
    fn test_to_acronym_underscore() {
        assert_eq!(to_acronym("my_project"), "mp");
    }

    #[test]
    fn test_to_acronym_multiple_parts() {
        assert_eq!(to_acronym("my-cool-app"), "mca");
    }

    #[test]
    fn test_to_acronym_mixed_separators() {
        assert_eq!(to_acronym("foo-bar_baz"), "fbb");
    }

    #[test]
    fn test_parse_repo_name_ssh() {
        assert_eq!(
            parse_repo_name("git@github.com:user/my-project.git").unwrap(),
            "my-project"
        );
    }

    #[test]
    fn test_parse_repo_name_https() {
        assert_eq!(
            parse_repo_name("https://github.com/user/my-project.git").unwrap(),
            "my-project"
        );
    }

    #[test]
    fn test_parse_repo_name_no_git_suffix() {
        assert_eq!(
            parse_repo_name("https://github.com/user/myproject").unwrap(),
            "myproject"
        );
    }
}
