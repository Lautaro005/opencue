import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, AtSign, Loader2, Mic, Paperclip, Plus, Square } from "lucide-react";
import type { Agent } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { useClickOutside } from "../ui";
import { useStore } from "../../lib/store";
import { cancelRecording, markVoiceThread, startRecording, stopRecording } from "../../lib/voice";

export function Composer({
  threadId,
  name,
  members,
  busy,
  onSend,
  onStop,
}: {
  threadId: string;
  name: string;
  members: Agent[] | null;
  busy: boolean;
  onSend: (text: string) => void;
  onStop: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [plus, setPlus] = useState(false);
  const [mention, setMention] = useState<{ query: string; index: number } | null>(null);
  const ref = useRef<HTMLTextAreaElement>(null);
  const plusRef = useRef<HTMLDivElement>(null);
  useClickOutside(plusRef, () => setPlus(false), plus);
  const [voice, setVoice] = useState<"idle" | "recording" | "transcribing">("idle");

  useEffect(() => () => cancelRecording(), [threadId]);

  const toggleVoice = async () => {
    const toast = useStore.getState().showToast;
    if (voice === "recording") {
      setVoice("transcribing");
      try {
        const said = (await stopRecording()).trim();
        if (said) {
          markVoiceThread(threadId);
          onSend(said);
        }
      } catch (e) {
        toast(String(e));
      } finally {
        setVoice("idle");
      }
      return;
    }
    if (voice !== "idle") return;
    try {
      await startRecording();
      setVoice("recording");
    } catch (e) {
      toast(t("chat.micError", { error: String(e) }));
    }
  };

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "0px";
    el.style.height = Math.min(el.scrollHeight, 220) + "px";
  }, [text]);

  useEffect(() => {
    ref.current?.focus();
  }, [name]);

  const candidates = members && mention ? members.filter((m) => m.name.toLowerCase().includes(mention.query.toLowerCase())) : [];

  const submit = () => {
    const v = text.trim();
    if (!v) return;
    onSend(v);
    setText("");
    setMention(null);
  };

  const insertMention = (agent: Agent) => {
    const el = ref.current;
    const caret = el?.selectionStart ?? text.length;
    const before = text.slice(0, caret).replace(/@[\w\- ]*$/, "");
    const after = text.slice(caret);
    const next = `${before}@${agent.name} ${after}`;
    setText(next);
    setMention(null);
    requestAnimationFrame(() => {
      el?.focus();
      const pos = before.length + agent.name.length + 2;
      el?.setSelectionRange(pos, pos);
    });
  };

  const onChange = (v: string, caret: number) => {
    setText(v);
    if (!members) return;
    const m = v.slice(0, caret).match(/(?:^|\s)@([\w\-]*)$/);
    setMention(m ? { query: m[1], index: 0 } : null);
  };

  return (
    <div className="px-6 pb-5">
      <div className="relative mx-auto max-w-[820px]">
        {mention && candidates.length > 0 && (
          <div className="animate-pop absolute bottom-full left-10 z-20 mb-2 w-60 rounded-xl border border-line bg-elevated p-1 shadow-pop">
            {candidates.map((a, i) => (
              <button
                key={a.id}
                onMouseDown={(e) => {
                  e.preventDefault();
                  insertMention(a);
                }}
                className={cx("flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px]", i === mention.index ? "bg-hover" : "hover:bg-hover")}
              >
                <AvatarView avatar={a.avatar} name={a.name} size={22} /> {a.name}
              </button>
            ))}
          </div>
        )}

        <div className="flex items-end gap-1.5 rounded-[22px] border border-line bg-bg py-1.5 pr-1.5 pl-1.5 shadow-[0_1px_2px_rgb(0_0_0/0.04)] focus-within:border-line-strong">
          <div className="relative" ref={plusRef}>
            <button
              aria-label={t("chat.add")}
              title={t("chat.add")}
              onClick={() => setPlus((v) => !v)}
              className="flex size-8 items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink"
            >
              <Plus size={19} />
            </button>
            {plus && (
              <div className="animate-pop absolute bottom-10 left-0 z-20 w-60 rounded-xl border border-line bg-elevated p-1 shadow-pop">
                {members?.map((a) => (
                  <button
                    key={a.id}
                    onClick={() => {
                      setPlus(false);
                      insertMention(a);
                    }}
                    className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover"
                  >
                    <AtSign size={14} className="text-muted" /> {a.name}
                  </button>
                ))}
                <div className="flex w-full cursor-not-allowed items-center gap-2 rounded-lg px-2.5 py-1.5 text-[13px] text-faint">
                  <Paperclip size={14} /> Files
                  <span className="ml-auto text-[11px]">{t("common.nextPhase")}</span>
                </div>
              </div>
            )}
          </div>

          <textarea
            ref={ref}
            rows={1}
            value={text}
            placeholder={members ? t("chat.placeholderGroup", { name }) : t("chat.placeholder", { name })}
            onChange={(e) => onChange(e.target.value, e.target.selectionStart)}
            onKeyDown={(e) => {
              if (mention && candidates.length > 0) {
                if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                  e.preventDefault();
                  const d = e.key === "ArrowDown" ? 1 : -1;
                  setMention({ ...mention, index: (mention.index + d + candidates.length) % candidates.length });
                  return;
                }
                if (e.key === "Enter" || e.key === "Tab") {
                  e.preventDefault();
                  insertMention(candidates[mention.index]);
                  return;
                }
                if (e.key === "Escape") {
                  setMention(null);
                  return;
                }
              }
              if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                e.preventDefault();
                submit();
              }
            }}
            className="max-h-[220px] min-h-8 flex-1 resize-none bg-transparent py-1.5 text-[14px] leading-[1.45] text-ink placeholder:text-faint outline-none focus-visible:outline-none"
          />

          <button
            aria-label={voice === "recording" ? t("chat.voiceStop") : t("chat.voice")}
            title={voice === "recording" ? t("chat.voiceStop") : t("chat.voice")}
            onClick={() => void toggleVoice()}
            className={cx(
              "flex size-8 items-center justify-center rounded-full",
              voice === "recording" ? "animate-pulse bg-danger text-white" : "text-muted hover:bg-hover hover:text-ink",
            )}
          >
            {voice === "transcribing" ? <Loader2 size={17} className="animate-spin" /> : <Mic size={18} />}
          </button>
          {busy && !text.trim() ? (
            <button
              aria-label={t("chat.stop")}
              title={t("chat.stop")}
              onClick={onStop}
              className="flex size-8 items-center justify-center rounded-full bg-ink text-bg hover:opacity-85"
            >
              <Square size={12} fill="currentColor" />
            </button>
          ) : (
            <button
              aria-label={t("chat.send")}
              title={t("chat.send")}
              onClick={submit}
              disabled={!text.trim()}
              className="flex size-8 items-center justify-center rounded-full bg-accent text-white transition-opacity hover:bg-accent-hover disabled:opacity-30"
            >
              <ArrowUp size={18} strokeWidth={2.4} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
