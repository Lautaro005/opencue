import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, CircleHelp, Pencil, Send, ShieldAlert, X } from "lucide-react";
import { AvatarView } from "../AvatarView";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";

type Option = { title: string; description: string };

/**
 * What an agent shows when it is waiting for the user. A permission is a yellow, animated frame with
 * Accept / Reject. A question is a card: the question, a line under it, up to three answers (a title
 * and what choosing it means) and an "Other" field to write your own.
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
  options?: Option[];
  onAccept?: () => void;
  onReject?: () => void;
  onAnswer?: (text: string) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [other, setOther] = useState(options.length === 0);
  const submit = (v: string) => {
    const a = v.trim();
    if (a) onAnswer?.(a);
  };
  if (permission) {
    return (
      <div role="alertdialog" aria-label={title} className="ask-card animate-pop w-full max-w-[520px] px-4 py-3.5 text-[13.5px]">
        <div className="flex items-center gap-2 font-semibold text-ink">
          <ShieldAlert size={16} className="shrink-0 text-warning" />
          <span className="min-w-0 truncate">{title}</span>
        </div>
        <p className="selectable mt-1.5 whitespace-pre-wrap break-words leading-relaxed text-ink/90">{body}</p>
        <div className="mt-3 flex gap-2">
          <button onClick={onAccept} className="inline-flex items-center gap-1 rounded-lg btn-primary px-3.5 py-1.5 text-[13px] font-medium text-white">
            <Check size={14} /> {t("ask.accept")}
          </button>
          <button onClick={onReject} className="inline-flex items-center gap-1 rounded-lg border border-line bg-elevated px-3.5 py-1.5 text-[13px] font-medium hover:bg-hover">
            <X size={14} /> {t("ask.reject")}
          </button>
        </div>
      </div>
    );
  }
  return (
    <div role="alertdialog" aria-label={title} className="ask-card animate-pop w-full max-w-[560px] px-4 py-4 text-[13.5px]">
      <div className="flex items-start gap-2.5">
        <CircleHelp size={18} className="mt-0.5 shrink-0 text-warning" />
        <div className="min-w-0">
          <p className="selectable text-[15px] leading-snug font-semibold text-ink">{title}</p>
          {body && <p className="selectable mt-1 whitespace-pre-wrap break-words text-[13px] leading-relaxed text-muted">{body}</p>}
        </div>
      </div>
      <div className="mt-3.5 space-y-1.5">
        {options.slice(0, 3).map((o, i) => (
          <button key={o.title} onClick={() => submit(o.title)} className="group/opt flex w-full items-start gap-3 rounded-xl border border-line bg-elevated px-3.5 py-2.5 text-left transition-colors hover:border-warning/70 hover:bg-hover">
            <span className="mt-px flex size-5 shrink-0 items-center justify-center rounded-full bg-hover text-[11px] font-semibold text-muted group-hover/opt:bg-warning/20 group-hover/opt:text-ink">{i + 1}</span>
            <span className="min-w-0">
              <span className="block text-[13.5px] font-medium text-ink">{o.title}</span>
              {o.description && <span className="mt-0.5 block text-[12.5px] leading-snug text-muted">{o.description}</span>}
            </span>
          </button>
        ))}
        {other ? (
          <div className="flex items-end gap-2 pt-0.5">
            <textarea
              value={text}
              rows={1}
              autoFocus={options.length > 0}
              onChange={(e) => setText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  submit(text);
                }
              }}
              placeholder={t("ask.other")}
              className="min-h-9 flex-1 resize-none rounded-lg border border-line bg-elevated px-3 py-2 text-[13.5px] outline-none focus:border-warning"
            />
            <button onClick={() => submit(text)} disabled={!text.trim()} className="inline-flex h-9 items-center gap-1 rounded-lg btn-primary px-3 text-[13px] font-medium text-white disabled:opacity-40">
              <Send size={14} /> {t("ask.send")}
            </button>
          </div>
        ) : (
          <button onClick={() => setOther(true)} className="flex w-full items-center gap-3 rounded-xl border border-dashed border-line-strong px-3.5 py-2.5 text-left text-[13.5px] text-muted hover:bg-hover hover:text-ink">
            <Pencil size={14} className="shrink-0" /> {t("ask.other")}
          </button>
        )}
      </div>
    </div>
  );
}

/** The questions agents are waiting on in this thread. */
export function QuestionCards({ threadId, isGroup }: { threadId: string; isGroup: boolean }) {
  const { t } = useTranslation();
  const questions = useStore((s) => s.questions);
  const agents = useStore((s) => s.agents);
  const working = useStore((s) => s.working[threadId]);
  const members = useStore((s) => s.groups[threadId]?.members);
  // An agent working in this group may ask from its own chat id (a CLI agent's own tools do not
  // say where they are running): its card belongs here, where the work is.
  const mine = Object.values(questions).filter(
    (q) => q.thread_id === threadId || (isGroup && q.thread_id === q.agent_id && !!members?.some((m) => m.agent_id === q.agent_id) && !!working?.includes(q.agent_id)),
  );
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
