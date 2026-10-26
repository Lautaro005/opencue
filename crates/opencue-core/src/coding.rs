//! Coding groups: a team of agents (OpenCue agents on any model, and CLI agents such as Claude Code
//! or Codex) working on one repository, with its pull requests and changes beside the chat.
//!
//! How the team talks is borrowed from Agent Relay (github.com/AgentWorkforce/relay): short
//! protocol lines (`ACK`, `STATUS`, `BLOCKED`, `DONE`) so everyone knows the state without asking,
//! handoffs that say what "done" means, one writer per branch, and an *obligation* that follows a
//! handoff until the receiver answers `DONE` or `BLOCKED` (a reminder, once, when they stop without).

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::github::{self, Changes, Pull, RepoStatus};
use crate::models::*;
use crate::Core;

/// The protocol words a teammate starts a line with.
pub const PROTOCOL: [&str; 4] = ["ACK", "STATUS", "BLOCKED", "DONE"];

/// `ACK: …`, `STATUS: …`, `BLOCKED: …` and `DONE: …` lines of a message, as (word, rest).
pub fn protocol_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim().trim_start_matches(['*', '_', '`', '>', '-', ' ']);
            let (word, rest) = l.split_once(':')?;
            let word = word
                .trim()
                .trim_end_matches(['*', '_', '`'])
                .to_ascii_uppercase();
            PROTOCOL.contains(&word.as_str()).then(|| {
                (
                    word,
                    rest.trim()
                        .trim_start_matches(['*', '_', '`'])
                        .trim()
                        .to_string(),
                )
            })
        })
        .collect()
}

/// Whether a message closes an obligation: it says it is done, or what is blocking it.
pub fn closes_obligation(text: &str) -> bool {
    protocol_lines(text)
        .iter()
        .any(|(w, _)| w == "DONE" || w == "BLOCKED")
}

/// What a coding group's agents are told about the team, on top of everything else.
pub fn team_prompt(
    me: &str,
    lead: &str,
    is_lead: bool,
    repo: Option<&str>,
    worktree: Option<&str>,
) -> String {
    let mut s = String::from("\n\n## Coding team\n");
    match repo {
        Some(r) => s.push_str(&format!("You are on a team of coding agents working on the repository at `{r}`.")),
        None => s.push_str("You are on a team of coding agents. No repository is linked yet: ask the user to pick the folder in the group's panel."),
    }
    if let Some(w) = worktree {
        s.push_str(&format!(
            " Your own working copy is `{w}`: work and commit only there."
        ));
    }
    s.push_str(&format!(
        " The lead is @{lead}{}.\n\n\
         **Talk in short protocol lines**, one per line, at the start of your message, so the team always knows where things stand:\n\
         - `ACK: <what you understood and will do>` the moment you receive a task (one line, then start).\n\
         - `STATUS: <progress, files touched, what is next>` at real milestones, not every step.\n\
         - `BLOCKED: <what you need and from whom>` instead of guessing, or waiting in silence.\n\
         - `DONE: <result and evidence>` when finished: the command you ran and its result, the tests, the diff summary or the PR link. A task is not done until you say `DONE`.\n\n\
         **Handoffs.** When you give work to a teammate (`send_to_agent` or an @mention) write: the task in one sentence, the context they need (files, branch, decisions), \
         how to tell it is done (a command, a test, an observable result), and \"reply with DONE or BLOCKED\". Teammates can't see your chat. \
         When you receive a handoff, `ACK` first. Whoever hands work out owes the user the result: follow each handoff until it comes back `DONE` or `BLOCKED`; \
         the app reminds a teammate once if they stop without either.\n\n\
         **Working rules.** One writer per file and per branch at a time: work on your own branch (`agent/<your-name>/<topic>`), never push to the default branch, \
         and open a pull request when a piece is ready. Read before you edit; make the smallest change that works; run the tests before you say `DONE`. \
         Say what you did not verify. Don't repeat what a teammate already said: add something or stay quiet. Ask the lead (or the user, with `ask_user`) before changing the scope, \
         touching other people's branches, force-pushing, deleting, merging or releasing.",
        if is_lead { " (you)" } else { "" }
    ));
    if is_lead {
        s.push_str(&format!(
            "\n\n**As the lead, {me}:** turn the user's goal into small tasks with a checkable \"done when\", hand each to the teammate best suited, keep the list with `update_plan`, \
             review each `DONE` against its evidence (read the diff, run the check) and send work back if it doesn't hold. Don't do a teammate's task yourself while they are on it. \
             Report to the user only when something is finished, blocked, or needs a decision; say which PRs are ready and what is left."
        ));
    }
    s
}

/// The instructions a coding agent starts with when it is added from the group's panel.
pub fn coding_agent_instructions(cli_label: &str) -> String {
    format!(
        "You are {cli_label}, a coding agent on a team that works on one repository. Make real changes in your working folder with your tools, \
         run the project's checks before you report, and follow the team protocol (ACK / STATUS / BLOCKED / DONE) from your system prompt. \
         Work on a branch of your own and propose changes as a pull request; never push to the default branch."
    )
}

/// One working folder of a coding group and what changed in it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Lane {
    pub agent_ids: Vec<String>,
    pub dir: String,
    pub branch: Option<String>,
    pub files: Vec<github::FileChange>,
}

impl Core {
    /// The group and its repository folder.
    async fn coding_dir(&self, group_id: &str) -> anyhow::Result<(Group, PathBuf)> {
        let group = self
            .store
            .get_group(group_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such group"))?;
        anyhow::ensure!(group.kind == "coding", "this is not a coding group");
        let dir = group
            .config
            .get("repo_dir")
            .and_then(Value::as_str)
            .filter(|d| !d.trim().is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("Pick the repository folder first."))?;
        Ok((group, dir))
    }

    pub async fn github_status(&self, group_id: &str) -> anyhow::Result<RepoStatus> {
        let (_, dir) = self.coding_dir(group_id).await?;
        Ok(github::status(&dir).await)
    }

    pub async fn github_pulls(&self, group_id: &str, state: &str) -> anyhow::Result<Vec<Pull>> {
        let (_, dir) = self.coding_dir(group_id).await?;
        github::pulls(&dir, state).await
    }

    pub async fn github_changes(&self, group_id: &str) -> anyhow::Result<Changes> {
        let (_, dir) = self.coding_dir(group_id).await?;
        github::changes(&dir).await
    }

    /// Which files each teammate has created or changed so far, from their own working folders
    /// (a worktree each, or the shared repository): what the Activity list of the panel shows.
    pub async fn coding_activity(&self, group_id: &str) -> anyhow::Result<Vec<Lane>> {
        let (group, repo) = self.coding_dir(group_id).await?;
        let mut lanes: Vec<Lane> = vec![];
        for m in &group.members {
            let Ok(agent) = self.store.get_agent(&m.agent_id).await else {
                continue;
            };
            let dir = agent
                .settings
                .working_dir
                .as_deref()
                .filter(|d| !d.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| repo.clone());
            if let Some(l) = lanes
                .iter_mut()
                .find(|l| l.dir == dir.display().to_string())
            {
                l.agent_ids.push(agent.id.clone());
                continue;
            }
            let branch = github::status(&dir).await.branch;
            lanes.push(Lane {
                agent_ids: vec![agent.id.clone()],
                dir: dir.display().to_string(),
                branch,
                files: github::changed_files(&dir).await,
            });
        }
        Ok(lanes)
    }

    pub async fn github_create_pr(
        &self,
        group_id: &str,
        title: &str,
        body: &str,
        base: Option<&str>,
        draft: bool,
    ) -> anyhow::Result<String> {
        let (_, dir) = self.coding_dir(group_id).await?;
        github::create_pull(&dir, title, body, base, draft).await
    }

    pub async fn github_merge_pr(
        &self,
        group_id: &str,
        number: u64,
        method: &str,
        auto: bool,
    ) -> anyhow::Result<String> {
        let (_, dir) = self.coding_dir(group_id).await?;
        github::merge_pull(&dir, number, method, auto).await
    }

    pub async fn github_comment(
        &self,
        group_id: &str,
        number: u64,
        body: &str,
    ) -> anyhow::Result<()> {
        let (_, dir) = self.coding_dir(group_id).await?;
        github::comment_pull(&dir, number, body).await
    }

    /// Ask the group to fix a pull request whose checks fail: the failing checks and the end of the
    /// failing log go to the lead as a message from the user.
    pub async fn coding_fix_ci(
        self: &std::sync::Arc<Self>,
        group_id: &str,
        number: u64,
    ) -> anyhow::Result<()> {
        let (group, dir) = self.coding_dir(group_id).await?;
        let report = github::failure_report(&dir, number).await?;
        let lead = group
            .members
            .iter()
            .find(|m| m.is_owner)
            .or(group.members.first())
            .ok_or_else(|| anyhow::anyhow!("the group has no members"))?;
        let name = self.store.get_agent(&lead.agent_id).await?.name;
        let text = format!(
            "@{name} fix this pull request's failing checks. Hand it to the teammate who owns the branch if there is one; otherwise do it: \
             check out the branch, reproduce the failure, push a fix, and report `DONE:` with the passing run.\n\n{report}"
        );
        self.send_message(group_id, &text).await?;
        Ok(())
    }

    /// Link the group to a repository folder and, when asked, give each agent a worktree of its own.
    pub async fn coding_set_repo(
        self: &std::sync::Arc<Self>,
        group_id: &str,
        dir: &str,
        worktrees: bool,
    ) -> anyhow::Result<RepoStatus> {
        let group = self
            .store
            .get_group(group_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such group"))?;
        anyhow::ensure!(group.kind == "coding", "this is not a coding group");
        let path = PathBuf::from(dir.trim());
        anyhow::ensure!(
            path.is_absolute(),
            "the repository folder must be a full path"
        );
        // The folder is the project: made if it doesn't exist, and turned into a git repository
        // (with a first commit, so branches and worktrees work) if it isn't one yet.
        std::fs::create_dir_all(&path)?;
        anyhow::ensure!(path.is_dir(), "{} is not a folder", path.display());
        let mut status = github::status(&path).await;
        if !status.is_repo {
            github::init_repo(&path).await?;
            status = github::status(&path).await;
        }
        anyhow::ensure!(status.is_repo, "{} is not a git repository", path.display());
        let mut config = group.config.clone();
        if !config.is_object() {
            config = json!({});
        }
        config["repo_dir"] = json!(path.display().to_string());
        config["worktrees"] = json!(worktrees);
        self.set_group_config(group_id, config).await?;
        for m in &group.members {
            if let Err(e) = self.coding_prepare_member(group_id, &m.agent_id).await {
                tracing::warn!("could not prepare {}: {e:#}", m.agent_id);
            }
        }
        Ok(github::status(&path).await)
    }

    /// Point an agent at the group's repository: a worktree of its own (if the group uses them),
    /// otherwise the repository folder, and a computer so it can run commands.
    pub async fn coding_prepare_member(
        &self,
        group_id: &str,
        agent_id: &str,
    ) -> anyhow::Result<()> {
        let group = self
            .store
            .get_group(group_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such group"))?;
        let Some(repo) = group
            .config
            .get("repo_dir")
            .and_then(Value::as_str)
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
        else {
            return Ok(());
        };
        let agent = self.store.get_agent(agent_id).await?;
        let isolate = group
            .config
            .get("worktrees")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let folder = if isolate {
            // The lead works in the repository itself; teammates each get a worktree.
            let lead = group
                .members
                .iter()
                .find(|m| m.is_owner)
                .is_some_and(|m| m.agent_id == agent.id);
            if lead {
                repo.clone()
            } else {
                github::add_worktree(&repo, &agent.name).await?
            }
        } else {
            repo.clone()
        };
        let mut settings = agent.settings.clone();
        settings.working_dir = Some(folder.display().to_string());
        if settings.computer == crate::computer::ComputerBackend::None {
            settings.computer = crate::computer::ComputerBackend::Host;
        }
        self.update_agent(
            agent_id,
            AgentPatch {
                settings: Some(settings),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Add an agent to a group; in a coding group it also gets pointed at the repository.
    pub async fn add_group_member(&self, group_id: &str, agent_id: &str) -> anyhow::Result<()> {
        self.store.add_group_member(group_id, agent_id).await?;
        let is_coding = self
            .store
            .get_group(group_id)
            .await?
            .is_some_and(|g| g.kind == "coding");
        if is_coding {
            if let Err(e) = self.coding_prepare_member(group_id, agent_id).await {
                tracing::warn!("could not prepare {agent_id} for the repository: {e:#}");
            }
        }
        self.emit(crate::CoreEvent::ThreadsChanged);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_protocol_lines() {
        let t = "Sure.\nACK: reading the auth module\n**DONE:** fixed, `cargo test` passes\n- status: halfway\nnot: a word";
        let l = protocol_lines(t);
        assert_eq!(l.len(), 3);
        assert_eq!(l[0], ("ACK".into(), "reading the auth module".into()));
        assert_eq!(l[1].0, "DONE");
        assert_eq!(l[2].0, "STATUS");
        assert!(closes_obligation(t));
        assert!(!closes_obligation("ACK: on it\nSTATUS: 50%"));
        assert!(closes_obligation("blocked: need the API key"));
    }

    #[test]
    fn the_team_prompt_names_the_lead_and_the_rules() {
        let p = team_prompt(
            "Ada",
            "Ada",
            true,
            Some("/r"),
            Some("/r/.opencue/worktrees/bo"),
        );
        assert!(p.contains("/r") && p.contains("DONE") && p.contains("As the lead"));
        assert!(p.contains("(you)"));
        let w = team_prompt("Bo", "Ada", false, None, None);
        assert!(!w.contains("As the lead") && w.contains("@Ada") && w.contains("No repository"));
    }
}
