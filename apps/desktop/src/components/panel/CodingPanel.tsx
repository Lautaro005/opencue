import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUpRight, Check, ChevronDown, CircleAlert, CircleDot, FileText, FolderGit2, GitBranch, GitPullRequest, GitPullRequestDraft, LoaderCircle, Plus, RefreshCw, TerminalSquare, Trash2, UserPlus, Wrench, X } from "lucide-react";
import { api, openUrl, pickFolder } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import { cx, shortTime } from "../../lib/format";
import { useStore } from "../../lib/store";
import { addCodingAgent, CODING_CLIS, sessionsOf, type Session, type SessionState } from "../../lib/coding";
import type { Changes, Group, Lane, Pull, RepoStatus } from "../../lib/types";
import { AvatarView } from "../AvatarView";
import { Badge, Button, DialogHeader, Field, IconButton, Input, Modal, Spinner, Textarea, Toggle, useClickOutside } from "../ui";
import { useResizable } from "../ui/Resizer";
import { SharedFiles } from "./SharedFiles";

type Tab = "sessions" | "changes" | "pulls" | "repo";
const NO_MESSAGES: never[] = [];
const NO_WORKERS: string[] = [];

/**
 * The side panel of a coding group, like the Code tab of Claude Code's desktop app: who is working,
 * waiting or blocked, the changes in the repository, its pull requests with their checks (and
 * fixing, merging), and the repository itself. Agents can be added here, CLI agents included.
 */
export function CodingPanel({ group }: { group: Group }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const [tab, setTab] = useState<Tab>("sessions");
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const hasRepo = !!group.config.repo_dir;
  const { width, handle } = useResizable("opencue:panel-coding", 420, 340, 820, "left");

  const refreshStatus = useCallback(() => {
    if (!hasRepo) return setStatus(null);
    void api.githubStatus(group.id).then(setStatus).catch(() => setStatus(null));
  }, [group.id, hasRepo]);
  useEffect(() => {
    refreshStatus();
    const timer = window.setInterval(refreshStatus, 8000);
    return () => window.clearInterval(timer);
  }, [refreshStatus]);

  const tabs: { id: Tab; label: string; icon: React.ReactNode; badge?: string }[] = [
    { id: "sessions", label: t("coding.sessions"), icon: <TerminalSquare size={14} /> },
    { id: "changes", label: t("coding.changes"), icon: <GitBranch size={14} />, badge: status && status.changed_files > 0 ? String(status.changed_files) : undefined },
    { id: "pulls", label: t("coding.pulls"), icon: <GitPullRequest size={14} /> },
    { id: "repo", label: t("coding.repo"), icon: <FolderGit2 size={14} /> },
  ];

  return (
    <aside style={{ width }} className="animate-slide relative flex h-full shrink-0 flex-col border-l border-line bg-panel">
      {handle}
      <div className="flex h-13 shrink-0 items-center gap-2 px-4">
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-[14px] leading-tight font-semibold">{group.name}</h2>
          <p className="flex items-center gap-1.5 truncate text-[11.5px] leading-tight text-muted">
            {status?.slug ?? (hasRepo ? group.config.repo_dir : t("coding.noRepo"))}
            {status?.branch && (
              <span className="inline-flex items-center gap-0.5 font-mono">
                <GitBranch size={11} /> {status.branch}
              </span>
            )}
          </p>
        </div>
        <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
          <X size={17} />
        </IconButton>
      </div>
      <div className="flex shrink-0 gap-0.5 border-b border-line px-3" role="tablist">
        {tabs.map((x) => (
          <button
            key={x.id}
            role="tab"
            aria-selected={tab === x.id}
            onClick={() => setTab(x.id)}
            className={cx("-mb-px inline-flex items-center gap-1.5 whitespace-nowrap border-b-2 px-2.5 py-2 text-[12.5px] font-medium", tab === x.id ? "border-accent text-ink" : "border-transparent text-muted hover:text-ink")}
          >
            {x.icon}
            {x.label}
            {x.badge && <span className="rounded-full bg-accent-soft px-1.5 text-[10.5px] text-accent">{x.badge}</span>}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto">
        {tab === "sessions" && <Sessions group={group} />}
        {tab === "changes" && (hasRepo ? <ChangesTab group={group} status={status} onChanged={refreshStatus} /> : <NeedsRepo group={group} onDone={refreshStatus} />)}
        {tab === "pulls" && (hasRepo ? <PullsTab group={group} status={status} /> : <NeedsRepo group={group} onDone={refreshStatus} />)}
        {tab === "repo" && <RepoTab group={group} status={status} onChanged={refreshStatus} />}
      </div>
    </aside>
  );
}

function NeedsRepo({ group, onDone }: { group: Group; onDone: () => void }) {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const pick = async () => {
    const dir = await pickFolder();
    if (!dir) return;
    setBusy(true);
    try {
      await api.codingSetRepo(group.id, dir, group.config.worktrees !== false);
      await useStore.getState().refresh();
      onDone();
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col items-center gap-2 px-8 py-12 text-center">
      <FolderGit2 size={30} className="text-faint" />
      <p className="text-[13.5px] font-medium">{t("coding.noRepoTitle")}</p>
      <p className="text-[12.5px] text-muted">{t("coding.noRepoHint")}</p>
      <Button variant="primary" onClick={() => void pick()} disabled={busy} className="mt-1">
        {busy ? <Spinner size={13} /> : <FolderGit2 size={14} />} {t("coding.pickRepo")}
      </Button>
    </div>
  );
}

// ---------- sessions ----------

const STATE_ORDER: SessionState[] = ["working", "pending", "blocked", "done", "idle"];
const STATE_TONE: Record<SessionState, string> = { working: "bg-accent", pending: "bg-warning", blocked: "bg-danger", done: "bg-success", idle: "bg-faint" };

function Sessions({ group }: { group: Group }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const messages = useStore((s) => s.messages[group.id] ?? NO_MESSAGES);
  const working = useStore((s) => s.working[group.id] ?? NO_WORKERS);
  const approvals = useStore((s) => s.approvals);
  const questions = useStore((s) => s.questions);
  const select = useStore((s) => s.select);
  const refresh = useStore((s) => s.refresh);
  const waiting = useMemo(() => new Set([...Object.values(approvals).map((a) => a.agent_id), ...Object.values(questions).map((q) => q.agent_id)]), [approvals, questions]);
  const sessions = useMemo(() => sessionsOf(group, agents, messages, working, waiting), [group, agents, messages, working, waiting]);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const menu = useRef<HTMLDivElement>(null);
  useClickOutside(menu, () => setOpen(false), open);
  const candidates = Object.values(agents).filter((a) => !group.members.some((m) => m.agent_id === a.id));
  const worktree = (s: Session) => (s.agent.settings.working_dir && s.agent.settings.working_dir !== group.config.repo_dir ? s.agent.settings.working_dir.split(/[\\/]/).slice(-1)[0] : null);

  const addExisting = async (id: string) => {
    setOpen(false);
    await api.addGroupMember(group.id, id).catch((e) => useStore.getState().showToast(String(e)));
    await refresh();
  };
  const addCli = async (kind: (typeof CODING_CLIS)[number]) => {
    setOpen(false);
    setBusy(kind.kind);
    try {
      await addCodingAgent(group, kind.kind, kind.label);
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="px-3 py-3">
      <Activity group={group} />
      {STATE_ORDER.map((state) => {
        const list = sessions.filter((s) => s.state === state);
        if (list.length === 0) return null;
        return (
          <section key={state} className="mb-3">
            <h3 className="mb-1 flex items-center gap-2 px-2 text-[11px] font-semibold tracking-wide text-faint uppercase">
              <span className={cx("size-2 rounded-full", STATE_TONE[state], state === "working" && "animate-pulse")} />
              {t(`coding.state.${state}`)}
              <span className="font-medium text-faint">{list.length}</span>
            </h3>
            <ul>
              {list.map((s) => (
                <li key={s.agent.id} className="group flex items-start gap-2.5 rounded-xl px-2 py-2 hover:bg-hover">
                  <button onClick={() => select(s.agent.id)} className="flex min-w-0 flex-1 items-start gap-2.5 text-left">
                    <AvatarView avatar={s.agent.avatar} name={s.agent.name} size={32} />
                    <span className="min-w-0 flex-1">
                      <span className="flex items-center gap-1.5">
                        <span className="truncate text-[13.5px] font-medium">{s.agent.name}</span>
                        {s.isLead && <Badge tone="accent">{t("coding.lead")}</Badge>}
                        {state === "working" && <LoaderCircle size={12} className="animate-spin text-accent" />}
                      </span>
                      <span className="line-clamp-2 block text-[12px] leading-snug text-muted">{s.last ? `${s.last.word}: ${s.last.rest}` : s.agent.model ?? ""}</span>
                      {worktree(s) && (
                        <span className="mt-0.5 inline-flex items-center gap-1 font-mono text-[10.5px] text-faint">
                          <GitBranch size={10} /> {worktree(s)}
                        </span>
                      )}
                    </span>
                  </button>
                  {!s.isLead && (
                    <button
                      onClick={() => void api.removeGroupMember(group.id, s.agent.id).then(refresh)}
                      className="mt-1 text-faint opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                      aria-label={t("group.removeMember")}
                      data-tip={t("group.removeMember")}
                    >
                      <X size={14} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
          </section>
        );
      })}
      <div className="relative mt-1" ref={menu}>
        <button onClick={() => setOpen((v) => !v)} className="flex w-full items-center gap-2.5 rounded-xl px-2 py-2 text-left text-[13.5px] text-accent hover:bg-hover">
          <span className="flex size-8 items-center justify-center rounded-full bg-accent-soft">{busy ? <Spinner size={14} /> : <UserPlus size={15} />}</span>
          {t("coding.addAgent")}
        </button>
        {open && (
          <div className="animate-pop absolute top-full right-0 left-0 z-30 mt-1 max-h-80 overflow-y-auto rounded-xl border border-line bg-elevated p-1 shadow-pop">
            <p className="px-2.5 pt-1.5 pb-1 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("coding.cliAgents")}</p>
            {CODING_CLIS.map((c) => (
              <button key={c.kind} onClick={() => void addCli(c)} className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover">
                <Plus size={13} className="text-muted" /> {c.label}
              </button>
            ))}
            {candidates.length > 0 && <p className="mt-1 border-t border-line px-2.5 pt-2 pb-1 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("coding.yourAgents")}</p>}
            {candidates.map((a) => (
              <button key={a.id} onClick={() => void addExisting(a.id)} className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover">
                <AvatarView avatar={a.avatar} name={a.name} size={20} /> {a.name}
              </button>
            ))}
          </div>
        )}
      </div>
      <p className="px-2 pt-3 text-[11.5px] leading-snug text-faint">{t("coding.protocolHint")}</p>
    </div>
  );
}

// ---------- activity ----------

/**
 * The files the team is creating and editing right now, per working folder (each agent works in a
 * worktree of its own): updated every few seconds while anyone is working, so a long run is never
 * a black box.
 */
function Activity({ group }: { group: Group }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const busy = useStore((s) => (s.working[group.id]?.length ?? 0) > 0);
  const [lanes, setLanes] = useState<Lane[] | null>(null);
  const seen = useRef<Record<string, string>>({});
  const [fresh, setFresh] = useState<Set<string>>(new Set());
  const hasRepo = !!group.config.repo_dir;
  useEffect(() => {
    if (!hasRepo) return;
    let live = true;
    const load = () =>
      void api
        .codingActivity(group.id)
        .then((next) => {
          if (!live) return;
          // Files that appeared or grew since the last look are marked for a moment.
          const marks = new Set<string>();
          const now: Record<string, string> = {};
          for (const l of next)
            for (const f of l.files) {
              const key = `${l.dir}|${f.path}`;
              const sig = `${f.status}${f.additions}/${f.deletions}`;
              now[key] = sig;
              if (seen.current[key] !== sig && Object.keys(seen.current).length > 0) marks.add(key);
            }
          seen.current = now;
          if (marks.size) {
            setFresh(marks);
            window.setTimeout(() => live && setFresh(new Set()), 2500);
          }
          setLanes(next);
        })
        .catch(() => {});
    load();
    const timer = window.setInterval(load, busy ? 2500 : 10000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [group.id, hasRepo, busy]);

  const withFiles = (lanes ?? []).filter((l) => l.files.length > 0);
  if (!hasRepo) return null;
  return (
    <section className="mb-3" aria-label={t("coding.activity")}>
      <h3 className="mb-1 flex items-center gap-2 px-2 text-[11px] font-semibold tracking-wide text-faint uppercase">
        {busy ? <LoaderCircle size={11} className="animate-spin text-accent" /> : <FileText size={11} />}
        {t("coding.activity")}
      </h3>
      {withFiles.length === 0 ? (
        <p className="px-2 py-1 text-[12px] text-muted">{busy ? t("coding.activityWaiting") : t("coding.activityNone")}</p>
      ) : (
        withFiles.map((l) => (
          <div key={l.dir} className="mb-2 rounded-xl border border-line px-2.5 py-2">
            <div className="mb-1 flex items-center gap-1.5 text-[12px]">
              <span className="flex -space-x-1">
                {l.agent_ids.map((id) => agents[id] && <AvatarView key={id} avatar={agents[id].avatar} name={agents[id].name} size={16} />)}
              </span>
              <span className="min-w-0 flex-1 truncate font-medium">{l.agent_ids.map((id) => agents[id]?.name).filter(Boolean).join(", ")}</span>
              {l.branch && (
                <span className="inline-flex shrink-0 items-center gap-0.5 font-mono text-[10.5px] text-faint">
                  <GitBranch size={10} /> {l.branch}
                </span>
              )}
            </div>
            <ul>
              {l.files.slice(0, 12).map((f) => (
                <li key={f.path} className={cx("flex items-center gap-2 rounded-md px-1 py-0.5 transition-colors duration-700", fresh.has(`${l.dir}|${f.path}`) && "bg-accent-soft")}>
                  <span className={cx("w-3.5 text-center font-mono text-[10.5px] font-bold", f.status === "A" || f.status === "?" ? "text-success" : f.status === "D" ? "text-danger" : "text-warning")}>{f.status}</span>
                  <span className="min-w-0 flex-1 truncate font-mono text-[11.5px]">{f.path}</span>
                  {(f.additions > 0 || f.deletions > 0) && (
                    <span className="shrink-0 font-mono text-[10.5px]">
                      <span className="text-success">+{f.additions}</span> <span className="text-danger">−{f.deletions}</span>
                    </span>
                  )}
                </li>
              ))}
              {l.files.length > 12 && <li className="px-1 pt-0.5 text-[11px] text-faint">{t("coding.moreFiles", { count: l.files.length - 12 })}</li>}
            </ul>
          </div>
        ))
      )}
    </section>
  );
}

// ---------- changes ----------

function parsePatch(patch: string): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  let cur = "";
  for (const line of patch.split("\n")) {
    const m = line.match(/^diff --git a\/(.+?) b\/(.+)$/);
    if (m) {
      cur = m[2];
      out[cur] = [];
    } else if (cur) out[cur].push(line);
  }
  return out;
}

function ChangesTab({ group, status, onChanged }: { group: Group; status: RepoStatus | null; onChanged: () => void }) {
  const { t } = useTranslation();
  const [changes, setChanges] = useState<Changes | null>(null);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [pr, setPr] = useState(false);
  const load = useCallback(() => {
    void api
      .githubChanges(group.id)
      .then((c) => (setChanges(c), setError("")))
      .catch((e) => setError(String(e)));
  }, [group.id]);
  useEffect(() => {
    load();
    const timer = window.setInterval(load, 6000);
    return () => window.clearInterval(timer);
  }, [load]);
  const patch = useMemo(() => parsePatch(changes?.patch ?? ""), [changes?.patch]);

  if (error) return <p className="px-5 py-8 text-center text-[12.5px] text-danger">{error}</p>;
  if (!changes) return <div className="flex justify-center py-10"><Spinner /></div>;
  return (
    <div className="px-3 py-3">
      <div className="mb-2 flex items-center gap-2 px-2">
        <span className="text-[13px] font-medium">{t("coding.filesChanged", { count: changes.files.length })}</span>
        <span className="font-mono text-[12px] text-success">+{status?.additions ?? 0}</span>
        <span className="font-mono text-[12px] text-danger">−{status?.deletions ?? 0}</span>
        <div className="ml-auto flex items-center">
          <IconButton label={t("computer.reload")} onClick={load} className="size-7">
            <RefreshCw size={13} />
          </IconButton>
          <Button size="sm" variant="primary" onClick={() => setPr(true)}>
            <GitPullRequest size={13} /> {t("coding.createPr")}
          </Button>
        </div>
      </div>
      {changes.files.length === 0 ? (
        <p className="px-2 py-8 text-center text-[12.5px] text-muted">{t("coding.clean")}</p>
      ) : (
        <ul className="space-y-0.5">
          {changes.files.map((f) => (
            <li key={f.path}>
              <button onClick={() => setOpen(open === f.path ? null : f.path)} className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left hover:bg-hover">
                <span className={cx("w-4 text-center font-mono text-[11px] font-bold", f.status === "A" || f.status === "?" ? "text-success" : f.status === "D" ? "text-danger" : "text-warning")}>{f.status}</span>
                <span className="min-w-0 flex-1 truncate font-mono text-[12px]">{f.path}</span>
                <span className="font-mono text-[11px] text-success">+{f.additions}</span>
                <span className="font-mono text-[11px] text-danger">−{f.deletions}</span>
                <ChevronDown size={13} className={cx("text-faint transition-transform", open === f.path && "rotate-180")} />
              </button>
              {open === f.path && (
                <pre className="selectable mx-1 mb-1 max-h-72 overflow-auto rounded-lg bg-hover/60 py-1.5 font-mono text-[11px] leading-[1.5]">
                  {(patch[f.path] ?? []).length === 0 ? <span className="px-3 text-muted">{f.status === "?" ? t("coding.untracked") : t("coding.noDiff")}</span> : (patch[f.path] ?? []).map((l, i) => (
                    <span key={i} className={cx("block px-3 whitespace-pre", l.startsWith("+") && !l.startsWith("+++") && "bg-success/10 text-success", l.startsWith("-") && !l.startsWith("---") && "bg-danger/10 text-danger", l.startsWith("@@") && "text-accent")}>
                      {l || " "}
                    </span>
                  ))}
                </pre>
              )}
            </li>
          ))}
        </ul>
      )}
      {changes.truncated && <p className="px-2 pt-2 text-[11.5px] text-faint">{t("coding.truncated")}</p>}
      {pr && <CreatePrDialog group={group} status={status} onClose={() => setPr(false)} onCreated={onChanged} />}
    </div>
  );
}

function CreatePrDialog({ group, status, onClose, onCreated }: { group: Group; status: RepoStatus | null; onClose: () => void; onCreated: () => void }) {
  const { t } = useTranslation();
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [draft, setDraft] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async () => {
    setBusy(true);
    setError("");
    try {
      const url = await api.githubCreatePr(group.id, title, body, draft);
      onCreated();
      onClose();
      useStore.getState().showToast(url || t("coding.prCreated"));
      if (url.startsWith("https://")) void openUrl(url);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal open onClose={onClose} className="w-[480px]" label={t("coding.createPr")}>
      <DialogHeader title={t("coding.createPr")} subtitle={t("coding.createPrHint", { branch: status?.branch ?? "", base: status?.default_branch ?? "main" })} onClose={onClose} />
      <div className="space-y-3.5 px-6 pt-2 pb-5">
        {status && status.changed_files > 0 && <p className="rounded-lg bg-warning-soft px-3 py-2 text-[12.5px]">{t("coding.uncommitted", { count: status.changed_files })}</p>}
        <Field label={t("coding.prTitle")}>
          <Input autoFocus value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label={t("coding.prBody")}>
          <Textarea rows={5} value={body} onChange={(e) => setBody(e.target.value)} />
        </Field>
        <label className="flex items-center justify-between text-[13px]">
          {t("coding.draft")}
          <Toggle checked={draft} onChange={setDraft} label={t("coding.draft")} />
        </label>
        {error && <p className="selectable text-[12px] whitespace-pre-wrap text-danger">{error}</p>}
        <div className="flex justify-end gap-2">
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!title.trim() || busy} onClick={() => void submit()}>
            {busy && <Spinner size={13} />} {t("coding.createPr")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

// ---------- pull requests ----------

function CheckDots({ p }: { p: Pull }) {
  const { t } = useTranslation();
  if (p.checks.total === 0) return <span className="text-[11.5px] text-faint">{t("coding.noChecks")}</span>;
  const tone = p.checks.failed ? "text-danger" : p.checks.pending ? "text-warning" : "text-success";
  return (
    <span className={cx("inline-flex items-center gap-1 text-[11.5px]", tone)} title={p.checks.failing.join(", ")}>
      {p.checks.failed ? <CircleAlert size={12} /> : p.checks.pending ? <LoaderCircle size={12} className="animate-spin" /> : <Check size={12} strokeWidth={3} />}
      {p.checks.failed ? t("coding.checksFailed", { failed: p.checks.failed, total: p.checks.total }) : p.checks.pending ? t("coding.checksPending", { done: p.checks.passed, total: p.checks.total }) : t("coding.checksPassed", { total: p.checks.total })}
    </span>
  );
}

function PullsTab({ group, status }: { group: Group; status: RepoStatus | null }) {
  const { t } = useTranslation();
  const [pulls, setPulls] = useState<Pull[] | null>(null);
  const [error, setError] = useState("");
  const [filter, setFilter] = useState<"open" | "merged" | "closed">("open");
  const [busy, setBusy] = useState<string | null>(null);
  const showToast = useStore((s) => s.showToast);
  const load = useCallback(() => {
    void api
      .githubPulls(group.id, filter)
      .then((p) => (setPulls(p), setError("")))
      .catch((e) => setError(String(e)));
  }, [group.id, filter]);
  useEffect(() => {
    setPulls(null);
    load();
    const timer = window.setInterval(load, 30000);
    return () => window.clearInterval(timer);
  }, [load]);

  const act = async (key: string, run: () => Promise<unknown>) => {
    setBusy(key);
    try {
      const r = await run();
      if (typeof r === "string" && r) showToast(r);
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  if (status && !status.gh_installed) {
    return (
      <div className="flex flex-col items-center gap-2 px-8 py-12 text-center">
        <GitPullRequest size={30} className="text-faint" />
        <p className="text-[13.5px] font-medium">{t("coding.ghMissing")}</p>
        <p className="text-[12.5px] text-muted">{t("coding.ghMissingHint")}</p>
        <Button variant="primary" onClick={() => void openUrl("https://cli.github.com")} className="mt-1">
          <ArrowUpRight size={14} /> {t("coding.ghInstall")}
        </Button>
      </div>
    );
  }
  if (status && status.gh_installed && !status.gh_user) {
    return (
      <div className="flex flex-col items-center gap-2 px-8 py-12 text-center">
        <GitPullRequest size={30} className="text-faint" />
        <p className="text-[13.5px] font-medium">{t("coding.ghSignIn")}</p>
        <p className="text-[12.5px] text-muted">{t("coding.ghSignInHint")}</p>
        <code className="mt-1 rounded-md bg-hover px-2 py-1 font-mono text-[12px]">gh auth login</code>
      </div>
    );
  }
  return (
    <div className="px-3 py-3">
      <div className="mb-2 flex items-center gap-1 px-1">
        {(["open", "merged", "closed"] as const).map((f) => (
          <button key={f} onClick={() => setFilter(f)} className={cx("rounded-lg px-2.5 py-1 text-[12.5px] font-medium", filter === f ? "bg-active text-ink" : "text-muted hover:bg-hover")}>
            {t(`coding.pr.${f}`)}
          </button>
        ))}
        <IconButton label={t("computer.reload")} onClick={load} className="ml-auto size-7">
          <RefreshCw size={13} />
        </IconButton>
      </div>
      {error ? (
        <p className="selectable px-3 py-6 text-center text-[12.5px] whitespace-pre-wrap text-danger">{error}</p>
      ) : !pulls ? (
        <div className="flex justify-center py-10"><Spinner /></div>
      ) : pulls.length === 0 ? (
        <p className="px-2 py-8 text-center text-[12.5px] text-muted">{t("coding.noPulls")}</p>
      ) : (
        <ul className="space-y-2">
          {pulls.map((p) => (
            <li key={p.number} className="rounded-xl border border-line p-3">
              <div className="flex items-start gap-2">
                <span className={cx("mt-0.5 shrink-0", filter === "merged" ? "text-accent" : p.draft ? "text-faint" : "text-success")}>{p.draft ? <GitPullRequestDraft size={15} /> : <GitPullRequest size={15} />}</span>
                <button onClick={() => void openUrl(p.url)} className="min-w-0 flex-1 text-left">
                  <span className="line-clamp-2 text-[13.5px] leading-snug font-medium hover:underline">{p.title}</span>
                  <span className="mt-0.5 flex flex-wrap items-center gap-x-2 text-[11.5px] text-muted">
                    <span>#{p.number}</span>
                    <span className="font-mono">{p.branch} → {p.base}</span>
                    <span>{shortTime(p.updated_at)}</span>
                  </span>
                </button>
              </div>
              <div className="mt-2 flex flex-wrap items-center gap-1.5">
                <CheckDots p={p} />
                {p.review === "APPROVED" && <Badge tone="success">{t("coding.approved")}</Badge>}
                {p.review === "CHANGES_REQUESTED" && <Badge tone="danger">{t("coding.changesRequested")}</Badge>}
                {p.review === "REVIEW_REQUIRED" && <Badge tone="warning">{t("coding.reviewRequired")}</Badge>}
                {p.draft && <Badge>{t("coding.draft")}</Badge>}
                {p.mergeable === "CONFLICTING" && <Badge tone="danger">{t("coding.conflicts")}</Badge>}
                {p.auto_merge && <Badge tone="accent">{t("coding.autoMerge")}</Badge>}
                <span className="ml-auto font-mono text-[11px]"><span className="text-success">+{p.additions}</span> <span className="text-danger">−{p.deletions}</span></span>
              </div>
              {filter === "open" && (
                <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
                  {p.checks.failed > 0 && (
                    <Button size="sm" onClick={() => void act(`fix-${p.number}`, async () => { await api.codingFixCi(group.id, p.number); return t("coding.fixSent"); })} disabled={busy !== null}>
                      {busy === `fix-${p.number}` ? <Spinner size={12} /> : <Wrench size={12} />} {t("coding.fixCi")}
                    </Button>
                  )}
                  {!p.draft && !p.auto_merge && (
                    <Button size="sm" onClick={() => void act(`auto-${p.number}`, () => api.githubMergePr(group.id, p.number, "squash", true))} disabled={busy !== null}>
                      {busy === `auto-${p.number}` ? <Spinner size={12} /> : <CircleDot size={12} />} {t("coding.mergeWhenReady")}
                    </Button>
                  )}
                  {!p.draft && p.checks.failed === 0 && p.checks.pending === 0 && p.mergeable !== "CONFLICTING" && (
                    <Button
                      size="sm"
                      variant="primary"
                      onClick={async () => {
                        if (await confirmDialog(t("coding.mergeConfirm", { n: p.number, title: p.title }), { confirmLabel: t("coding.mergeNow"), danger: false })) void act(`merge-${p.number}`, () => api.githubMergePr(group.id, p.number, "squash", false));
                      }}
                      disabled={busy !== null}
                    >
                      {t("coding.mergeNow")}
                    </Button>
                  )}
                </div>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

// ---------- repository ----------

function RepoTab({ group, status, onChanged }: { group: Group; status: RepoStatus | null; onChanged: () => void }) {
  const { t } = useTranslation();
  const refresh = useStore((s) => s.refresh);
  const setPanel = useStore((s) => s.setPanel);
  const [name, setName] = useState(group.name);
  useEffect(() => setName(group.name), [group.id, group.name]);
  const worktrees = group.config.worktrees !== false;

  const link = async (dir: string, isolate: boolean) => {
    try {
      await api.codingSetRepo(group.id, dir, isolate);
      await refresh();
      onChanged();
    } catch (e) {
      useStore.getState().showToast(String(e));
    }
  };
  const pick = async () => {
    const dir = await pickFolder();
    if (dir) await link(dir, worktrees);
  };

  return (
    <div className="space-y-4 px-5 py-4">
      <Field label={t("group.name")}>
        <input
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => name.trim() && name !== group.name && void api.updateGroup(group.id, name).then(refresh)}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
          className="w-full rounded-lg border border-line bg-bg px-3 py-1.5 text-[13.5px] outline-none focus:border-accent"
        />
      </Field>
      <Field label={t("coding.repoFolder")}>
        <div className="flex items-center gap-2">
          <span className="selectable min-w-0 flex-1 truncate rounded-lg bg-hover px-3 py-1.5 font-mono text-[12px]">{group.config.repo_dir ?? t("coding.noRepo")}</span>
          <Button onClick={() => void pick()}>{t("coding.choose")}</Button>
        </div>
      </Field>
      {status && (
        <div className="grid grid-cols-2 gap-2 text-[12.5px]">
          <Stat label={t("coding.branch")} value={status.branch ?? "—"} mono />
          <Stat label={t("coding.default")} value={status.default_branch ?? "—"} mono />
          <Stat label={t("coding.sync")} value={`↑${status.ahead} ↓${status.behind}`} mono />
          <Stat label="GitHub" value={status.gh_user ? `@${status.gh_user}` : status.gh_installed ? t("coding.notSignedIn") : t("coding.ghMissingShort")} />
        </div>
      )}
      <label className="flex items-center justify-between gap-4 text-[13px]">
        <span>
          {t("coding.worktrees")}
          <span className="block text-[11.5px] text-muted">{t("coding.worktreesHint")}</span>
        </span>
        <Toggle checked={worktrees} onChange={(v) => group.config.repo_dir && void link(group.config.repo_dir, v)} label={t("coding.worktrees")} />
      </label>
      {status && status.worktrees.length > 1 && (
        <ul className="space-y-1 rounded-xl border border-line p-2">
          {status.worktrees.map((w) => (
            <li key={w.path} className="flex items-center gap-2 px-1.5 py-1 text-[11.5px]">
              <GitBranch size={12} className="shrink-0 text-muted" />
              <span className="font-mono">{w.branch || "(detached)"}</span>
              <span className="min-w-0 flex-1 truncate text-right font-mono text-faint">{w.path}</span>
            </li>
          ))}
        </ul>
      )}
      <Field label={t("group.autoTurns")}>
        <div className="flex items-center gap-3">
          <Input type="number" min={0} max={50} defaultValue={group.max_auto_turns} key={group.id} onBlur={(e) => void api.updateGroup(group.id, undefined, Number(e.target.value)).then(refresh)} className="w-20" />
          <p className="text-[11.5px] text-muted">{t("group.autoTurnsHint")}</p>
        </div>
      </Field>
      <div>
        <h3 className="mb-1.5 text-[12.5px] font-medium text-muted">{t("group.files")}</h3>
        <SharedFiles threadId={group.id} />
      </div>
      <Button
        variant="danger"
        className="w-full"
        onClick={async () => {
          if (await confirmDialog(t("group.deleteConfirm", { name: group.name }), { confirmLabel: t("common.delete") })) {
            setPanel(null);
            void api.deleteGroup(group.id);
          }
        }}
      >
        <Trash2 size={15} /> {t("group.deleteGroup")}
      </Button>
    </div>
  );
}

function Stat({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-lg bg-hover/60 px-3 py-2">
      <div className="text-[11px] text-faint">{label}</div>
      <div className={cx("truncate text-[13px]", mono && "font-mono")}>{value}</div>
    </div>
  );
}
