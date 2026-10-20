import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Send, ShieldAlert, X } from "lucide-react";
import { AvatarView } from "../AvatarView";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";

/**
 * What an agent shows when it is waiting for the user: a yellow, animated frame with a title, the
 * request underneath, and either Accept / Reject or a field to answer in (plus any options).
 */
export function AskCard({
  title,
  body,
  permission,
  options = [],
  onAccept,
  onReject,
  onAnswer,
}: {
  title: string;
  body: string;
  permission: boolean;
  options?: string[];
  onAccept?: () => void;
  onReject?: () => void;
  onAnswer?: (text: string) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const submit = (v: string) => {
    const a = v.trim();
    if (a) onAnswer?.(a);
  };
  return (
    <div role="alertdialog" aria-label={title} className="ask-card animate-pop w-full max-w-[520px] px-4 py-3.5 text-[13.5px]">
      <div className="flex items-center gap-2 font-semibold text-ink">
        <ShieldAlert size={16} className="shrink-0 text-warning" />
        <span className="min-w-0 truncate">{title}</span>
      </div>
      <p className="selectable mt-1.5 whitespace-pre-wrap break-words leading-relaxed text-ink/90">{body}</p>
      {permission ? (
        <div className="mt-3 flex gap-2">
          <button onClick={onAccept} className="inline-flex items-center gap-1 rounded-lg bg-accent px-3.5 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
            <Check size={14} /> {t("ask.accept")}
          </button>
          <button onClick={onReject} className="inline-flex items-center gap-1 rounded-lg border border-line bg-elevated px-3.5 py-1.5 text-[13px] font-medium hover:bg-hover">
            <X size={14} /> {t("ask.reject")}
          </button>
        </div>
      ) : (
        <div className="mt-3 space-y-2">
          {options.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {options.map((o) => (
                <button key={o} onClick={() => submit(o)} className="rounded-full border border-warning/60 bg-elevated px-3 py-1 text-[12.5px] font-medium hover:bg-hover">
                  {o}
                </button>
              ))}
            </div>
          )}
          <div className="flex items-end gap-2">
            <textarea
              value={text}
              rows={1}
              autoFocus
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  submit(text);
                }
              }}
              placeholder={t("ask.placeholder")}
              className="min-h-9 flex-1 resize-none rounded-lg border border-line bg-elevated px-3 py-2 text-[13.5px] outline-none focus:border-warning"
            />
            <button
              onClick={() => submit(text)}
              disabled={!text.trim()}
              className="inline-flex h-9 items-center gap-1 rounded-lg bg-accent px-3 text-[13px] font-medium text-white hover:bg-accent-hover disabled:opacity-40"
            >
              <Send size={14} /> {t("ask.send")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

/** The questions agents are waiting on in this thread. */
export function QuestionCards({ threadId, isGroup }: { threadId: string; isGroup: boolean }) {
  const { t } = useTranslation();
  const questions = useStore((s) => s.questions);
  const agents = useStore((s) => s.agents);
  const mine = Object.values(questions).filter((q) => q.thread_id === threadId);
  if (mine.length === 0) return null;
  return (
    <div className="mt-3 flex flex-col gap-3">
      {mine.map((q) => {
        const name = agents[q.agent_id]?.name ?? "";
        const permission = q.kind === "permission";
        const asker = agents[q.agent_id];
        return (
          // Lined up with the agent's replies: under their avatar column in groups, not starting at it.
          <div key={q.id} className="flex items-start gap-2.5">
            {isGroup && <div className="w-8 shrink-0">{asker && <AvatarView avatar={asker.avatar} name={asker.name} size={30} />}</div>}
            <div className="min-w-0 max-w-[85%] flex-1">
              <AskCard
                permission={permission}
                title={q.title || (permission ? t("ask.permissionTitle", { name }) : t("ask.questionTitle", { name }))}
                body={q.body}
                options={q.options}
                onAccept={() => void api.answerQuestion(q.id, "approved")}
                onReject={() => void api.answerQuestion(q.id, "rejected")}
                onAnswer={(text) => void api.answerQuestion(q.id, text)}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
