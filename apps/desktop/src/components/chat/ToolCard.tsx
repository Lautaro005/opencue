import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUpRight, BookmarkPlus, Brain, ChevronRight, FileText, FolderOpen, Globe, HelpCircle, Inbox, Mail, Plug, Search, Send, ShieldAlert, Sparkles, SquareTerminal, Trash2 } from "lucide-react";
import type { Agent, ApprovalRequest, Message, ToolCall } from "../../lib/types";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
import { dmThreadId, useStore } from "../../lib/store";
import { AvatarView } from "../AvatarView";
import { Spinner, Typing } from "../ui";
import { AskCard } from "./AskCard";
import { ArtifactProgress } from "./ArtifactProgress";

export function describe(name: string, args: Record<string, unknown> | undefined, t: (k: string, o?: Record<string, unknown>) => string) {
  const a = args ?? {};
  switch (name) {
    case "web_search":
      return { icon: <Search size={14} />, title: t("chat.tools.web_search"), detail: String(a.query ?? "") };
    case "web_fetch":
      return { icon: <Globe size={14} />, title: t("chat.tools.web_fetch"), detail: String(a.url ?? "") };
    case "remember":
      return { icon: <BookmarkPlus size={14} />, title: t("chat.tools.remember"), detail: String(a.text ?? "") };
    case "recall":
      return { icon: <Brain size={14} />, title: t("chat.tools.recall"), detail: String(a.query ?? "") };
    case "forget":
      return { icon: <Trash2 size={14} />, title: t("chat.tools.forget"), detail: `#${a.id ?? ""}` };
    case "send_to_agent":
      return { icon: <Send size={14} />, title: t("chat.tools.send_to_agent", { agent: a.agent ?? "" }), detail: String(a.message ?? "") };
    case "ask_user":
      return { icon: <HelpCircle size={14} />, title: t("chat.tools.ask_user"), detail: String(a.question ?? "") };
    case "use_skill":
      return { icon: <Sparkles size={14} />, title: t("chat.tools.use_skill"), detail: String(a.name ?? "") };
    case "save_skill":
      return { icon: <Sparkles size={14} />, title: t("chat.tools.save_skill"), detail: String(a.name ?? "") };
    case "run_command":
      return { icon: <SquareTerminal size={14} />, title: t("chat.tools.run_command"), detail: String(a.command ?? "") };
    case "read_file":
      return { icon: <FileText size={14} />, title: t("chat.tools.read_file"), detail: String(a.path ?? "") };
    case "write_file":
      return { icon: <FileText size={14} />, title: t("chat.tools.write_file"), detail: String(a.path ?? "") };
    case "list_files":
      return { icon: <FolderOpen size={14} />, title: t("chat.tools.list_files"), detail: String(a.path ?? "") };
    case "share_file":
      return { icon: <FolderOpen size={14} />, title: t("chat.tools.share_file"), detail: String(a.path ?? "") };
    case "read_emails":
      return { icon: <Inbox size={14} />, title: t("chat.tools.read_emails"), detail: "" };
    case "search_inbox":
      return { icon: <Search size={14} />, title: t("chat.tools.search_inbox"), detail: String(a.query ?? "") };
    case "create_artifact":
      return { icon: <FileText size={14} />, title: t("chat.tools.create_artifact"), detail: String(a.title ?? "") };
    case "read_artifact":
      return { icon: <FileText size={14} />, title: t("chat.tools.read_artifact"), detail: "" };
    case "update_plan":
      return { icon: <Sparkles size={14} />, title: t("chat.tools.update_plan"), detail: "" };
    case "send_email":
      return { icon: <Mail size={14} />, title: t("chat.tools.send_email", { to: a.to ?? "" }), detail: String(a.subject ?? "") };
    default: {
      const m = name.match(/^mcp__(.+?)__(.+)$/);
      return {
        icon: <Plug size={14} />,
        title: m ? t("chat.tools.mcp", { tool: m[2], server: m[1] }) : name,
        detail: Object.keys(a).length ? JSON.stringify(a) : "",
      };
    }
  }
}

export function ToolCard({
  message,
  call,
  approval,
  agentName,
  agentId,
  isGroup,
}: {
  message: Message;
  call?: ToolCall;
  approval?: ApprovalRequest;
  agentName: string;
  agentId?: string | null;
  isGroup?: boolean;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const agents = useStore((s) => s.agents);
  const threadMessages = useStore((s) => s.messages[message.thread_id]);
  const name = message.tool_name ?? call?.name ?? "tool";
  const d = describe(name, call?.arguments, t);
  const status = message.status;
  const waiting = status === "awaiting_approval";

  // An artifact being made is a card with a progress bar; the finished one is the card that follows.
  if (name === "create_artifact" && (status === "running" || status === "done")) {
    if (status === "done") return null;
    const a = call?.arguments ?? {};
    return <ArtifactProgress title={typeof a.title === "string" ? a.title : null} chars={typeof a.content === "string" ? a.content.length : undefined} canvas={a.canvas === true} kind={typeof a.type === "string" ? (a.type as never) : undefined} />;
  }

  // Talking to another agent from a 1:1 chat: a pill that opens their conversation.
  if (name === "send_to_agent" && !isGroup && agentId) {
    const wanted = String(call?.arguments?.agent ?? "").replace(/^@/, "").trim().toLowerCase();
    const target = Object.values(agents).find((a) => a.name.toLowerCase() === wanted);
    if (target) {
      // This exchange ends where the next one with the same agent starts, so each card keeps its own.
      const later = (threadMessages ?? []).filter((m) => m.role === "tool" && m.tool_name === "send_to_agent" && m.agent_id === agentId && m.created_at > message.created_at);
      const callArgs = (id: string | null) => (threadMessages ?? []).flatMap((m) => m.tool_calls ?? []).find((c) => c.id === id)?.arguments;
      const next = later.find((m) => String(callArgs(m.tool_call_id)?.agent ?? "").replace(/^@/, "").trim().toLowerCase() === wanted);
      return <LiveChat fromId={agentId} target={target} question={String(call?.arguments?.message ?? "")} since={message.created_at} until={next?.created_at} running={status === "running"} />;
    }
  }

  return (
    <>
    <div className="animate-pop max-w-[520px] rounded-xl border border-line bg-bg text-[12.5px]">
      <button onClick={() => setOpen((v) => !v)} className="flex w-full items-center gap-2 px-3 py-2 text-left">
        <span className={cx("shrink-0", status === "error" ? "text-danger" : "text-muted")}>
          {status === "running" ? <Spinner size={13} /> : waiting ? <ShieldAlert size={14} className="text-warning" /> : d.icon}
        </span>
        <span className="shrink-0 font-medium text-ink">{d.title}</span>
        {d.detail && <span className="min-w-0 truncate text-muted">{d.detail}</span>}
        <span className="ml-auto flex shrink-0 items-center gap-1.5 pl-2">
          {status !== "done" && status !== "running" && (
            <span className={cx("text-[11px]", status === "error" || status === "denied" ? "text-danger" : "text-muted")}>{t(`chat.status.${status}`, { defaultValue: status })}</span>
          )}
          <ChevronRight size={14} className={cx("text-faint transition-transform", open && "rotate-90")} />
        </span>
      </button>

      {open && (
        <div className="selectable space-y-2 border-t border-line px-3 py-2.5">
          {call && Object.keys(call.arguments ?? {}).length > 0 && (
            <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-all rounded-lg bg-hover px-2.5 py-2 font-mono text-[11.5px] text-muted">
              {JSON.stringify(call.arguments, null, 2)}
            </pre>
          )}
          {message.content && (
            <pre className="max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-hover px-2.5 py-2 font-mono text-[11.5px]">{message.content}</pre>
          )}
        </div>
      )}
    </div>
    {waiting && approval && (
      <AskCard
        permission
        title={t("ask.permissionTitle", { name: agentName })}
        body={approval.reason}
        onAccept={() => void api.resolveApproval(approval.id, true)}
        onReject={() => void api.resolveApproval(approval.id, false)}
      />
    )}
    </>
  );
}

const NO_MESSAGES: Message[] = [];
const NO_WORKERS: string[] = [];

/** One exchange with another agent, WhatsApp-style: their name in their own color, a dropdown with the
 * question and the answer, and a button that opens the whole conversation. */
function LiveChat({ fromId, target, question, since, until, running }: { fromId: string; target: Agent; question: string; since: string; until?: string; running: boolean }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const openDm = useStore((s) => s.openDm);
  const loadMessages = useStore((s) => s.loadMessages);
  const threadId = dmThreadId(fromId, target.id);
  const messages = useStore((s) => s.messages[threadId]) ?? NO_MESSAGES;
  const working = useStore((s) => s.working[threadId] ?? NO_WORKERS);
  const [open, setOpen] = useState(false);
  // Loading the thread is what makes its messages stream in here as they are written.
  useEffect(() => void loadMessages(threadId), [threadId, loadMessages]);
  const answers = messages.filter((m) => m.role === "assistant" && m.agent_id === target.id && (m.content || m.status === "streaming") && m.created_at >= since && (!until || m.created_at < until));
  const busy = running || (working.length > 0 && !until);
  const sender = agents[fromId];
  const color = target.avatar.color || "var(--success)";

  return (
    <div className="agent-card animate-pop my-1.5 w-full max-w-[520px] overflow-hidden text-[12.5px]" data-busy={busy} style={{ ["--agent-color" as string]: color }}>
      <div className="flex items-center">
        <button onClick={() => setOpen((v) => !v)} className="flex min-w-0 flex-1 items-center gap-2 px-3 py-2 text-left hover:bg-hover/50">
          {busy ? <Spinner size={14} /> : <AvatarView avatar={target.avatar} name={target.name} size={20} />}
          <span className="min-w-0 truncate font-medium text-ink">{target.name}</span>
          <ChevronRight size={14} className={cx("ml-auto shrink-0 text-faint transition-transform", open && "rotate-90")} />
        </button>
        <button onClick={() => openDm(fromId, target.id)} className="mr-1.5 rounded-lg p-1.5 text-muted hover:bg-hover hover:text-ink" aria-label={t("chat.openChat")} data-tip={t("chat.openChat")}>
          <ArrowUpRight size={15} />
        </button>
      </div>
      {open && (
        <div className="selectable space-y-2.5 border-t border-line/70 px-3 py-2.5">
          {question && (
            <div>
              <div className="text-[11px] font-medium text-muted">{sender?.name ?? t("chat.question")}</div>
              <div className="whitespace-pre-wrap break-words leading-snug">{question}</div>
            </div>
          )}
          <div>
            <div className="text-[11px] font-medium text-muted">{target.name}</div>
            {answers.length > 0 ? (
              answers.map((m) => (
                <div key={m.id} className="whitespace-pre-wrap break-words leading-snug">
                  {m.content || <Typing />}
                </div>
              ))
            ) : busy ? (
              <Typing />
            ) : (
              <div className="text-faint">…</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
