use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRef {
    pub owner: String,
    pub repo: String,
    pub git_ref: String,
}

impl GitHubRef {
    pub fn parse(url: &str) -> Result<Self> {
        let url = url.trim().trim_end_matches('/');

        let path = url
            .strip_prefix("https://github.com/")
            .or_else(|| url.strip_prefix("http://github.com/"))
            .ok_or_else(|| Error::GitHubParse(format!("not a GitHub URL: {url}")))?;

        let parts: Vec<&str> = path.split('/').collect();

        if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(Error::GitHubParse(format!(
                "missing owner/repo in URL: {url}"
            )));
        }

        let owner = parts[0].to_string();
        let repo = parts[1].to_string();

        let git_ref = if parts.len() >= 4 && parts[2] == "tree" {
            parts[3..].join("/")
        } else {
            "main".to_string()
        };

        Ok(Self {
            owner,
            repo,
            git_ref,
        })
    }

    pub fn archive_url(&self) -> String {
        // Use /{ref}.zip format which works for branches, tags, and commit SHAs
        // This is more universal than refs/heads/ or refs/tags/ specific paths
        format!(
            "https://github.com/{}/{}/archive/{}.zip",
            self.owner, self.repo, self.git_ref
        )
    }

    pub fn raw_url(&self, path: &str) -> String {
        let path = path.trim_start_matches('/');
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            self.owner, self.repo, self.git_ref, path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_url() {
        let gh = GitHubRef::parse("https://github.com/anthropics/claude-code").unwrap();
        assert_eq!(gh.owner, "anthropics");
        assert_eq!(gh.repo, "claude-code");
        assert_eq!(gh.git_ref, "main");
    }

    #[test]
    fn parse_url_with_branch() {
        let gh = GitHubRef::parse("https://github.com/owner/repo/tree/develop").unwrap();
        assert_eq!(gh.owner, "owner");
        assert_eq!(gh.repo, "repo");
        assert_eq!(gh.git_ref, "develop");
    }

    #[test]
    fn parse_url_with_nested_branch() {
        let gh = GitHubRef::parse("https://github.com/owner/repo/tree/feature/foo").unwrap();
        assert_eq!(gh.git_ref, "feature/foo");
    }

    #[test]
    fn parse_url_with_trailing_slash() {
        let gh = GitHubRef::parse("https://github.com/owner/repo/").unwrap();
        assert_eq!(gh.owner, "owner");
        assert_eq!(gh.repo, "repo");
    }

    #[test]
    fn parse_http_url() {
        let gh = GitHubRef::parse("http://github.com/owner/repo").unwrap();
        assert_eq!(gh.owner, "owner");
    }

    #[test]
    fn parse_invalid_url() {
        assert!(GitHubRef::parse("https://gitlab.com/owner/repo").is_err());
        assert!(GitHubRef::parse("https://github.com/").is_err());
        assert!(GitHubRef::parse("https://github.com/owner").is_err());
    }

    #[test]
    fn archive_url_format() {
        let gh = GitHubRef {
            owner: "anthropics".into(),
            repo: "claude-code".into(),
            git_ref: "main".into(),
        };
        assert_eq!(
            gh.archive_url(),
            "https://github.com/anthropics/claude-code/archive/main.zip"
        );
    }

    #[test]
    fn raw_url_format() {
        let gh = GitHubRef {
            owner: "anthropics".into(),
            repo: "claude-code".into(),
            git_ref: "main".into(),
        };
        assert_eq!(
            gh.raw_url("README.md"),
            "https://raw.githubusercontent.com/anthropics/claude-code/main/README.md"
        );
    }

    #[test]
    fn raw_url_strips_leading_slash() {
        let gh = GitHubRef {
            owner: "o".into(),
            repo: "r".into(),
            git_ref: "main".into(),
        };
        assert_eq!(
            gh.raw_url("/path/to/file.txt"),
            "https://raw.githubusercontent.com/o/r/main/path/to/file.txt"
        );
    }
}
