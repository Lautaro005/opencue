//! GitHub for coding groups, the way Claude Code's desktop app does it: the repository's branch and
//! changes (from `git`), its pull requests with their checks and reviews, creating and merging them
//! (from the GitHub CLI, `gh`), and a worktree for each agent so a team can work in parallel without
//! stepping on each other. Nothing here talks to GitHub directly: it all goes through the user's own
//! `git` and `gh`, with their credentials.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::process::Command;

const TIMEOUT: Duration = Duration::from_secs(40);
/// The most of a diff the app keeps (the rest is cut, and the view says so).
const MAX_PATCH: usize = 400_000;

async fn run(program: &str, args: &[&str], dir: &Path, secs: Duration) -> anyhow::Result<String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let out = tokio::time::timeout(secs, cmd.output())
        .await
        .map_err(|_| anyhow::anyhow!("`{program} {}` took too long", args.join(" ")))?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!("`{program}` is not installed")
            } else {
                anyhow::anyhow!("could not run `{program}`: {e}")
            }
        })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let msg = err.trim();
        anyhow::bail!(
            "{}",
            if msg.is_empty() {
                format!("`{program} {}` failed", args.join(" "))
            } else {
                msg.lines().take(6).collect::<Vec<_>>().join("\n")
            }
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

async fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    run("git", args, dir, TIMEOUT).await
}

async fn gh(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    run("gh", args, dir, TIMEOUT).await
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RepoStatus {
    pub dir: String,
    pub is_repo: bool,
    pub git_installed: bool,
    pub gh_installed: bool,
    /// The GitHub account `gh` is signed in as, if it is.
    pub gh_user: Option<String>,
    /// `owner/name` on GitHub.
    pub slug: Option<String>,
    pub branch: Option<String>,
    pub default_branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub changed_files: usize,
    pub additions: u64,
    pub deletions: u64,
    pub worktrees: Vec<Worktree>,
    pub error: Option<String>,
}

/// `owner/name` from a git remote URL (https or ssh), when it points at GitHub.
pub fn slug_from_remote(url: &str) -> Option<String> {
    let u = url.trim().trim_end_matches(".git").trim_end_matches('/');
    let rest = u
        .strip_prefix("git@github.com:")
        .or_else(|| u.strip_prefix("ssh://git@github.com/"))
        .or_else(|| u.split_once("github.com/").map(|x| x.1))?;
    let mut parts = rest.split('/');
    let (owner, name) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !name.is_empty()).then(|| format!("{owner}/{name}"))
}

/// `git worktree list --porcelain` as paths and branches.
pub fn parse_worktrees(porcelain: &str) -> Vec<Worktree> {
    let mut out = vec![];
    let mut cur = Worktree::default();
    for line in porcelain.lines().chain(std::iter::once("")) {
        if let Some(p) = line.strip_prefix("worktree ") {
            cur = Worktree {
                path: p.to_string(),
                branch: String::new(),
            };
        } else if let Some(b) = line.strip_prefix("branch ") {
            cur.branch = b.trim_start_matches("refs/heads/").to_string();
        } else if line.is_empty() && !cur.path.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    out
}

/// Added and removed lines from `git diff --numstat`, and how many files changed.
pub fn parse_numstat(numstat: &str) -> (usize, u64, u64) {
    let (mut files, mut add, mut del) = (0, 0u64, 0u64);
    for l in numstat.lines() {
        let mut p = l.split('\t');
        let (a, d) = (p.next().unwrap_or("-"), p.next().unwrap_or("-"));
        files += 1;
        add += a.parse::<u64>().unwrap_or(0);
        del += d.parse::<u64>().unwrap_or(0);
    }
    (files, add, del)
}

/// `git init` with a first (empty) commit on `main`, so branches and worktrees can be made.
pub async fn init_repo(dir: &Path) -> anyhow::Result<()> {
    git(dir, &["init", "-b", "main"]).await?;
    git(
        dir,
        &[
            "-c",
            "user.name=OpenCue",
            "-c",
            "user.email=opencue@localhost",
            "commit",
            "--allow-empty",
            "-m",
            "Start the project",
        ],
    )
    .await?;
    Ok(())
}

pub async fn status(dir: &Path) -> RepoStatus {
    let mut s = RepoStatus {
        dir: dir.display().to_string(),
        git_installed: true,
        ..Default::default()
    };
    if !dir.is_dir() {
        s.error = Some("the repository folder does not exist".into());
        return s;
    }
    match git(dir, &["rev-parse", "--is-inside-work-tree"]).await {
        Ok(o) if o.trim() == "true" => s.is_repo = true,
        Err(e) if e.to_string().contains("not installed") => {
            s.git_installed = false;
            s.error = Some(e.to_string());
            return s;
        }
        _ => {
            s.error = Some("this folder is not a git repository".into());
            return s;
        }
    }
    s.branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .ok()
        .map(|b| b.trim().to_string());
    s.slug = git(dir, &["remote", "get-url", "origin"])
        .await
        .ok()
        .and_then(|u| slug_from_remote(&u));
    s.default_branch = git(
        dir,
        &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"],
    )
    .await
    .ok()
    .map(|b| b.trim().trim_start_matches("origin/").to_string());
    if let Ok(counts) = git(
        dir,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )
    .await
    {
        let mut p = counts.split_whitespace();
        s.behind = p.next().and_then(|n| n.parse().ok()).unwrap_or(0);
        s.ahead = p.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    }
    if let Ok(n) = git(dir, &["diff", "HEAD", "--numstat"]).await {
        let (f, a, d) = parse_numstat(&n);
        s.changed_files = f;
        s.additions = a;
        s.deletions = d;
    }
    if let Ok(porcelain) = git(dir, &["status", "--porcelain", "-uall"]).await {
        // Files git has not seen yet count as changed too.
        let untracked = porcelain.lines().filter(|l| l.starts_with("??")).count();
        s.changed_files += untracked;
    }
    s.worktrees = git(dir, &["worktree", "list", "--porcelain"])
        .await
        .map(|o| parse_worktrees(&o))
        .unwrap_or_default();
    match gh(dir, &["api", "user", "-q", ".login"]).await {
        Ok(u) => {
            s.gh_installed = true;
            s.gh_user = Some(u.trim().to_string()).filter(|u| !u.is_empty());
        }
        Err(e) => s.gh_installed = !e.to_string().contains("not installed"),
    }
    s
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Checks {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    /// Names of the checks that failed.
    pub failing: Vec<String>,
}

/// Summarise a pull request's `statusCheckRollup` (check runs and commit statuses).
pub fn summarize_checks(rollup: &Value) -> Checks {
    let mut c = Checks::default();
    for item in rollup.as_array().into_iter().flatten() {
        let name = item
            .get("name")
            .or_else(|| item.get("context"))
            .and_then(Value::as_str)
            .unwrap_or("check")
            .to_string();
        let status = item.get("status").and_then(Value::as_str).unwrap_or("");
        let conclusion = item
            .get("conclusion")
            .or_else(|| item.get("state"))
            .and_then(Value::as_str)
            .unwrap_or("");
        c.total += 1;
        let running = matches!(
            status,
            "IN_PROGRESS" | "QUEUED" | "PENDING" | "WAITING" | "REQUESTED"
        ) || matches!(conclusion, "PENDING" | "EXPECTED");
        let bad = matches!(
            conclusion,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        );
        if bad {
            c.failed += 1;
            c.failing.push(name);
        } else if running || (status.is_empty() && conclusion.is_empty()) {
            c.pending += 1;
        } else {
            c.passed += 1;
        }
    }
    c
}

#[derive(Debug, Clone, Serialize)]
pub struct Pull {
    pub number: u64,
    pub title: String,
    pub branch: String,
    pub base: String,
    pub author: String,
    pub draft: bool,
    /// APPROVED, CHANGES_REQUESTED, REVIEW_REQUIRED or empty.
    pub review: String,
    pub checks: Checks,
    pub url: String,
    pub updated_at: String,
    pub additions: u64,
    pub deletions: u64,
    pub files: u64,
    /// MERGEABLE, CONFLICTING or UNKNOWN.
    pub mergeable: String,
    pub auto_merge: bool,
}

const PR_FIELDS: &str = "number,title,headRefName,baseRefName,author,isDraft,reviewDecision,statusCheckRollup,mergeable,url,updatedAt,additions,deletions,changedFiles,autoMergeRequest";

pub fn parse_pulls(json: &str) -> anyhow::Result<Vec<Pull>> {
    let v: Value = serde_json::from_str(json)?;
    let s = |x: &Value, k: &str| {
        x.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    Ok(v.as_array()
        .into_iter()
        .flatten()
        .map(|p| Pull {
            number: p.get("number").and_then(Value::as_u64).unwrap_or(0),
            title: s(p, "title"),
            branch: s(p, "headRefName"),
            base: s(p, "baseRefName"),
            author: p
                .pointer("/author/login")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            draft: p.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
            review: s(p, "reviewDecision"),
            checks: summarize_checks(p.get("statusCheckRollup").unwrap_or(&Value::Null)),
            url: s(p, "url"),
            updated_at: s(p, "updatedAt"),
            additions: p.get("additions").and_then(Value::as_u64).unwrap_or(0),
            deletions: p.get("deletions").and_then(Value::as_u64).unwrap_or(0),
            files: p.get("changedFiles").and_then(Value::as_u64).unwrap_or(0),
            mergeable: s(p, "mergeable"),
            auto_merge: p.get("autoMergeRequest").is_some_and(|a| !a.is_null()),
        })
        .collect())
}

pub async fn pulls(dir: &Path, state: &str) -> anyhow::Result<Vec<Pull>> {
    anyhow::ensure!(
        ["open", "closed", "merged", "all"].contains(&state),
        "unknown state"
    );
    let out = gh(
        dir,
        &[
            "pr", "list", "--state", state, "--limit", "30", "--json", PR_FIELDS,
        ],
    )
    .await?;
    parse_pulls(&out)
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct FileChange {
    pub path: String,
    /// M, A, D, R or ? (not tracked yet).
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Changes {
    pub files: Vec<FileChange>,
    pub patch: String,
    pub truncated: bool,
}

/// What `git status --porcelain` says about one line: its status letter and path.
fn porcelain_entry(line: &str) -> Option<(String, String)> {
    if line.len() < 4 {
        return None;
    }
    let (x, y) = (line.as_bytes()[0] as char, line.as_bytes()[1] as char);
    let path = line[3..]
        .rsplit(" -> ")
        .next()
        .unwrap_or(&line[3..])
        .trim_matches('"');
    let letter = if x == '?' {
        '?'
    } else if x == 'A' || y == 'A' {
        'A'
    } else if x == 'D' || y == 'D' {
        'D'
    } else if x == 'R' {
        'R'
    } else {
        'M'
    };
    Some((letter.to_string(), path.to_string()))
}

/// Files git has not seen yet: each one listed (not just its folder), with its lines counted and
/// a patch written for it, because `git diff` never shows them.
fn untracked_patch(dir: &Path, path: &str, with_patch: bool) -> (u64, String) {
    const MAX_FILE: u64 = 200_000;
    let full = dir.join(path);
    let Ok(meta) = std::fs::metadata(&full) else {
        return (0, String::new());
    };
    if !meta.is_file() || meta.len() > MAX_FILE {
        return (0, String::new());
    }
    let Ok(bytes) = std::fs::read(&full) else {
        return (0, String::new());
    };
    if bytes.contains(&0) {
        return (0, String::new());
    }
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    if !with_patch {
        return (lines.len() as u64, String::new());
    }
    let mut p = format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{} @@\n",
        lines.len()
    );
    for l in &lines {
        p.push('+');
        p.push_str(l);
        p.push('\n');
    }
    (lines.len() as u64, p)
}

/// The working tree against HEAD: tracked changes from `git diff`, and every new file.
async fn collect(dir: &Path, with_patch: bool) -> anyhow::Result<Changes> {
    let numstat = git(dir, &["diff", "HEAD", "--numstat"])
        .await
        .unwrap_or_default();
    // `-uall` lists the files inside a new folder instead of only the folder.
    let porcelain = git(dir, &["status", "--porcelain", "-uall"]).await?;
    let mut counts = std::collections::HashMap::new();
    for l in numstat.lines() {
        let mut p = l.split('\t');
        if let (Some(a), Some(d), Some(path)) = (p.next(), p.next(), p.next()) {
            counts.insert(
                path.to_string(),
                (a.parse::<u64>().unwrap_or(0), d.parse::<u64>().unwrap_or(0)),
            );
        }
    }
    let mut files = vec![];
    let mut extra = String::new();
    for (status, path) in porcelain.lines().filter_map(porcelain_entry) {
        let (mut additions, deletions) = counts.get(&path).copied().unwrap_or((0, 0));
        if status == "?" {
            let (n, patch) = untracked_patch(dir, &path, with_patch && extra.len() < MAX_PATCH);
            additions = n;
            extra.push_str(&patch);
        }
        files.push(FileChange {
            path,
            status,
            additions,
            deletions,
        });
    }
    let mut patch = if with_patch {
        git(dir, &["diff", "HEAD", "--no-color", "--no-ext-diff"])
            .await
            .unwrap_or_default()
    } else {
        String::new()
    };
    patch.push_str(&extra);
    Ok(Changes {
        files,
        truncated: cut(&mut patch),
        patch,
    })
}

/// Keeps a patch within `MAX_PATCH`; true when it had to be cut.
fn cut(patch: &mut String) -> bool {
    if patch.len() <= MAX_PATCH {
        return false;
    }
    let mut end = MAX_PATCH;
    while !patch.is_char_boundary(end) {
        end -= 1;
    }
    patch.truncate(end);
    true
}

/// The working tree's changes against HEAD: the files with their line counts, and the patch.
pub async fn changes(dir: &Path) -> anyhow::Result<Changes> {
    collect(dir, true).await
}

/// The changed files of a working tree with their line counts, without the patch.
pub async fn changed_files(dir: &Path) -> Vec<FileChange> {
    collect(dir, false)
        .await
        .map(|c| c.files)
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub author: String,
    pub when: String,
    pub files: Vec<FileChange>,
    /// Whether it is already on the upstream branch (on GitHub).
    pub pushed: bool,
}

/// The latest commits of the branch with the files each one touched.
pub async fn commits(dir: &Path, limit: usize) -> anyhow::Result<Vec<Commit>> {
    let n = format!("-n{limit}");
    let out = git(
        dir,
        &[
            "log",
            &n,
            "--numstat",
            "--no-color",
            "--format=\u{1}%H\u{2}%s\u{2}%an\u{2}%cr",
        ],
    )
    .await?;
    // What is not on the upstream yet (empty without one, so then nothing counts as pushed).
    let unpushed: std::collections::HashSet<String> = git(dir, &["rev-list", "@{upstream}..HEAD"])
        .await
        .map(|o| o.lines().map(str::to_string).collect())
        .unwrap_or_default();
    let has_upstream = git(dir, &["rev-parse", "@{upstream}"]).await.is_ok();
    let mut list: Vec<Commit> = vec![];
    for block in out.split('\u{1}').filter(|b| !b.trim().is_empty()) {
        let mut lines = block.lines();
        let mut head = lines.next().unwrap_or_default().split('\u{2}');
        let (sha, subject, author, when) = (
            head.next().unwrap_or_default().to_string(),
            head.next().unwrap_or_default().to_string(),
            head.next().unwrap_or_default().to_string(),
            head.next().unwrap_or_default().to_string(),
        );
        let files = lines
            .filter_map(|l| {
                let mut p = l.split('\t');
                let (a, d, path) = (p.next()?, p.next()?, p.next()?);
                Some(FileChange {
                    path: path.to_string(),
                    status: "M".into(),
                    additions: a.parse().unwrap_or(0),
                    deletions: d.parse().unwrap_or(0),
                })
            })
            .collect();
        list.push(Commit {
            pushed: has_upstream && !unpushed.contains(&sha),
            sha,
            subject,
            author,
            when,
            files,
        });
    }
    Ok(list)
}

/// The patch of one commit.
pub async fn commit_patch(dir: &Path, sha: &str) -> anyhow::Result<Changes> {
    anyhow::ensure!(
        !sha.is_empty() && sha.chars().all(|c| c.is_ascii_hexdigit()),
        "not a commit"
    );
    let mut patch = git(
        dir,
        &["show", sha, "--format=", "--no-color", "--no-ext-diff"],
    )
    .await?;
    Ok(Changes {
        files: vec![],
        truncated: cut(&mut patch),
        patch,
    })
}

/// What a pull request changes on GitHub: its files and its patch.
pub async fn pull_changes(dir: &Path, number: u64) -> anyhow::Result<Changes> {
    let n = number.to_string();
    let files = gh(dir, &["pr", "view", &n, "--json", "files", "-q", ".files"]).await?;
    let list: Vec<Value> = serde_json::from_str(files.trim()).unwrap_or_default();
    let files = list
        .iter()
        .map(|f| FileChange {
            path: f["path"].as_str().unwrap_or_default().to_string(),
            status: match f["changeType"].as_str() {
                Some("ADDED") => "A",
                Some("DELETED") => "D",
                Some("RENAMED") => "R",
                _ => "M",
            }
            .into(),
            additions: f["additions"].as_u64().unwrap_or(0),
            deletions: f["deletions"].as_u64().unwrap_or(0),
        })
        .collect();
    let mut patch = gh(dir, &["pr", "diff", &n, "--color", "never"]).await?;
    Ok(Changes {
        files,
        truncated: cut(&mut patch),
        patch,
    })
}

/// A new pull request from the current branch (it is pushed first). Returns its address.
pub async fn create_pull(
    dir: &Path,
    title: &str,
    body: &str,
    base: Option<&str>,
    draft: bool,
) -> anyhow::Result<String> {
    anyhow::ensure!(!title.trim().is_empty(), "the pull request needs a title");
    let branch = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"]).await?;
    let branch = branch.trim();
    anyhow::ensure!(
        branch != "HEAD",
        "you are not on a branch: check one out first"
    );
    git(dir, &["push", "-u", "origin", branch]).await?;
    let mut args = vec!["pr", "create", "--title", title.trim(), "--body", body];
    if let Some(b) = base.filter(|b| !b.trim().is_empty()) {
        args.extend(["--base", b]);
    }
    if draft {
        args.push("--draft");
    }
    let out = run("gh", &args, dir, Duration::from_secs(90)).await?;
    Ok(out.lines().last().unwrap_or_default().trim().to_string())
}

/// Merge a pull request now, or turn on auto-merge so GitHub does it when the checks pass.
pub async fn merge_pull(
    dir: &Path,
    number: u64,
    method: &str,
    auto: bool,
) -> anyhow::Result<String> {
    let flag = match method {
        "merge" => "--merge",
        "rebase" => "--rebase",
        _ => "--squash",
    };
    let n = number.to_string();
    let mut args = vec!["pr", "merge", n.as_str(), flag];
    if auto {
        args.push("--auto");
    }
    let out = gh(dir, &args).await?;
    Ok(if out.trim().is_empty() {
        if auto {
            format!("Auto-merge is on for #{number}.")
        } else {
            format!("Merged #{number}.")
        }
    } else {
        out.trim().to_string()
    })
}

pub async fn comment_pull(dir: &Path, number: u64, body: &str) -> anyhow::Result<()> {
    anyhow::ensure!(!body.trim().is_empty(), "the comment is empty");
    let n = number.to_string();
    gh(dir, &["pr", "comment", &n, "--body", body]).await?;
    Ok(())
}

/// The failing checks of a pull request with a tail of the failing log, for an agent to fix.
pub async fn failure_report(dir: &Path, number: u64) -> anyhow::Result<String> {
    let n = number.to_string();
    let json = gh(
        dir,
        &[
            "pr",
            "view",
            &n,
            "--json",
            &format!("{PR_FIELDS},headRefName"),
        ],
    )
    .await?;
    let pull = parse_pulls(&format!("[{json}]"))?.into_iter().next();
    let Some(p) = pull else {
        anyhow::bail!("no pull request #{number}");
    };
    let mut out = format!(
        "PR #{} “{}” (branch {}, {}): {} of {} checks failed.\n",
        p.number, p.title, p.branch, p.url, p.checks.failed, p.checks.total
    );
    for name in &p.checks.failing {
        out.push_str(&format!("- failing: {name}\n"));
    }
    // The failing log of the latest run on the branch, if gh can get it.
    if let Ok(runs) = gh(
        dir,
        &[
            "run",
            "list",
            "--branch",
            &p.branch,
            "--limit",
            "5",
            "--json",
            "databaseId,conclusion",
        ],
    )
    .await
    {
        let id = serde_json::from_str::<Value>(&runs).ok().and_then(|v| {
            v.as_array()?
                .iter()
                .find(|r| r["conclusion"] == "failure")
                .and_then(|r| r["databaseId"].as_u64())
        });
        if let Some(id) = id {
            if let Ok(log) = run(
                "gh",
                &["run", "view", &id.to_string(), "--log-failed"],
                dir,
                Duration::from_secs(60),
            )
            .await
            {
                let tail: Vec<&str> = log.lines().rev().take(80).collect();
                out.push_str("\nEnd of the failing log:\n");
                out.push_str(&tail.into_iter().rev().collect::<Vec<_>>().join("\n"));
            }
        }
    }
    Ok(out)
}

fn slug_name(s: &str) -> String {
    let mut out = String::new();
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "agent".into()
    } else {
        out.chars().take(40).collect()
    }
}

/// A folder of its own inside the repository for one agent, on a branch of its own
/// (`agent/<name>/work`), so agents can work at the same time. Returns the folder.
pub async fn add_worktree(repo: &Path, agent_name: &str) -> anyhow::Result<PathBuf> {
    let slug = slug_name(agent_name);
    let path = repo.join(".opencue").join("worktrees").join(&slug);
    if path.join(".git").exists() {
        return Ok(path);
    }
    let branch = format!("agent/{slug}/work");
    // Keep the worktrees out of `git status`.
    if let Ok(common) = git(repo, &["rev-parse", "--git-common-dir"]).await {
        let info = repo.join(common.trim()).join("info");
        let _ = std::fs::create_dir_all(&info);
        let exclude = info.join("exclude");
        let cur = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !cur.lines().any(|l| l.trim() == "/.opencue/") {
            let _ = std::fs::write(
                &exclude,
                format!(
                    "{cur}{}/.opencue/\n",
                    if cur.is_empty() || cur.ends_with('\n') {
                        ""
                    } else {
                        "\n"
                    }
                ),
            );
        }
    }
    let p = path.display().to_string();
    // A new branch, or the one a previous run left behind.
    if git(repo, &["worktree", "add", "-b", &branch, &p])
        .await
        .is_err()
    {
        git(repo, &["worktree", "add", &p, &branch]).await?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn new_files_inside_new_folders_are_listed_with_a_patch_and_commits_show() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        init_repo(root).await.unwrap();
        std::fs::create_dir_all(root.join("src/ui")).unwrap();
        std::fs::write(root.join("src/ui/app.js"), "one\ntwo\nthree\n").unwrap();
        let c = changes(root).await.unwrap();
        assert_eq!(c.files.len(), 1, "{:?}", c.files);
        assert_eq!(c.files[0].path, "src/ui/app.js");
        assert_eq!((c.files[0].status.as_str(), c.files[0].additions), ("?", 3));
        assert!(c.patch.contains("+++ b/src/ui/app.js") && c.patch.contains("+two"));
        git(root, &["add", "-A"]).await.unwrap();
        git(
            root,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "add app",
            ],
        )
        .await
        .unwrap();
        let log = commits(root, 5).await.unwrap();
        assert_eq!(log[0].subject, "add app");
        assert_eq!(log[0].files[0].path, "src/ui/app.js");
        let p = commit_patch(root, &log[0].sha).await.unwrap();
        assert!(p.patch.contains("+three"));
    }

    #[test]
    fn reads_github_remotes() {
        assert_eq!(
            slug_from_remote("git@github.com:Lautaro005/opencue.git"),
            Some("Lautaro005/opencue".into())
        );
        assert_eq!(
            slug_from_remote("https://github.com/a/b.git\n"),
            Some("a/b".into())
        );
        assert_eq!(
            slug_from_remote("https://github.com/a/b/"),
            Some("a/b".into())
        );
        assert_eq!(slug_from_remote("https://gitlab.com/a/b"), None);
    }

    #[test]
    fn reads_worktrees_and_numstat() {
        let w = parse_worktrees("worktree /r\nHEAD abc\nbranch refs/heads/main\n\nworktree /r/.opencue/worktrees/x\nHEAD def\nbranch refs/heads/agent/x/work\n");
        assert_eq!(w.len(), 2);
        assert_eq!(w[1].branch, "agent/x/work");
        assert_eq!(parse_numstat("3\t1\ta.rs\n-\t-\timg.png\n"), (2, 3, 1));
    }

    #[test]
    fn summarizes_checks() {
        let rollup = serde_json::json!([
            {"__typename": "CheckRun", "name": "build", "status": "COMPLETED", "conclusion": "SUCCESS"},
            {"__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "FAILURE"},
            {"__typename": "CheckRun", "name": "lint", "status": "IN_PROGRESS", "conclusion": ""},
            {"__typename": "StatusContext", "context": "ci/legacy", "state": "SUCCESS"},
            {"__typename": "StatusContext", "context": "ci/other", "state": "PENDING"}
        ]);
        let c = summarize_checks(&rollup);
        assert_eq!((c.total, c.passed, c.failed, c.pending), (5, 2, 1, 2));
        assert_eq!(c.failing, vec!["test"]);
    }

    #[test]
    fn reads_pull_requests() {
        let json = r#"[{"number":7,"title":"Add x","headRefName":"feat/x","baseRefName":"main","author":{"login":"me"},"isDraft":false,"reviewDecision":"APPROVED","statusCheckRollup":[],"mergeable":"MERGEABLE","url":"https://github.com/a/b/pull/7","updatedAt":"2026-10-01T00:00:00Z","additions":10,"deletions":2,"changedFiles":3,"autoMergeRequest":null}]"#;
        let p = parse_pulls(json).unwrap();
        assert_eq!(p[0].number, 7);
        assert_eq!(p[0].author, "me");
        assert_eq!(p[0].review, "APPROVED");
        assert!(!p[0].auto_merge);
    }

    #[test]
    fn reads_git_status_lines() {
        assert_eq!(
            porcelain_entry(" M src/a.rs"),
            Some(("M".into(), "src/a.rs".into()))
        );
        assert_eq!(
            porcelain_entry("?? new.txt"),
            Some(("?".into(), "new.txt".into()))
        );
        assert_eq!(
            porcelain_entry("R  old.rs -> new.rs"),
            Some(("R".into(), "new.rs".into()))
        );
        assert_eq!(
            porcelain_entry("D  gone.rs"),
            Some(("D".into(), "gone.rs".into()))
        );
    }

    #[test]
    fn names_are_branch_safe() {
        assert_eq!(slug_name("Claude Code #2"), "claude-code-2");
        assert_eq!(slug_name("***"), "agent");
    }

    #[tokio::test]
    async fn status_and_worktree_in_a_real_repo() {
        if run("git", &["--version"], Path::new("."), TIMEOUT)
            .await
            .is_err()
        {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@t"],
            vec!["config", "user.name", "t"],
        ] {
            git(p, &args).await.unwrap();
        }
        std::fs::write(p.join("a.txt"), "one\n").unwrap();
        git(p, &["add", "."]).await.unwrap();
        git(p, &["commit", "-q", "-m", "init"]).await.unwrap();
        std::fs::write(p.join("a.txt"), "one\ntwo\n").unwrap();
        std::fs::write(p.join("b.txt"), "new\n").unwrap();
        let s = status(p).await;
        assert!(s.is_repo);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.changed_files, 2);
        assert_eq!(s.additions, 1);
        let c = changes(p).await.unwrap();
        assert!(c
            .files
            .iter()
            .any(|f| f.path == "a.txt" && f.status == "M" && f.additions == 1));
        assert!(c.files.iter().any(|f| f.path == "b.txt" && f.status == "?"));
        assert!(c.patch.contains("+two"));
        let wt = add_worktree(p, "Claude Code").await.unwrap();
        assert!(wt.join(".git").exists());
        assert!(add_worktree(p, "Claude Code").await.is_ok());
        let s2 = status(p).await;
        assert!(s2
            .worktrees
            .iter()
            .any(|w| w.branch == "agent/claude-code/work"));
        // The worktree folder is hidden from git status.
        assert_eq!(s2.changed_files, 2);
    }
}
