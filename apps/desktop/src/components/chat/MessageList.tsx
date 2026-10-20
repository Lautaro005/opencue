import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertCircle, Brain, ChevronRight, Sparkles, Zap } from "lucide-react";
import { useStore } from "../../lib/store";
import type { Agent, Message, ToolCall } from "../../lib/types";
import { clockTime, cx, splitThink } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { Spinner, Typing } from "../ui";
import { Markdown } from "./Markdown";
import { ToolCard } from "./ToolCard";
import { AttachmentList } from "./Attachments";
import { QuestionCards } from "./AskCard";

const EMPTY: Message[] = [];

function dayLabel(iso: string, lang: string) {
  const d = new Date(iso);
  const today = new Date();
  if (d.toDateString() === today.toDateString()) return null;
  return d.toLocaleDateString(lang, { weekday: "long", day: "numeric", month: "long" });
}

export function MessageList({ threadId, isGroup }: { threadId: string; isGroup: boolean }) {
  const { t, i18n } = useTranslation();
  const messages = useStore((s) => s.messages[threadId] ?? EMPTY);
  const agents = useStore((s) => s.agents);
  const approvals = useStore((s) => s.approvals);
  const questionCount = useStore((s) => Object.keys(s.questions).length);
  const openSettings = useStore((s) => s.openSettings);
  const scroller = useRef<HTMLDivElement>(null);
  const stick = useRef(true);

  // Index tool calls so tool result cards can show their arguments.
  const calls = useMemo(() => {
    const map: Record<string, ToolCall> = {};
    for (const m of messages) for (const c of m.tool_calls ?? []) map[c.id] = c;
    return map;
  }, [messages]);

  useEffect(() => {
    stick.current = true;
  }, [threadId]);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [messages, questionCount]);

  const onScroll = () => {
    const el = scroller.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  };

  let lastDay = "";
  let prevAuthor: string | null = null;

  return (
    <div ref={scroller} onScroll={onScroll} className="flex-1 overflow-y-auto">
      <div className="flex flex-col gap-1 px-4 py-6">
        {messages.map((m) => {
          const day = new Date(m.created_at).toDateString();
          const divider = day !== lastDay ? dayLabel(m.created_at, i18n.language) : null;
          lastDay = day;
          // Assistant turns that only carry tool calls render nothing themselves.
          const invisible = m.role === "assistant" && !m.content && !m.attachments?.length && m.status === "done";
          const author = m.role === "user" ? "user" : m.role === "event" ? "event" : m.agent_id;
          const firstOfRun = !invisible && author !== prevAuthor;
          if (!invisible) prevAuthor = author;
          const agent = m.agent_id ? agents[m.agent_id] : undefined;

          return (
            <Fragment key={m.id}>
              {divider && <div className="my-3 text-center text-[11.5px] font-medium text-faint">{divider}</div>}
              {m.role === "user" && <UserBubble m={m} firstOfRun={firstOfRun} />}
              {m.role === "event" && <EventCard m={m} />}
              {m.role === "assistant" && (
                <AgentRow agent={agent} showHeader={firstOfRun} isGroup={isGroup}>
                  {m.status === "error" ? (
                    <div className="max-w-full rounded-2xl rounded-tl-md border border-danger/30 bg-danger-soft px-3.5 py-2.5 text-[13.5px]">
                      <div className="flex items-center gap-1.5 font-medium text-danger">
                        <AlertCircle size={15} /> {t("chat.errorTitle")}
                      </div>
                      <p className="selectable mt-1 whitespace-pre-wrap text-ink/90">{m.content}</p>
                      {/settings|ajustes|api key|provider/i.test(m.content) && (
                        <button onClick={() => openSettings("models")} className="mt-2 text-[12.5px] font-medium text-accent hover:underline">
                          {t("chat.openSettings")}
                        </button>
                      )}
                    </div>
                  ) : m.content ? (
                    <AssistantText m={m} />
                  ) : m.status === "streaming" ? (
                    <div className="inline-flex h-10 items-center rounded-[20px] bg-agent px-4 text-muted">
                      <Typing />
                    </div>
                  ) : null}
                  {m.attachments?.length ? <AttachmentList items={m.attachments} /> : null}
                </AgentRow>
              )}
              {m.role === "tool" && (
                <AgentRow agent={agent} showHeader={firstOfRun} isGroup={isGroup}>
                  <ToolCard message={m} call={m.tool_call_id ? calls[m.tool_call_id] : undefined} approval={approvals[m.id]} agentName={agent?.name ?? ""} agentId={m.agent_id} isGroup={isGroup} />
                </AgentRow>
              )}
            </Fragment>
          );
        })}
        <QuestionCards threadId={threadId} isGroup={isGroup} />
      </div>
    </div>
  );
}

/** An agent's reply; its reasoning, if any, is a folded message above the answer. */
function AssistantText({ m }: { m: Message }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const { think, text, thinking } = splitThink(m.content);
  return (
    <>
      {think && (
        <div className="max-w-full">
          <button onClick={() => setOpen((v) => !v)} className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-[12.5px] text-muted hover:bg-hover">
            {thinking && m.status === "streaming" ? <Spinner size={12} /> : <Brain size={13} />}
            <span>{thinking && m.status === "streaming" ? t("chat.thinking") : t("chat.thought")}</span>
            <ChevronRight size={13} className={cx("text-faint transition-transform", open && "rotate-90")} />
          </button>
          {open && <div className="selectable mt-1 ml-1 max-h-72 overflow-y-auto whitespace-pre-wrap rounded-xl border border-line bg-event px-3 py-2 text-[12.5px] leading-relaxed text-muted">{think}</div>}
        </div>
      )}
      {text && (
        <div className="max-w-full rounded-[20px] bg-agent px-4 py-2.5 text-[14px] leading-relaxed text-agent-ink">
          <Markdown text={text} />
          {m.status === "stopped" && <div className="mt-1 text-[11.5px] text-muted">— {t("chat.stopped")}</div>}
        </div>
      )}
      {!text && m.status === "streaming" && !thinking && (
        <div className="inline-flex h-10 items-center rounded-[20px] bg-agent px-4 text-muted">
          <Typing />
        </div>
      )}
    </>
  );
}

function UserBubble({ m, firstOfRun }: { m: Message; firstOfRun: boolean }) {
  return (
    <div className={cx("flex justify-end", firstOfRun && "mt-3")}>
      <div className="group flex max-w-[75%] flex-col items-end gap-1">
        {m.attachments?.length ? <AttachmentList items={m.attachments} align="end" /> : null}
        {m.content && <div className="selectable whitespace-pre-wrap break-words rounded-[20px] bg-me px-4 py-2.5 text-[14px] leading-relaxed text-me-ink">{m.content}</div>}
        <span className="mt-0.5 h-4 text-[11px] text-faint opacity-0 transition-opacity group-hover:opacity-100">{clockTime(m.created_at)}</span>
      </div>
    </div>
  );
}

function AgentRow({ agent, showHeader, isGroup, children }: { agent?: Agent; showHeader: boolean; isGroup: boolean; children: React.ReactNode }) {
  if (!children) return null;
  return (
    <div className={cx("flex items-start gap-2.5", showHeader && "mt-3")}>
      {isGroup && <div className="w-8 shrink-0">{showHeader && agent && <AvatarView avatar={agent.avatar} name={agent.name} size={30} />}</div>}
      <div className="flex min-w-0 max-w-[85%] flex-col items-start gap-1">
        {showHeader && isGroup && agent && <span className="ml-1 text-[12px] font-medium text-muted">{agent.name}</span>}
        {children}
      </div>
    </div>
  );
}

function EventCard({ m }: { m: Message }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const isMail = /received an email/i.test(m.content);
  const auto = m.content.match(/^Automation “(.+?)”:/);
  const firstLine = m.content.split("\n")[0];
  const title = isMail ? t("chat.receivedEmail") : auto ? t("chat.automationRan", { name: auto[1] }) : firstLine;
  const hasMore = m.content.trim() !== title.trim();
  return (
    <div className="my-1.5">
      <button
        onClick={() => hasMore && setOpen((v) => !v)}
        className={cx("inline-flex max-w-[80%] items-center gap-2 rounded-lg px-2 py-1 text-[13px] text-muted", hasMore && "hover:bg-hover")}
      >
        {isMail ? <span className="text-[15px] leading-none">✉️</span> : auto ? <Zap size={14} className="text-warning" /> : <Sparkles size={14} />}
        <span className="truncate">{title}</span>
        {hasMore && <ChevronRight size={14} className={cx("shrink-0 text-faint transition-transform", open && "rotate-90")} />}
      </button>
      {open && <pre className="selectable mt-1 ml-2 max-w-[80%] rounded-xl border border-line bg-event px-3 py-2 font-sans text-[12.5px] whitespace-pre-wrap text-muted">{m.content}</pre>}
    </div>
  );
}
