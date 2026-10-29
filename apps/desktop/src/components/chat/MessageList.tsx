import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertCircle, Check, ChevronRight, Clock, Copy, MessageCircle, Pencil, RefreshCw, Reply, Sparkles, Zap } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Agent, Message, ToolCall } from "../../lib/types";
import { clockTime, cx, splitThink } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { IconButton, Spinner, Typing } from "../ui";
import { Markdown } from "./Markdown";
import { ToolCard } from "./ToolCard";
import { AttachmentList } from "./Attachments";
import { QuestionCards } from "./AskCard";
import { ArtifactProgress } from "./ArtifactProgress";
import { chatMessages, PlanCard } from "./PlanCard";

const EMPTY: Message[] = [];

function dayLabel(iso: string, lang: string) {
  const d = new Date(iso);
  const today = new Date();
  if (d.toDateString() === today.toDateString()) return null;
  return d.toLocaleDateString(lang, { weekday: "long", day: "numeric", month: "long" });
}

export function MessageList({ threadId, isGroup }: { threadId: string; isGroup: boolean }) {
  const { t, i18n } = useTranslation();
  const all = useStore((s) => s.messages[threadId] ?? EMPTY);
  // What the user wrote while the agents were busy waits at the bottom until they read it.
  const messages = useMemo(() => (all.some((m) => m.status === "queued") ? [...all.filter((m) => m.status !== "queued"), ...all.filter((m) => m.status === "queued")] : all), [all]);
  const agents = useStore((s) => s.agents);
  const approvals = useStore((s) => s.approvals);
  const toolProgress = useStore((s) => s.toolProgress);
  const workingNow = useStore((s) => (s.working[threadId]?.length ?? 0) > 0);
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

  // Coming from the search: scroll to the message and flash it.
  const focusMessage = useStore((s) => s.focusMessage);
  useEffect(() => {
    if (!focusMessage) return;
    const el = scroller.current?.querySelector<HTMLElement>(`[data-mid="${focusMessage}"]`);
    if (!el) return;
    stick.current = false;
    el.scrollIntoView({ block: "center" });
    el.classList.add("flash");
    window.setTimeout(() => el.classList.remove("flash"), 1800);
    useStore.setState({ focusMessage: null });
  }, [focusMessage, messages]);

  useLayoutEffect(() => {
    const el = scroller.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [messages, questionCount]);

  const onScroll = () => {
    const el = scroller.current;
    if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
  };

  // A /plan message owns what follows it up to the next message from the user: that is one card.
  // Of those, only the answers show in the chat as well (`shown`); the rest is in the card.
  const planRuns = useMemo(() => {
    const runs = new Map<string, Message[]>();
    const owner = new Map<string, string>();
    const shown = new Set<string>();
    let cur: Message[] | null = null;
    let curId = "";
    for (const m of messages) {
      if (m.role === "user" && m.status === "queued") continue;
      if (m.role === "user") {
        cur = /^\/plan(\s|$)/i.test(m.content.trimStart()) ? [] : null;
        curId = m.id;
        if (cur) runs.set(m.id, cur);
      } else if (cur) {
        cur.push(m);
        owner.set(m.id, curId);
      }
    }
    for (const run of runs.values()) for (const id of chatMessages(run)) shown.add(id);
    return { runs, owner, shown };
  }, [messages]);

  const lastUserId = useMemo(() => [...messages].reverse().find((m) => m.role === "user" && m.status !== "queued")?.id, [messages]);

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
              <span data-mid={m.id} className="block h-0" />
              {divider && <div className="my-3 text-center text-[11.5px] font-medium text-faint">{divider}</div>}
              {m.role === "user" && <UserBubble m={m} firstOfRun={firstOfRun} isLast={m.id === lastUserId} />}
              {m.role === "user" && planRuns.runs.has(m.id) && (
                <div className="mt-1 mb-1">
                  <PlanCard
                    threadId={threadId}
                    started={m.created_at}
                    task={m.content.trimStart().replace(/^\/plan\s*/i, "").split("\n")[0].slice(0, 120)}
                    run={planRuns.runs.get(m.id)!}
                    active={workingNow && m.id === lastUserId}
                  />
                </div>
              )}
              {m.role === "event" && <EventCard m={m} />}
              {m.role === "assistant" && (!planRuns.owner.has(m.id) || planRuns.shown.has(m.id)) && (
                <AgentRow agent={agent} showHeader={firstOfRun || (isGroup && planRuns.owner.has(m.id))} isGroup={isGroup}>
                  {m.status === "error" ? (
                    <div className="max-w-full rounded-2xl rounded-tl-md border border-danger/30 bg-danger-soft px-3.5 py-2.5 text-[13.5px]">
                      <div className="flex items-center gap-1.5 font-medium text-danger">
                        <AlertCircle size={15} /> {t("chat.errorTitle")}
                      </div>
                      <p className="selectable mt-1 whitespace-pre-wrap text-ink/90">{m.content}</p>
                      {/settings|ajustes|api key|provider/i.test(m.content) && (
                        <button onClick={() => openSettings("providers")} className="mt-2 text-[12.5px] font-medium text-accent hover:underline">
                          {t("chat.openSettings")}
                        </button>
                      )}
                    </div>
                  ) : m.content ? (
                    <AssistantText m={m} />
                  ) : m.status === "streaming" && !toolProgress[m.id] ? (
                    <div className="inline-flex h-10 items-center rounded-[20px] bg-agent px-4 text-muted">
                      <Typing />
                    </div>
                  ) : null}
                  {m.status === "streaming" && toolProgress[m.id] && <ArtifactProgress title={toolProgress[m.id].title} chars={toolProgress[m.id].chars} />}
                  {m.attachments?.length ? <AttachmentList items={m.attachments} /> : null}
                </AgentRow>
              )}
              {m.role === "tool" && !planRuns.owner.has(m.id) && (
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
  const split = splitThink(m.content);
  // What an agent says while it still has tools to call is its train of thought, not the answer.
  const narrating = !!m.tool_calls?.length;
  const think = [split.think, narrating ? split.text : ""].filter(Boolean).join("\n\n");
  const text = narrating ? "" : split.text;
  const thinking = split.thinking && m.status === "streaming";
  return (
    <div className="group/msg flex min-w-0 max-w-full flex-col items-start gap-0.5">
      {think && (
        <div className="max-w-full">
          <button onClick={() => setOpen((v) => !v)} className="inline-flex items-center gap-1 text-[12px] text-faint transition-colors hover:text-muted">
            {thinking ? <Spinner size={11} /> : null}
            <span>{thinking ? t("chat.thinking") : t("chat.thought")}</span>
            <ChevronRight size={12} className={cx("transition-transform", open && "rotate-90")} />
          </button>
          {open && <div className="selectable mt-1 max-h-72 overflow-y-auto whitespace-pre-wrap border-l-2 border-line pl-3 text-[12.5px] leading-relaxed text-muted">{think}</div>}
        </div>
      )}
      {text && (
        <div className="max-w-full rounded-[20px] bg-agent px-4 py-2.5 text-[14px] leading-relaxed text-agent-ink">
          <Markdown text={text} />
          {m.status === "stopped" && <div className="mt-1 text-[11.5px] text-muted">— {t("chat.stopped")}</div>}
        </div>
      )}
      {!text && !think && m.status === "streaming" && (
        <div className="inline-flex h-10 items-center rounded-[20px] bg-agent px-4 text-muted">
          <Typing />
        </div>
      )}
      {text && m.status !== "streaming" && (
        <div className="ml-1 flex h-6 items-center opacity-0 transition-opacity group-hover/msg:opacity-100 focus-within:opacity-100">
          <ReplyButton m={m} text={text} />
          <CopyButton text={text} />
        </div>
      )}
    </div>
  );
}

/** Quotes this reply in the box below, so the next message answers it. */
function ReplyButton({ m, text }: { m: Message; text: string }) {
  const { t } = useTranslation();
  const setReplyTo = useStore((s) => s.setReplyTo);
  const name = useStore((s) => (m.agent_id ? s.agents[m.agent_id]?.name : undefined)) ?? "";
  return (
    <IconButton
      label={t("chat.reply")}
      onClick={() => {
        setReplyTo(m.thread_id, { agentId: m.agent_id ?? null, name, text: text.replace(/\s+/g, " ").trim().slice(0, 280) });
        window.dispatchEvent(new CustomEvent("opencue:focus-composer"));
      }}
      className="size-6"
    >
      <Reply size={13} />
    </IconButton>
  );
}

/** Copies a reply to the clipboard and says so for a moment. */
function CopyButton({ text }: { text: string }) {
  const { t } = useTranslation();
  const [done, setDone] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      // Clipboard blocked: fall back to a hidden selection.
      const ta = document.createElement("textarea");
      ta.value = text;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      ta.remove();
    }
    setDone(true);
    window.setTimeout(() => setDone(false), 1500);
  };
  return (
    <IconButton label={done ? t("chat.copied") : t("chat.copyReply")} onClick={() => void copy()} className="size-6">
      {done ? <Check size={13} className="text-success" /> : <Copy size={13} />}
    </IconButton>
  );
}

/** A message to the agents; a leading "> quote" (from Reply) shows as a small quote above the text. */
function UserText({ content, queued }: { content: string; queued: boolean }) {
  const lines = content.split("\n");
  let i = 0;
  while (i < lines.length && lines[i].startsWith(">")) i++;
  const quote = lines.slice(0, i).map((l) => l.replace(/^>\s?/, "")).join(" ").trim();
  const body = quote ? lines.slice(i).join("\n").trim() : content;
  return (
    <div className={cx("bubble-me selectable break-words rounded-[20px] px-4 py-2.5 text-[14px] leading-relaxed text-me-ink", queued && "opacity-70")}>
      {quote && <div className="mb-1.5 line-clamp-3 border-l-2 border-me-ink/40 pl-2.5 text-[12.5px] opacity-80">{quote.replace(/\*\*/g, "")}</div>}
      <div className="whitespace-pre-wrap">{body}</div>
    </div>
  );
}

function UserBubble({ m, firstOfRun, isLast }: { m: Message; firstOfRun: boolean; isLast: boolean }) {
  const { t } = useTranslation();
  const editLast = useStore((s) => s.editLast);
  const regenerate = useStore((s) => s.regenerate);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(m.content);
  const save = () => {
    const text = draft.trim();
    setEditing(false);
    if (text && text !== m.content) void editLast(m.thread_id, text);
    else if (text) void regenerate(m.thread_id);
  };
  return (
    <div className={cx("flex justify-end", firstOfRun && "mt-3")}>
      <div className={cx("group flex flex-col items-end gap-1", editing ? "w-[75%]" : "max-w-[75%]")}>
        {m.attachments?.length ? <AttachmentList items={m.attachments} align="end" /> : null}
        {editing ? (
          <div className="w-full rounded-[20px] border border-accent bg-bg p-2">
            <textarea
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  save();
                }
                if (e.key === "Escape") {
                  e.stopPropagation();
                  setEditing(false);
                }
              }}
              rows={Math.min(8, Math.max(2, draft.split("\n").length))}
              className="w-full resize-none bg-transparent px-2 py-1 text-[14px] leading-relaxed outline-none"
            />
            <div className="flex justify-end gap-1.5 px-1 pb-0.5">
              <button onClick={() => setEditing(false)} className="rounded-lg px-2.5 py-1 text-[12.5px] text-muted hover:bg-hover">
                {t("common.cancel")}
              </button>
              <button onClick={save} disabled={!draft.trim()} className="rounded-lg btn-primary px-2.5 py-1 text-[12.5px] font-medium text-white disabled:opacity-40">
                {t("chat.saveEdit")}
              </button>
            </div>
          </div>
        ) : (
          m.content && <UserText content={m.content} queued={m.status === "queued"} />
        )}
        {m.status === "queued" && (
          <div className="flex items-center gap-2 text-[11.5px] text-muted">
            <Clock size={12} /> {t("chat.queued")}
            <button onClick={() => void api.unqueueMessage(m.thread_id, m.id).catch(() => {})} className="rounded-md px-1.5 py-0.5 text-[11.5px] font-medium text-accent hover:bg-hover">
              {t("chat.unqueue")}
            </button>
          </div>
        )}
        <div className="mt-0.5 flex h-6 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          <span className="mr-1 text-[11px] text-faint">{clockTime(m.created_at)}</span>
          {isLast && !editing && (
            <>
              <IconButton
                label={t("chat.edit")}
                onClick={() => {
                  setDraft(m.content);
                  setEditing(true);
                }}
                className="size-6"
              >
                <Pencil size={13} />
              </IconButton>
              <IconButton label={t("chat.regenerate")} onClick={() => void regenerate(m.thread_id)} className="size-6">
                <RefreshCw size={13} />
              </IconButton>
            </>
          )}
        </div>
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
  // A WhatsApp message the agent answered: a green card that unfolds into the question and the answer.
  const wa = m.content.match(/^WhatsApp · (.+?)\n\nQ: ([\s\S]*?)\n\nA: ([\s\S]*)$/);
  if (wa) {
    return (
      <div className="live-card animate-pop my-1.5 w-full max-w-[520px] overflow-hidden text-[12.5px]" data-busy="false">
        <button onClick={() => setOpen((v) => !v)} className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-hover/50">
          <MessageCircle size={15} className="shrink-0 text-success" />
          <span className="font-medium text-ink">WhatsApp</span>
          <span className="min-w-0 truncate text-muted">{wa[1]}</span>
          <ChevronRight size={14} className={cx("ml-auto shrink-0 text-faint transition-transform", open && "rotate-90")} />
        </button>
        {open && (
          <div className="selectable space-y-2.5 border-t border-line/70 px-3 py-2.5">
            <div>
              <div className="text-[11px] font-medium text-muted">{wa[1]}</div>
              <div className="whitespace-pre-wrap break-words leading-snug">{wa[2]}</div>
            </div>
            <div>
              <div className="text-[11px] font-medium text-muted">{t("chat.whatsappReply")}</div>
              <div className="whitespace-pre-wrap break-words leading-snug">{wa[3]}</div>
            </div>
          </div>
        )}
      </div>
    );
  }
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
