import { useState } from "react";
import { useTranslation } from "react-i18next";
import { BookmarkPlus, Brain, Check, ChevronRight, FileText, FolderOpen, Globe, Inbox, Mail, Plug, Search, Send, ShieldAlert, Sparkles, SquareTerminal, Trash2, X, Zap } from "lucide-react";
import type { ApprovalRequest, Message, ToolCall } from "../../lib/types";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
import { Spinner } from "../ui";

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
    case "composio_search":
      return { icon: <Zap size={14} />, title: t("chat.tools.composio_search"), detail: String(a.query ?? "") };
    case "composio_execute":
      return { icon: <Zap size={14} />, title: t("chat.tools.composio_execute"), detail: String(a.slug ?? "") };
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

export function ToolCard({ message, call, approval, agentName }: { message: Message; call?: ToolCall; approval?: ApprovalRequest; agentName: string }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const name = message.tool_name ?? call?.name ?? "tool";
  const d = describe(name, call?.arguments, t);
  const status = message.status;
  const waiting = status === "awaiting_approval";

  return (
    <div className={cx("animate-pop max-w-[520px] rounded-xl border text-[12.5px]", waiting ? "border-warning/50 bg-warning-soft" : "border-line bg-bg")}>
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

      {waiting && approval && (
        <div className="border-t border-warning/30 px-3 py-2.5">
          <p className="text-[12.5px] text-ink">
            <span className="font-medium">{t("chat.needsApproval", { name: agentName })}</span> — <span className="text-muted">{approval.reason}</span>
          </p>
          <div className="mt-2 flex gap-2">
            <button
              onClick={() => void api.resolveApproval(approval.id, true)}
              className="inline-flex items-center gap-1 rounded-lg bg-accent px-3 py-1 text-[12.5px] font-medium text-white hover:bg-accent-hover"
            >
              <Check size={13} /> {t("chat.approve")}
            </button>
            <button
              onClick={() => void api.resolveApproval(approval.id, false)}
              className="inline-flex items-center gap-1 rounded-lg border border-line bg-elevated px-3 py-1 text-[12.5px] font-medium hover:bg-hover"
            >
              <X size={13} /> {t("chat.deny")}
            </button>
          </div>
        </div>
      )}

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
  );
}
