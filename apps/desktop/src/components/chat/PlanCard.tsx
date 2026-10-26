import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, CircleAlert, CircleCheck, Square } from "lucide-react";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
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
  for (const m of messages) {
    for (const c of m.tool_calls ?? []) {
      if (c.name !== "update_plan") continue;
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

function clock(ms: number) {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function seconds(ms: number) {
  return ms < 950 ? `${Math.max(0.1, ms / 1000).toFixed(1)}s` : `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)}s`;
}

/**
 * The card a `/plan` run lives in: what the agent is doing right now, how far along the plan is
 * (one segment per step), each thing it did and how long it took, and an Approve / Deny pair when
 * it is waiting on you. It follows the run until the agent answers.
 */
export function PlanCard({ threadId, started, run, agent, active }: { threadId: string; started: string; run: Message[]; agent?: Agent; active: boolean }) {
  const { t } = useTranslation();
  const approvals = useStore((s) => s.approvals);
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
  const waiting = run.find((m) => m.role === "tool" && m.status === "awaiting_approval");
  const approval: ApprovalRequest | undefined = waiting ? approvals[waiting.id] : undefined;
  const lastReply = [...run].reverse().find((m) => m.role === "assistant");
  const failed = !active && lastReply?.status === "error";
  const stopped = !active && !failed && (lastReply?.status === "stopped" || run.some((m) => m.status === "stopped"));
  const done = steps.filter((s) => s.status === "done").length;
  const doing = steps.find((s) => s.status === "doing");
  const finished = !active && !failed && !stopped;
  const currentIndex = Math.min(steps.length, done + (doing ? 1 : 0));

  const rows = tools.map((m, i) => {
    const call = m.tool_call_id ? calls[m.tool_call_id] : undefined;
    const d = describe(m.tool_name ?? call?.name ?? "tool", call?.arguments, t);
    const next = tools[i + 1] ?? run[run.indexOf(m) + 1];
    const end = next ? new Date(next.created_at).getTime() : active ? now : new Date(m.created_at).getTime();
    return { m, d, ms: Math.max(0, end - new Date(m.created_at).getTime()) };
  });
  const shown = open ? rows : rows.slice(-4);

  const startMs = new Date(started).getTime();
  const lastMs = run.length ? new Date(run[run.length - 1].created_at).getTime() : startMs;
  const elapsed = (active ? now : lastMs) - startMs;

  let title: string;
  if (waiting && approval) title = t("plan.needsApproval", { action: describe(waiting.tool_name ?? "tool", calls[waiting.tool_call_id ?? ""]?.arguments, t).title.toLowerCase() });
  else if (failed) title = t("plan.failed");
  else if (stopped) title = t("plan.stopped");
  else if (finished) title = t("plan.done", { n: steps.length || tools.length });
  else if (doing) title = doing.step;
  else if (steps.length === 0) title = t("plan.planning");
  else title = t("plan.working");

  return (
    <div className="animate-pop w-full max-w-[540px] overflow-hidden rounded-[22px] border border-line bg-bg shadow-sm" data-plan-card>
      <div className="flex items-center gap-3 px-4 pt-3.5">
        <span className="relative flex size-8 shrink-0 items-center justify-center rounded-full bg-hover">
          {agent ? <AvatarView avatar={agent.avatar} name={agent.name} size={26} /> : <Spinner size={16} />}
          {(active || waiting) && <span className={cx("absolute -top-0.5 -right-0.5 size-2.5 rounded-full ring-2 ring-bg", waiting ? "bg-warning" : "animate-pulse bg-accent")} />}
        </span>
        <span className="min-w-0 flex-1 truncate text-[14.5px] font-medium">{title}</span>
        <span className="shrink-0 font-mono text-[12.5px] text-muted tabular-nums">{clock(elapsed)}</span>
      </div>
      <div className="flex items-center justify-between px-4 pt-2.5 text-[12.5px] text-muted">
        <span className="truncate">{agent?.model ?? agent?.name ?? ""}</span>
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

      <ul className="space-y-0.5 px-4 pt-3 pb-1">
        {shown.map(({ m, d, ms }) => (
          <li key={m.id} className="flex items-center gap-3 py-1.5 text-[13.5px]">
            <span className={cx("shrink-0", m.status === "error" || m.status === "denied" ? "text-danger" : "text-muted")}>{m.status === "running" ? <Spinner size={14} /> : d.icon}</span>
            <span className="min-w-0 flex-1 truncate">
              <span className="text-ink">{d.title}</span>
              {d.detail && <span className="ml-1.5 text-muted">{d.detail}</span>}
            </span>
            <span className="shrink-0 text-[12.5px] text-muted tabular-nums">{m.status === "awaiting_approval" ? "0s" : seconds(ms)}</span>
          </li>
        ))}
        {rows.length === 0 && steps.length === 0 && active && (
          <li className="flex items-center gap-3 py-1.5 text-[13.5px] text-muted">
            <Spinner size={14} /> {t("plan.planning")}
          </li>
        )}
        {waiting && (
          <li className="flex items-center gap-3 py-1.5 text-[13.5px] text-muted">
            <span className="size-3.5 shrink-0 rounded-sm border border-warning" />
            <span className="flex-1 truncate">{t("plan.waiting", { action: describe(waiting.tool_name ?? "tool", calls[waiting.tool_call_id ?? ""]?.arguments, t).title.toLowerCase() })}</span>
          </li>
        )}
      </ul>

      {open && steps.length > 0 && (
        <ol className="mx-4 mt-1 mb-2 space-y-1 rounded-xl bg-hover/60 px-3 py-2.5 text-[13px]">
          {steps.map((s, i) => (
            <li key={i} className="flex items-start gap-2">
              <span className="mt-0.5 shrink-0">
                {s.status === "done" ? <CircleCheck size={14} className="text-success" /> : s.status === "doing" ? <Spinner size={13} /> : <span className="mt-0.5 block size-3 rounded-full border border-line-strong" />}
              </span>
              <span className={cx(s.status === "done" && "text-muted line-through decoration-faint")}>{s.step}</span>
            </li>
          ))}
        </ol>
      )}

      <div className="flex items-center gap-2.5 px-4 pt-2 pb-3.5">
        {waiting && approval ? (
          <>
            <button onClick={() => void api.resolveApproval(approval.id, true)} className="inline-flex items-center gap-1.5 rounded-full bg-accent px-5 py-2 text-[14px] font-medium text-white hover:bg-accent-hover">
              <Check size={16} /> {t("plan.approve")}
            </button>
            <button onClick={() => void api.resolveApproval(approval.id, false)} className="inline-flex items-center gap-1.5 rounded-full border border-line px-4 py-2 text-[14px] hover:bg-hover">
              ✕ {t("plan.deny")}
            </button>
          </>
        ) : active ? (
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
