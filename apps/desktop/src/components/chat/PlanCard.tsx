import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, CircleAlert, Crown, Square } from "lucide-react";
import { api } from "../../lib/api";
import { cx, splitThink } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { Agent, Message, ToolCall } from "../../lib/types";
import { AvatarView } from "../AvatarView";
import { describe } from "./ToolCard";

interface Step {
  step: string;
  status: "pending" | "doing" | "done";
  /** Who does it, by name (the orchestrator hands steps out in a group). */
  agent?: string;
}

/** The steps of the latest `update_plan` call, and whose list it is (the orchestrator's). */
function planSteps(messages: Message[]): { steps: Step[]; owner: string | null } {
  let steps: Step[] = [];
  // Only one agent keeps the checklist: the first one to write it.
  let owner: string | null | undefined;
  for (const m of messages) {
    for (const c of m.tool_calls ?? []) {
      if (c.name !== "update_plan") continue;
      if (owner === undefined) owner = m.agent_id;
      else if (m.agent_id !== owner) continue;
      const raw = (c.arguments as { steps?: unknown }).steps;
      if (Array.isArray(raw)) {
        steps = raw
          .map((x) => x as { step?: unknown; status?: unknown; agent?: unknown })
          .filter((x) => typeof x.step === "string" && x.step.trim())
          .map((x) => ({
            step: String(x.step),
            status: x.status === "done" ? "done" : x.status === "doing" ? "doing" : "pending",
            agent: typeof x.agent === "string" && x.agent.trim() ? x.agent.trim().replace(/^@/, "") : undefined,
          }));
      }
    }
  }
  return { steps, owner: owner ?? null };
}

/** An agent message that is an answer, not narration between tool calls. */
function isAnswer(m: Message) {
  return m.role === "assistant" && !m.tool_calls?.length && !(m.status === "streaming" && !m.content);
}

/** The orchestrator of a run: whoever keeps the checklist, or the first agent to speak. */
function runLead(run: Message[], owner: string | null) {
  return owner ?? run.find((m) => m.agent_id && m.role !== "event")?.agent_id ?? null;
}

/**
 * The messages of a plan run that the chat shows as their own bubbles: answers, except a
 * teammate's `DONE:` / `BLOCKED:` report to the lead, which the checklist already shows.
 * Everything else lives in the card (Open run), so nothing appears twice.
 */
export function chatMessages(run: Message[]): Set<string> {
  const lead = runLead(run, planSteps(run).owner);
  const ids = new Set<string>();
  for (const m of run) {
    if (!isAnswer(m)) continue;
    const report = m.agent_id !== lead && /^\s*(@[^\s:]+[\s,:]+)?(DONE|BLOCKED)\b/i.test(splitThink(m.content).text);
    if (!report) ids.add(m.id);
  }
  return ids;
}

const NO_IDS: string[] = [];

function clock(ms: number) {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

type Phase = "planning" | "working" | "waiting" | "done" | "failed" | "stopped";

/**
 * The card a `/plan` run lives in. One agent orchestrates: it writes the checklist and hands the
 * steps out; the card shows who takes part, the checklist ticking itself (with who does each step),
 * what is happening right now, and Approve / Deny rows when someone is waiting on you. Answers stay
 * in the chat as their own messages; **Open run** holds only what happened between them.
 */
export function PlanCard({ threadId, started, task, run, active }: { threadId: string; started: string; task: string; run: Message[]; active: boolean }) {
  const { t } = useTranslation();
  const approvals = useStore((s) => s.approvals);
  const questions = useStore((s) => s.questions);
  const agents = useStore((s) => s.agents);
  const workingIds = useStore((s) => s.working[threadId]) ?? NO_IDS;
  const [open, setOpen] = useState(false);
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!active) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [active]);

  const { steps, owner } = useMemo(() => planSteps(run), [run]);
  const calls = useMemo(() => {
    const map: Record<string, ToolCall> = {};
    for (const m of run) for (const c of m.tool_calls ?? []) map[c.id] = c;
    return map;
  }, [run]);
  const tools = run.filter((m) => m.role === "tool" && m.tool_name !== "update_plan");
  // Every agent waiting on you, not just the first: parallel agents ask at the same time.
  const waiting = run.filter((m) => m.role === "tool" && m.status === "awaiting_approval" && approvals[m.id]);
  const lastReply = [...run].reverse().find((m) => m.role === "assistant");
  const failed = !active && lastReply?.status === "error";
  const stopped = !active && !failed && (lastReply?.status === "stopped" || run.some((m) => m.status === "stopped"));
  const done = steps.filter((s) => s.status === "done").length;
  const doing = steps.find((s) => s.status === "doing");
  const phase: Phase = waiting.length ? "waiting" : failed ? "failed" : stopped ? "stopped" : !active ? "done" : steps.length === 0 ? "planning" : "working";

  // The orchestrator; before anyone has spoken, whoever started working.
  const leadId = runLead(run, owner) ?? workingIds[0] ?? null;
  const lead = leadId ? agents[leadId] : undefined;
  const byName = useMemo(() => {
    const map: Record<string, Agent> = {};
    for (const a of Object.values(agents)) map[a.name.toLowerCase()] = a;
    return map;
  }, [agents]);

  // Everyone taking part (the lead first, then whoever has a step or spoke), with what they are doing.
  const askers = useMemo(() => new Set(Object.values(questions).map((q) => q.agent_id)), [questions]);
  const team = useMemo(() => {
    const ids: string[] = leadId ? [leadId] : [];
    const add = (id?: string | null) => id && !ids.includes(id) && ids.push(id);
    for (const s of steps) add(s.agent ? byName[s.agent.toLowerCase()]?.id : null);
    for (const m of run) if (m.role !== "event") add(m.agent_id);
    return ids
      .map((id) => agents[id])
      .filter(Boolean)
      .map((a) => {
        const busy = workingIds.includes(a.id);
        const asking = askers.has(a.id) || waiting.some((m) => m.agent_id === a.id);
        return { agent: a, state: busy ? (asking ? "waiting" : "working") : "idle" } as const;
      });
  }, [leadId, steps, run, agents, byName, workingIds, askers, waiting]);
  const busyNames = team.filter((x) => x.state !== "idle").map((x) => x.agent.name);

  // What the agents say between tool calls (and teammates' reports); answers are in the chat.
  const { log, notes } = useMemo(() => {
    const inChat = chatMessages(run);
    const said = (m: Message) => m.role === "assistant" && m.status !== "error" && !inChat.has(m.id) && !!splitThink(m.content).text.trim();
    return {
      log: run.filter((m) => said(m) || (m.role === "tool" && m.tool_name !== "update_plan") || m.role === "event"),
      notes: run.filter(said),
    };
  }, [run]);
  const last = active ? notes[notes.length - 1] : undefined;
  const lastNote = last ? { m: last, text: splitThink(last.content).text.trim() } : undefined;

  const startMs = new Date(started).getTime();
  const lastMs = run.length ? new Date(run[run.length - 1].created_at).getTime() : startMs;
  const elapsed = (active ? now : lastMs) - startMs;
  const pct = steps.length ? (done / steps.length) * 100 : phase === "done" ? 100 : 0;
  const action = (m: Message) => describe(m.tool_name ?? "tool", calls[m.tool_call_id ?? ""]?.arguments, t).title.toLowerCase();

  let status: string;
  if (phase === "waiting") status = t("plan.needsApproval", { action: action(waiting[0]) });
  else if (phase === "failed") status = t("plan.failed");
  else if (phase === "stopped") status = t("plan.stopped");
  else if (phase === "done") status = steps.length || tools.length ? t("plan.done", { n: steps.length || tools.length }) : t("plan.doneNoSteps");
  else if (phase === "planning") status = lead ? t("plan.planningBy", { name: lead.name }) : t("plan.planning");
  else if (busyNames.length > 1) status = t("plan.agentsWorking", { names: busyNames.join(", ") });
  else status = doing?.step ?? t("plan.working");
  const title = task || t("plan.untitled");
  const live = phase === "planning" || phase === "working";

  return (
    <div className="plan-card animate-pop w-full max-w-[560px]" data-phase={phase} data-plan-card>
      {/* Header: the task, how long it has taken, and a progress line that fills as steps tick. */}
      <div className="flex items-center gap-3 px-4 pt-4">
        <span className="relative flex size-9 shrink-0 items-center justify-center rounded-[12px] bg-accent-soft">
          {lead ? <AvatarView avatar={lead.avatar} name={lead.name} size={28} /> : <span className="size-3 rounded-full bg-accent" />}
          {(live || phase === "waiting") && <span className={cx("absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-[var(--plan-bg)]", phase === "waiting" ? "bg-warning" : "live-dot bg-accent")} />}
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[14.5px] font-semibold tracking-[-0.01em]" title={title}>{title}</div>
          <div key={status} className={cx("animate-fade truncate text-[12.5px]", phase === "waiting" ? "text-warning" : phase === "failed" ? "text-danger" : "text-muted")}>
            {live ? <span className="shimmer-text">{status}</span> : status}
          </div>
        </div>
        <div className="shrink-0 text-right">
          <div className="font-mono text-[12.5px] text-muted tabular-nums">{clock(elapsed)}</div>
          {steps.length > 0 && <div className="text-[11px] text-faint tabular-nums">{t("plan.progress", { done, total: steps.length })}</div>}
        </div>
      </div>
      <div className="mx-4 mt-3 h-[5px] overflow-hidden rounded-full bg-hover" role="progressbar" aria-valuemin={0} aria-valuemax={steps.length || 1} aria-valuenow={done}>
        <div className={cx("plan-bar h-full rounded-full transition-[width] duration-700 ease-out", live && "is-live", phase === "planning" && "is-planning")} style={{ width: phase === "planning" ? "100%" : `${Math.max(pct, live ? 4 : 0)}%` }} />
      </div>

      {/* Who is on the run; the orchestrator is marked. */}
      {team.length > 0 && (
        <div className="flex flex-wrap gap-1.5 px-4 pt-3" aria-label={t("plan.team")}>
          {team.map(({ agent: a, state }) => (
            <span
              key={a.id}
              title={a.id === leadId ? t("plan.leadHint") : undefined}
              className={cx(
                "inline-flex items-center gap-1.5 rounded-full border py-[3px] pr-2.5 pl-[3px] text-[12px] transition-colors",
                state === "working" ? "border-accent/35 bg-accent-soft text-ink" : state === "waiting" ? "border-warning/50 bg-warning-soft" : "border-line bg-[var(--plan-bg)] text-muted",
              )}
            >
              <span className={cx("rounded-full", state === "working" && "agent-bob")}>
                <AvatarView avatar={a.avatar} name={a.name} size={18} />
              </span>
              {a.name}
              {a.id === leadId && team.length > 1 && (
                <span className="inline-flex items-center gap-0.5 rounded-full bg-ink/[0.06] px-1.5 text-[10px] font-semibold tracking-wide text-muted uppercase">
                  <Crown size={9} /> {t("plan.lead")}
                </span>
              )}
              <span className={cx("size-1.5 rounded-full", state === "working" ? "live-dot bg-accent" : state === "waiting" ? "bg-warning" : "bg-faint/70")} />
            </span>
          ))}
        </div>
      )}

      {/* The checklist, under the agents: it ticks itself as steps are done. */}
      {steps.length > 0 ? (
        <ol className="mx-4 mt-3 space-y-px rounded-[14px] border border-line bg-[var(--plan-inset)] p-1.5" aria-label={t("plan.checklist")}>
          {steps.map((st, i) => {
            const who = st.agent ? byName[st.agent.toLowerCase()] : undefined;
            const current = st.status === "doing" && live;
            return (
              <li
                key={i}
                style={{ animationDelay: `${i * 45}ms` }}
                className={cx("animate-rise flex items-center gap-2.5 rounded-[10px] px-2 py-[7px] text-[13.5px] transition-colors", current && "step-current")}
              >
                <span className="shrink-0">
                  {st.status === "done" ? (
                    <span className="check-pop flex size-[18px] items-center justify-center rounded-[6px] bg-success text-white">
                      <Check size={12} strokeWidth={3.5} />
                    </span>
                  ) : current ? (
                    <span className="step-ring block size-[18px] rounded-[6px]" />
                  ) : (
                    <span className="block size-[18px] rounded-[6px] border-[1.5px] border-line-strong" />
                  )}
                </span>
                <span className={cx("min-w-0 flex-1 leading-snug transition-colors", st.status === "done" ? "text-muted line-through decoration-faint/70" : current ? "font-medium text-ink" : "text-ink/85")}>{st.step}</span>
                {who ? (
                  <span className="flex shrink-0 items-center gap-1 text-[11.5px] text-muted" title={who.name}>
                    <AvatarView avatar={who.avatar} name={who.name} size={16} />
                    <span className="max-w-[84px] truncate">{who.name}</span>
                  </span>
                ) : st.agent ? (
                  <span className="max-w-[90px] shrink-0 truncate text-[11.5px] text-muted">{st.agent}</span>
                ) : null}
              </li>
            );
          })}
        </ol>
      ) : (
        phase === "planning" && (
          <div className="mx-4 mt-3 space-y-2 rounded-[14px] border border-line bg-[var(--plan-inset)] p-3" aria-label={t("plan.planning")}>
            {[78, 62, 70].map((w, i) => (
              <div key={i} className="flex items-center gap-2.5">
                <span className="skeleton size-[18px] rounded-[6px]" />
                <span className="skeleton h-2.5 rounded-full" style={{ width: `${w}%`, animationDelay: `${i * 120}ms` }} />
              </div>
            ))}
          </div>
        )
      )}

      {/* What is happening right now. */}
      {lastNote && (
        <p key={lastNote.m.id} className="animate-fade mx-4 mt-3 line-clamp-2 border-l-2 border-accent/50 pl-3 text-[13px] leading-snug text-muted">
          {team.length > 1 && lastNote.m.agent_id && agents[lastNote.m.agent_id] && <span className="font-medium text-ink">{agents[lastNote.m.agent_id].name}: </span>}
          {lastNote.text}
        </p>
      )}

      {waiting.length > 0 && (
        <ul className="space-y-1.5 px-4 pt-3">
          {waiting.map((w) => {
            const who = w.agent_id ? agents[w.agent_id] : undefined;
            const ap = approvals[w.id];
            return (
              <li key={w.id} className="animate-rise flex items-center gap-2 rounded-xl border border-warning/40 bg-warning-soft px-2.5 py-1.5 text-[13px]">
                {who && <AvatarView avatar={who.avatar} name={who.name} size={18} />}
                <span className="min-w-0 flex-1 truncate">
                  {who && team.length > 1 && <span className="font-medium">{who.name}: </span>}
                  {t("plan.waiting", { action: action(w) })}
                </span>
                <button onClick={() => void api.resolveApproval(ap.id, true)} className="rounded-full btn-primary px-3 py-1 text-[12.5px] font-medium text-white">{t("plan.approve")}</button>
                <button onClick={() => void api.resolveApproval(ap.id, false)} className="rounded-full border border-line bg-[var(--plan-bg)] px-3 py-1 text-[12.5px] hover:bg-hover">{t("plan.deny")}</button>
              </li>
            );
          })}
        </ul>
      )}

      {/* Open run: the timeline of tool calls, notes and events (answers are in the chat). */}
      {open && (
        <div className="animate-rise mx-4 mt-3 max-h-80 overflow-y-auto rounded-[14px] border border-line bg-[var(--plan-inset)] px-3 py-2.5 text-[12.5px]">
          <p className="mb-2 text-[10.5px] font-semibold tracking-[0.06em] text-faint uppercase">{t("plan.activity")}</p>
          {log.length === 0 && <p className="py-1 text-muted">{t("plan.noEvents")}</p>}
          <ul className="relative space-y-2 before:absolute before:top-1 before:bottom-1 before:left-[7.5px] before:w-px before:bg-line">
            {log.map((m) => {
              const who = m.agent_id ? agents[m.agent_id] : undefined;
              const call = m.tool_call_id ? calls[m.tool_call_id] : undefined;
              const d = m.role === "tool" ? describe(m.tool_name ?? call?.name ?? "tool", call?.arguments, t) : null;
              return (
                <li key={m.id} className="relative flex items-start gap-2.5">
                  <span className="relative mt-0.5 shrink-0 rounded-full bg-[var(--plan-inset)]">
                    {who ? <AvatarView avatar={who.avatar} name={who.name} size={16} /> : <span className="m-[4px] block size-2 rounded-full bg-faint" />}
                  </span>
                  <span className="selectable min-w-0 flex-1 whitespace-pre-wrap break-words">
                    {d ? (
                      <>
                        <span className={cx("text-ink", m.status === "error" && "text-danger")}>{d.title}</span>
                        {d.detail && <span className="ml-1.5 text-muted">{d.detail}</span>}
                      </>
                    ) : (
                      <span className={m.role === "event" ? "text-muted italic" : "text-ink/90"}>{m.role === "assistant" ? splitThink(m.content).text.trim().slice(0, 600) : m.content.slice(0, 300)}</span>
                    )}
                  </span>
                  <span className="shrink-0 text-[11px] text-faint tabular-nums">{new Date(m.created_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</span>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      <div className="flex items-center gap-2.5 px-4 pt-3 pb-3.5">
        {active ? (
          <button onClick={() => void api.stopThread(threadId)} className="inline-flex items-center gap-1.5 rounded-full border border-line bg-[var(--plan-bg)] px-3.5 py-1.5 text-[13px] transition-colors hover:bg-hover">
            <Square size={10} fill="currentColor" /> {t("chat.stop")}
          </button>
        ) : failed ? (
          <span className="inline-flex items-center gap-1.5 text-[13px] text-danger">
            <CircleAlert size={14} /> {t("plan.failedHint")}
          </span>
        ) : phase === "done" ? (
          <span className="inline-flex items-center gap-1.5 text-[12.5px] text-success">
            <Check size={14} strokeWidth={2.6} /> {t("plan.completedIn", { time: clock(elapsed) })}
          </span>
        ) : null}
        <button onClick={() => setOpen((v) => !v)} aria-expanded={open} className="ml-auto inline-flex items-center gap-1 rounded-full px-2 py-1 text-[13px] text-muted transition-colors hover:bg-hover hover:text-ink">
          {t("plan.openRun")}
          {log.length > 0 && <span className="rounded-full bg-hover px-1.5 text-[10.5px] tabular-nums">{log.length}</span>}
          <ChevronDown size={14} className={cx("transition-transform duration-200", open && "rotate-180")} />
        </button>
      </div>
    </div>
  );
}
