import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, CircleAlert, Square } from "lucide-react";
import { api } from "../../lib/api";
import { cx, splitThink } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { Agent, ApprovalRequest, Message, ToolCall } from "../../lib/types";
import { AvatarView } from "../AvatarView";
import { Spinner } from "../ui";
import { describe } from "./ToolCard";

interface Step {
  step: string;
  status: "pending" | "doing" | "done";
}

/** The steps of the latest `update_plan` call. */
export function planSteps(messages: Message[]): Step[] {
  let steps: Step[] = [];
  // Several agents may keep a list; the checklist is the first one's (the lead's).
  let owner: string | null | undefined;
  for (const m of messages) {
    for (const c of m.tool_calls ?? []) {
      if (c.name !== "update_plan") continue;
      if (owner === undefined) owner = m.agent_id;
      else if (m.agent_id !== owner) continue;
      const raw = (c.arguments as { steps?: unknown }).steps;
      if (Array.isArray(raw)) {
        steps = raw
          .map((x) => x as { step?: unknown; status?: unknown })
          .filter((x) => typeof x.step === "string" && x.step.trim())
          .map((x) => ({ step: String(x.step), status: x.status === "done" ? "done" : x.status === "doing" ? "doing" : "pending" }));
      }
    }
  }
  return steps;
}

const NO_IDS: string[] = [];

function clock(ms: number) {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/**
 * The card a `/plan` run lives in: what the agent is doing right now, how far along the plan is
 * (one segment per step), each thing it did and how long it took, and an Approve / Deny pair when
 * it is waiting on you. It follows the run until the agent answers.
 */
export function PlanCard({ threadId, started, task, run, agent, active }: { threadId: string; started: string; task: string; run: Message[]; agent?: Agent; active: boolean }) {
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

  const steps = useMemo(() => planSteps(run), [run]);
  const calls = useMemo(() => {
    const map: Record<string, ToolCall> = {};
    for (const m of run) for (const c of m.tool_calls ?? []) map[c.id] = c;
    return map;
  }, [run]);
  const tools = run.filter((m) => m.role === "tool" && m.tool_name !== "update_plan");
  // Every agent waiting on you, not just the first: parallel agents ask at the same time.
  const waitingAll = run.filter((m) => m.role === "tool" && m.status === "awaiting_approval" && approvals[m.id]);
  const waiting = waitingAll[0];
  const approval: ApprovalRequest | undefined = waiting ? approvals[waiting.id] : undefined;
  const lastReply = [...run].reverse().find((m) => m.role === "assistant");
  const failed = !active && lastReply?.status === "error";
  const stopped = !active && !failed && (lastReply?.status === "stopped" || run.some((m) => m.status === "stopped"));
  const done = steps.filter((s) => s.status === "done").length;
  const doing = steps.find((s) => s.status === "doing");
  const finished = !active && !failed && !stopped;
  const currentIndex = Math.min(steps.length, done + (doing ? 1 : 0));

  // Everyone who took part, with what they are doing: several agents work in parallel in a group.
  const askers = useMemo(() => new Set(Object.values(questions).map((q) => q.agent_id)), [questions]);
  const team = useMemo(() => {
    const ids: string[] = [];
    for (const m of run) if (m.agent_id && m.role !== "event" && !ids.includes(m.agent_id)) ids.push(m.agent_id);
    return ids
      .map((id) => agents[id])
      .filter(Boolean)
      .map((a) => ({ agent: a, state: workingIds.includes(a.id) ? (askers.has(a.id) || run.some((m) => m.agent_id === a.id && m.status === "awaiting_approval") ? "waiting" : "working") : "idle" }));
  }, [run, agents, workingIds, askers]);
  const many = team.length > 1;
  const busyNames = team.filter((x) => x.state !== "idle").map((x) => x.agent.name);

  // What the agents say as they go (their notes between tool calls): the latest one is shown live.
  const notes = useMemo(
    () =>
      run
        .filter((m) => m.role === "assistant" && m.status !== "error")
        .map((m) => ({ m, text: splitThink(m.content).text.trim() }))
        .filter((x) => x.text),
    [run],
  );
  const lastNote = active ? notes[notes.length - 1] : undefined;

  const startMs = new Date(started).getTime();
  const lastMs = run.length ? new Date(run[run.length - 1].created_at).getTime() : startMs;
  const elapsed = (active ? now : lastMs) - startMs;

  let status: string;
  if (waiting && approval) status = t("plan.needsApproval", { action: describe(waiting.tool_name ?? "tool", calls[waiting.tool_call_id ?? ""]?.arguments, t).title.toLowerCase() });
  else if (failed) status = t("plan.failed");
  else if (stopped) status = t("plan.stopped");
  else if (finished) status = t("plan.done", { n: steps.length || tools.length });
  else if (many && busyNames.length > 1) status = t("plan.agentsWorking", { names: busyNames.join(", ") });
  else if (doing) status = doing.step;
  else if (steps.length === 0) status = t("plan.planning");
  else status = t("plan.working");
  const title = task || t("plan.untitled");

  return (
    <div className="animate-pop w-full max-w-[540px] overflow-hidden rounded-[22px] border border-line bg-bg shadow-sm" data-plan-card>
      <div className="flex items-center gap-3 px-4 pt-3.5">
        <span className="relative flex size-8 shrink-0 items-center justify-center rounded-full bg-hover">
          {agent ? <AvatarView avatar={agent.avatar} name={agent.name} size={26} /> : <Spinner size={16} />}
          {(active || waiting) && <span className={cx("absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-bg", waiting ? "bg-warning" : "animate-pulse bg-accent")} />}
        </span>
        <span className="min-w-0 flex-1 truncate text-[14.5px] font-semibold" title={title}>{title}</span>
        <span className="shrink-0 font-mono text-[12.5px] text-muted tabular-nums">{clock(elapsed)}</span>
      </div>
      <div className="flex items-center justify-between gap-3 px-4 pt-2.5 text-[12.5px] text-muted">
        <span className={cx("truncate", waiting && "text-warning")}>{status}</span>
        <span className="shrink-0">{steps.length ? t("plan.step", { n: Math.max(1, currentIndex), total: steps.length }) : t("plan.planning")}</span>
      </div>
      <div className="flex gap-1.5 px-4 pt-2" role="progressbar" aria-valuemin={0} aria-valuemax={steps.length || 1} aria-valuenow={done}>
        {(steps.length ? steps : [{ step: "", status: active ? "doing" : "pending" } as Step]).map((s, i) => (
          <span
            key={i}
            title={s.step}
            className={cx("h-1.5 flex-1 rounded-full transition-colors", s.status === "done" || (finished && steps.length === 0) ? "bg-accent" : s.status === "doing" ? "animate-pulse bg-accent/55" : "bg-hover")}
          />
        ))}
      </div>

      {many && (
        <div className="flex flex-wrap gap-1.5 px-4 pt-3" aria-label={t("plan.team")}>
          {team.map(({ agent: a, state }) => (
            <span key={a.id} className={cx("inline-flex items-center gap-1.5 rounded-full border py-0.5 pr-2.5 pl-0.5 text-[12px]", state === "working" ? "border-accent/40 bg-accent-soft" : state === "waiting" ? "border-warning/50 bg-warning-soft" : "border-line text-muted")}>
              <AvatarView avatar={a.avatar} name={a.name} size={18} />
              {a.name}
              <span className={cx("size-1.5 rounded-full", state === "working" ? "animate-pulse bg-accent" : state === "waiting" ? "bg-warning" : "bg-faint")} />
            </span>
          ))}
        </div>
      )}
      {lastNote && (
        <p className="mx-4 mt-3 line-clamp-2 border-l-2 border-accent/40 pl-3 text-[13px] leading-snug text-muted">
          {many && lastNote.m.agent_id && agents[lastNote.m.agent_id] && <span className="font-medium text-ink">{agents[lastNote.m.agent_id].name}: </span>}
          {lastNote.text}
        </p>
      )}

      {steps.length > 0 ? (
        <ul className="space-y-0.5 px-4 pt-3 pb-1" aria-label={t("plan.checklist")}>
          {steps.map((st, i) => (
            <li key={i} className="flex items-start gap-3 py-1 text-[13.5px]">
              <span className="mt-0.5 shrink-0">
                {st.status === "done" ? (
                  <span className="flex size-4 items-center justify-center rounded-[5px] bg-accent text-white"><Check size={11} strokeWidth={3.5} /></span>
                ) : st.status === "doing" && (active || waiting) ? (
                  <Spinner size={15} />
                ) : (
                  <span className="block size-4 rounded-[5px] border border-line-strong" />
                )}
              </span>
              <span className={cx("min-w-0 flex-1", st.status === "done" ? "text-muted line-through decoration-faint" : st.status === "doing" ? "font-medium text-ink" : "text-ink/80")}>{st.step}</span>
            </li>
          ))}
        </ul>
      ) : (
        active && (
          <p className="flex items-center gap-3 px-4 pt-3 pb-1 text-[13.5px] text-muted">
            <Spinner size={14} /> {t("plan.planning")}
          </p>
        )
      )}
      {waitingAll.length > 0 && (
        <ul className="space-y-1 px-4 pt-2 pb-1">
          {waitingAll.map((w) => {
            const who = w.agent_id ? agents[w.agent_id] : undefined;
            const ap = approvals[w.id];
            return (
              <li key={w.id} className="flex items-center gap-2 rounded-xl border border-warning/40 bg-warning-soft px-2.5 py-1.5 text-[13px]">
                {who && <AvatarView avatar={who.avatar} name={who.name} size={18} />}
                <span className="min-w-0 flex-1 truncate">
                  {who && many && <span className="font-medium">{who.name}: </span>}
                  {t("plan.waiting", { action: describe(w.tool_name ?? "tool", calls[w.tool_call_id ?? ""]?.arguments, t).title.toLowerCase() })}
                </span>
                <button onClick={() => void api.resolveApproval(ap.id, true)} className="rounded-full bg-accent px-3 py-1 text-[12.5px] font-medium text-white hover:bg-accent-hover">{t("plan.approve")}</button>
                <button onClick={() => void api.resolveApproval(ap.id, false)} className="rounded-full border border-line px-3 py-1 text-[12.5px] hover:bg-hover">{t("plan.deny")}</button>
              </li>
            );
          })}
        </ul>
      )}

      {open && (
        <div className="mx-4 mt-1 mb-2 max-h-80 overflow-y-auto rounded-xl bg-hover/60 px-3 py-2.5 text-[12.5px]">
          <p className="mb-1.5 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("plan.activity")}</p>
          <ul className="space-y-1.5">
            {run
              .filter((m) => (m.role === "assistant" && m.status !== "error" && splitThink(m.content).text.trim()) || (m.role === "tool" && m.tool_name !== "update_plan") || m.role === "event")
              .map((m) => {
                const who = m.agent_id ? agents[m.agent_id] : undefined;
                const call = m.tool_call_id ? calls[m.tool_call_id] : undefined;
                const d = m.role === "tool" ? describe(m.tool_name ?? call?.name ?? "tool", call?.arguments, t) : null;
                return (
                  <li key={m.id} className="flex items-start gap-2">
                    <span className="mt-0.5 shrink-0">{who ? <AvatarView avatar={who.avatar} name={who.name} size={16} /> : <span className="block size-4" />}</span>
                    <span className="selectable min-w-0 flex-1 whitespace-pre-wrap break-words">
                      {d ? (
                        <>
                          <span className="text-ink">{d.title}</span>
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
      <div className="flex items-center gap-2.5 px-4 pt-2 pb-3.5">
        {active ? (
          <button onClick={() => void api.stopThread(threadId)} className="inline-flex items-center gap-1.5 rounded-full border border-line px-3.5 py-1.5 text-[13px] hover:bg-hover">
            <Square size={11} fill="currentColor" /> {t("chat.stop")}
          </button>
        ) : failed ? (
          <span className="inline-flex items-center gap-1.5 text-[13px] text-danger">
            <CircleAlert size={14} /> {t("plan.failedHint")}
          </span>
        ) : null}
        <button onClick={() => setOpen((v) => !v)} className="ml-auto inline-flex items-center gap-1 text-[13px] text-muted hover:text-ink">
          {t("plan.openRun")} <ChevronDown size={14} className={cx("transition-transform", open && "rotate-180")} />
        </button>
      </div>
    </div>
  );
}
