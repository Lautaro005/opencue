import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { BookmarkPlus, Brain, ChevronRight, FileText, FolderOpen, Globe, HelpCircle, Inbox, Mail, Plug, Search, Send, ShieldAlert, Sparkles, SquareTerminal, Trash2 } from "lucide-react";
import type { Agent, ApprovalRequest, Message, ToolCall } from "../../lib/types";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
import { dmThreadId, useStore } from "../../lib/store";
import { AvatarView } from "../AvatarView";
import { Spinner, Typing } from "../ui";
import { AskCard } from "./AskCard";

function describe(name: string, args: Record<string, unknown> | undefined, t: (k: string, o?: Record<string, unknown>) => string) {
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
  const name = message.tool_name ?? call?.name ?? "tool";
  const d = describe(name, call?.arguments, t);
  const status = message.status;
  const waiting = status === "awaiting_approval";

  // Talking to another agent from a 1:1 chat: a pill that opens their conversation.
  if (name === "send_to_agent" && !isGroup && agentId) {
    const wanted = String(call?.arguments?.agent ?? "").replace(/^@/, "").trim().toLowerCase();
    const target = Object.values(agents).find((a) => a.name.toLowerCase() === wanted);
    if (target) return <LiveChat fromId={agentId} target={target} running={status === "running"} />;
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

/** Talking to another agent from a 1:1 chat: a green, moving-border card that follows their conversation live. */
function LiveChat({ fromId, target, running }: { fromId: string; target: Agent; running: boolean }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const openDm = useStore((s) => s.openDm);
  const loadMessages = useStore((s) => s.loadMessages);
  const threadId = dmThreadId(fromId, target.id);
  const messages = useStore((s) => s.messages[threadId]) ?? NO_MESSAGES;
  const working = useStore((s) => s.working[threadId] ?? NO_WORKERS);
  const endRef = useRef<HTMLDivElement>(null);
  // Loading the thread is what makes its messages stream in here as they are written.
  useEffect(() => void loadMessages(threadId), [threadId, loadMessages]);
  const talk = messages.filter((m) => m.role === "assistant" && (m.content || m.status === "streaming"));
  useEffect(() => endRef.current?.scrollIntoView({ block: "end" }), [talk.length, talk[talk.length - 1]?.content]);
  const busy = running || working.length > 0;

  return (
    <div className="live-card animate-pop w-full max-w-[520px] overflow-hidden text-[12.5px]" data-busy={busy}>
      <button onClick={() => openDm(fromId, target.id)} className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-hover/50">
        {busy ? <Spinner size={14} /> : <AvatarView avatar={target.avatar} name={target.name} size={20} />}
        <span className="min-w-0 truncate font-medium text-ink">{t("chat.chatWith", { name: target.name })}</span>
        <ChevronRight size={14} className="ml-auto shrink-0 text-faint" />
      </button>
      {talk.length > 0 && (
        <div className="selectable max-h-64 space-y-2 overflow-y-auto border-t border-line/70 px-3 py-2.5">
          {talk.map((m) => {
            const who = m.agent_id ? agents[m.agent_id] : undefined;
            return (
              <div key={m.id} className="flex gap-2">
                {who && <AvatarView avatar={who.avatar} name={who.name} size={18} />}
                <div className="min-w-0 flex-1">
                  <div className="text-[11px] font-medium text-muted">{who?.name}</div>
                  <div className="whitespace-pre-wrap break-words text-[12.5px] leading-snug">{m.content || <Typing />}</div>
                </div>
              </div>
            );
          })}
          {busy && talk[talk.length - 1]?.status !== "streaming" && <Typing />}
          <div ref={endRef} />
        </div>
      )}
    </div>
  );
}
