import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, AtSign, FileText, Loader2, ChevronRight, Mic, Paperclip, Plus, Reply, Sparkles, Square, X } from "lucide-react";
import type { Agent, CliSkill, SkillSummary } from "../../lib/types";
import { api } from "../../lib/api";
import { cx, formatBytes } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { IconButton, useClickOutside } from "../ui";
import { useStore } from "../../lib/store";
import { systemSkills, SkillIcon } from "../../lib/skills";
import { cancelRecording, markVoiceThread, startRecording, stopRecording } from "../../lib/voice";

export function Composer({
  threadId,
  name,
  members,
  mentionable,
  busy,
  onSend,
  onStop,
}: {
  threadId: string;
  name: string;
  members: Agent[] | null;
  /** Agents that can be @mentioned in a 1:1 chat (the agent asks them for help). */
  mentionable?: Agent[];
  busy: boolean;
  onSend: (text: string, files?: File[]) => void;
  onStop: () => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const [plus, setPlus] = useState(false);
  const [sub, setSub] = useState<"agents" | "skills" | null>(null);
  const [mention, setMention] = useState<{ query: string; index: number } | null>(null);
  const [slashIndex, setSlashIndex] = useState(0);
  const dashboardAgent = useStore((s) => s.settings.dashboard_agent);
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  // Skills the installed coding CLIs already have (Claude Code, Codex…), listed apart.
  const [cliSkills, setCliSkills] = useState<CliSkill[]>([]);
  const ref = useRef<HTMLTextAreaElement>(null);
  const plusRef = useRef<HTMLDivElement>(null);
  useClickOutside(plusRef, () => { setPlus(false); setSub(null); }, plus);
  const [voice, setVoice] = useState<"idle" | "recording" | "transcribing">("idle");
  const [files, setFiles] = useState<File[]>([]);
  const [dragging, setDragging] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);

  useEffect(() => () => cancelRecording(), [threadId]);
  useEffect(() => void api.listSkills().then(setSkills).catch(() => {}), [threadId]);
  useEffect(() => void api.listCliSkills().then(setCliSkills).catch(() => {}), [threadId]);
  useEffect(() => setFiles([]), [threadId]);

  const addFiles = (list: FileList | File[] | null) => {
    if (!list) return;
    const picked = Array.from(list);
    const tooBig = picked.find((f) => f.size > 25 * 1024 * 1024);
    if (tooBig) useStore.getState().showToast(t("chat.fileTooBig", { name: tooBig.name }));
    setFiles((cur) => [...cur, ...picked.filter((f) => f.size <= 25 * 1024 * 1024)].slice(0, 10));
    ref.current?.focus();
  };

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

  // Something (Settings → Dashboard) asked for a starting text.
  const prefill = useStore((s) => s.prefill);
  useEffect(() => {
    if (prefill === null) return;
    setText(prefill);
    useStore.getState().setPrefill(null);
    requestAnimationFrame(() => {
      const el = ref.current;
      el?.focus();
      el?.setSelectionRange(prefill.length, prefill.length);
    });
  }, [prefill]);

  // "/" at the start of the message opens the command list: built-ins first, then skills.
  const slashMatch = text.match(/^\/([\w-]*)$/);
  const system = systemSkills({ dashboard: dashboardAgent === threadId, group: !!members });
  const commands: { name: string; description: string; kind: "builtin" | "skill" | "cli"; icon?: string; source?: string }[] = [
    ...system.map((k) => ({ name: k.name, description: t(k.desc), kind: "builtin" as const, icon: k.icon })),
    ...skills.map((k) => ({ name: k.name, description: k.description, kind: "skill" as const })),
    // A name OpenCue already has wins over the CLI's.
    ...cliSkills.filter((k) => !skills.some((o) => o.name === k.name) && !system.some((o) => o.name === k.name)).map((k) => ({ name: k.name, description: k.description, kind: "cli" as const, source: k.source })),
  ];
  const slashQuery = slashMatch?.[1].toLowerCase() ?? "";
  const slashItems = slashMatch
    ? [...commands.filter((c) => c.name.toLowerCase().startsWith(slashQuery)), ...commands.filter((c) => !c.name.toLowerCase().startsWith(slashQuery) && c.name.toLowerCase().includes(slashQuery))]
    : [];
  const slashOpen = slashItems.length > 0;
  const pickCommand = (name: string) => {
    setText(`/${name} `);
    setSlashIndex(0);
    requestAnimationFrame(() => {
      const el = ref.current;
      el?.focus();
      el?.setSelectionRange(name.length + 2, name.length + 2);
    });
  };

  // A skill goes at the start of the message, which is where "/" commands are read.
  const insertSkill = (name: string) => {
    const rest = text.replace(/^\/[\w-]*\s*/, "");
    setText(`/${name} ${rest}`);
    requestAnimationFrame(() => ref.current?.focus());
  };

  const pool = members ?? mentionable ?? null;
  const candidates = pool && mention ? pool.filter((m) => m.name.toLowerCase().includes(mention.query.toLowerCase())) : [];

  const reply = useStore((s) => s.replyTo[threadId]);
  const setReplyTo = useStore((s) => s.setReplyTo);
  useEffect(() => {
    const focus = () => ref.current?.focus();
    window.addEventListener("opencue:focus-composer", focus);
    return () => window.removeEventListener("opencue:focus-composer", focus);
  }, []);

  const submit = () => {
    let v = text.trim();
    if (!v && files.length === 0) return;
    if (reply && v) {
      // The quote travels with the message; in a group the agent being answered is addressed.
      const mention = members && reply.name && !v.toLowerCase().includes(`@${reply.name.toLowerCase()}`) ? `@${reply.name} ` : "";
      v = `> **${reply.name || "…"}**: ${reply.text}\n\n${mention}${v}`;
    }
    if (reply) setReplyTo(threadId, null);
    onSend(v, files);
    setText("");
    setFiles([]);
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
    if (!pool) return;
    const m = v.slice(0, caret).match(/(?:^|\s)@([\w\-]*)$/);
    setMention(m ? { query: m[1], index: 0 } : null);
  };

  return (
    <div
      className="px-4 pb-4"
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes("Files")) {
          e.preventDefault();
          setDragging(true);
        }
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        addFiles(e.dataTransfer.files);
      }}
    >
      <input ref={fileInput} type="file" multiple hidden onChange={(e) => { addFiles(e.target.files); e.target.value = ""; }} />
      <div className="relative">
        {slashOpen && (
          <div className="animate-pop absolute bottom-full left-0 z-20 mb-2 max-h-72 w-[min(440px,100%)] overflow-y-auto rounded-xl border border-line bg-elevated p-1 shadow-pop">
            {slashItems.map((c, i) => (
              <button
                key={c.name}
                onMouseDown={(e) => {
                  e.preventDefault();
                  pickCommand(c.name);
                }}
                onMouseEnter={() => setSlashIndex(i)}
                className={cx("flex w-full items-start gap-2.5 rounded-lg px-2.5 py-1.5 text-left", i === slashIndex ? "bg-hover" : "")}
              >
                <span className="mt-0.5 text-muted">{c.kind === "builtin" ? <SkillIcon name={c.icon ?? ""} /> : <Sparkles size={14} />}</span>
                <span className="min-w-0">
                  <span className="flex items-center gap-1.5 text-[13px] font-medium">
                    /{c.name}
                    {c.source && <span className="rounded-full bg-hover px-1.5 py-px text-[10px] font-normal text-muted">{t(`chat.cliSource.${c.source}`)}</span>}
                  </span>
                  <span className="line-clamp-1 block text-[11.5px] text-muted">{c.description}</span>
                </span>
              </button>
            ))}
          </div>
        )}
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

        <div
          className={cx(
            "rounded-[22px] border bg-bg py-1.5 pr-1.5 pl-1.5 shadow-[0_1px_2px_rgb(0_0_0/0.04)] focus-within:border-line-strong",
            dragging ? "border-accent ring-2 ring-accent/20" : "border-line",
          )}
        >
          {files.length > 0 && (
            <div className="flex flex-wrap gap-1.5 px-1.5 pt-1 pb-1.5">
              {files.map((f, i) => (
                <span key={i} className="flex max-w-[220px] items-center gap-1.5 rounded-lg bg-hover py-1 pr-1 pl-2 text-[12px]">
                  <FileText size={13} className="shrink-0 text-muted" />
                  <span className="truncate">{f.name}</span>
                  <span className="shrink-0 text-faint">{formatBytes(f.size)}</span>
                  <button aria-label={t("common.remove")} onClick={() => setFiles((cur) => cur.filter((_, j) => j !== i))} className="rounded p-0.5 text-muted hover:bg-active hover:text-ink">
                    <X size={12} />
                  </button>
                </span>
              ))}
            </div>
          )}
        <div className="flex items-end gap-1.5">
          <div className="relative" ref={plusRef}>
            <button
              aria-label={t("chat.add")}
              data-tip={t("chat.add")}
              onClick={() => setPlus((v) => !v)}
              className="flex size-8 items-center justify-center rounded-full text-muted hover:bg-hover hover:text-ink"
            >
              <Plus size={19} />
            </button>
            {plus && (
              <div className="animate-pop absolute bottom-10 left-0 z-20 w-52 rounded-xl border border-line bg-elevated p-1 shadow-pop">
                <button
                  onMouseEnter={() => setSub(null)}
                  onClick={() => {
                    setPlus(false);
                    fileInput.current?.click();
                  }}
                  className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover"
                >
                  <Paperclip size={14} className="text-muted" /> {t("chat.files")}
                </button>
                {([
                  { key: "agents" as const, label: t("chat.agents"), icon: <AtSign size={14} className="text-muted" />, count: pool?.length ?? 0 },
                  { key: "skills" as const, label: t("chat.skills"), icon: <Sparkles size={14} className="text-muted" />, count: skills.length + system.length + cliSkills.length },
                ]).filter((g) => g.count > 0).map((g) => (
                  <div key={g.key} className="relative" onMouseEnter={() => setSub(g.key)}>
                    <button onClick={() => setSub(sub === g.key ? null : g.key)} className={cx("flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover", sub === g.key && "bg-hover")}>
                      {g.icon} {g.label}
                      <ChevronRight size={14} className="ml-auto text-faint" />
                    </button>
                    {sub === g.key && (
                      <div className="animate-pop absolute bottom-0 left-full z-30 ml-1 max-h-80 w-64 overflow-y-auto rounded-xl border border-line bg-elevated p-1 shadow-pop">
                        {g.key === "agents"
                          ? pool?.map((a) => (
                              <button
                                key={a.id}
                                onClick={() => {
                                  setPlus(false);
                                  insertMention(a);
                                }}
                                className="flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover"
                              >
                                <AvatarView avatar={a.avatar} name={a.name} size={20} />
                                <span className="truncate">{a.name}</span>
                              </button>
                            ))
                          : (
                              <>
                                <p className="px-2.5 pt-1 pb-0.5 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("chat.skillsSystem")}</p>
                                {system.map((k) => (
                                  <button
                                    key={k.name}
                                    onClick={() => {
                                      setPlus(false);
                                      insertSkill(k.name);
                                    }}
                                    className="flex w-full items-start gap-2 rounded-lg px-2.5 py-1.5 text-left hover:bg-hover"
                                  >
                                    <span className="mt-0.5 text-muted">
                                      <SkillIcon name={k.icon} />
                                    </span>
                                    <span className="min-w-0">
                                      <span className="block text-[13px] font-medium">/{k.name}</span>
                                      <span className="line-clamp-1 block text-[11.5px] text-muted">{t(k.desc)}</span>
                                    </span>
                                  </button>
                                ))}
                                <p className="mt-1 border-t border-line px-2.5 pt-2 pb-0.5 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("chat.skillsYours")}</p>
                                {skills.length === 0 && <p className="px-2.5 py-1.5 text-[11.5px] text-faint">{t("chat.skillsNone")}</p>}
                                {skills.map((k) => (
                                  <button
                                    key={k.name}
                                    onClick={() => {
                                      setPlus(false);
                                      insertSkill(k.name);
                                    }}
                                    className="flex w-full flex-col rounded-lg px-2.5 py-1.5 text-left hover:bg-hover"
                                  >
                                    <span className="text-[13px] font-medium">/{k.name}</span>
                                    <span className="line-clamp-1 text-[11.5px] text-muted">{k.description}</span>
                                  </button>
                                ))}
                                {Object.entries(
                                  cliSkills
                                    .filter((k) => !skills.some((o) => o.name === k.name))
                                    .reduce<Record<string, CliSkill[]>>((acc, k) => ((acc[k.source] ??= []).push(k), acc), {}),
                                ).map(([source, list]) => (
                                  <div key={source}>
                                    <p className="mt-1 border-t border-line px-2.5 pt-2 pb-0.5 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t(`chat.cliSource.${source}`)}</p>
                                    {list.map((k) => (
                                      <button
                                        key={k.name}
                                        onClick={() => {
                                          setPlus(false);
                                          insertSkill(k.name);
                                        }}
                                        className="flex w-full flex-col rounded-lg px-2.5 py-1.5 text-left hover:bg-hover"
                                      >
                                        <span className="text-[13px] font-medium">/{k.name}</span>
                                        <span className="line-clamp-1 text-[11.5px] text-muted">{k.description}</span>
                                      </button>
                                    ))}
                                  </div>
                                ))}
                              </>
                            )}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>

          {reply && (
            <div className="mx-1 mb-1.5 flex items-start gap-2 rounded-xl bg-hover px-3 py-2 text-[12.5px]">
              <Reply size={14} className="mt-0.5 shrink-0 text-muted" />
              <span className="min-w-0 flex-1">
                <span className="block font-medium">{t("chat.replyingTo", { name: reply.name || t("chat.reply") })}</span>
                <span className="line-clamp-2 block text-muted">{reply.text}</span>
              </span>
              <IconButton label={t("common.cancel")} onClick={() => setReplyTo(threadId, null)} className="size-6">
                <X size={13} />
              </IconButton>
            </div>
          )}
          <textarea
            ref={ref}
            rows={1}
            value={text}
            placeholder={members ? t("chat.placeholderGroup", { name }) : t("chat.placeholder", { name })}
            onChange={(e) => {
              setSlashIndex(0);
              onChange(e.target.value, e.target.selectionStart);
            }}
            onPaste={(e) => {
              if (e.clipboardData.files.length) {
                e.preventDefault();
                addFiles(e.clipboardData.files);
              }
            }}
            onKeyDown={(e) => {
              if (slashOpen) {
                if (e.key === "ArrowDown" || e.key === "ArrowUp") {
                  e.preventDefault();
                  const d = e.key === "ArrowDown" ? 1 : -1;
                  setSlashIndex((slashIndex + d + slashItems.length) % slashItems.length);
                  return;
                }
                if (e.key === "Enter" || e.key === "Tab") {
                  e.preventDefault();
                  pickCommand(slashItems[Math.min(slashIndex, slashItems.length - 1)].name);
                  return;
                }
                if (e.key === "Escape") {
                  setText("");
                  return;
                }
              }
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
            data-tip={voice === "recording" ? t("chat.voiceStop") : t("chat.voice")}
            onClick={() => void toggleVoice()}
            className={cx(
              "flex size-8 items-center justify-center rounded-full",
              voice === "recording" ? "animate-pulse bg-danger text-white" : "text-muted hover:bg-hover hover:text-ink",
            )}
          >
            {voice === "transcribing" ? <Loader2 size={17} className="animate-spin" /> : <Mic size={18} />}
          </button>
          {busy && !text.trim() && files.length === 0 ? (
            <button
              aria-label={t("chat.stop")}
              data-tip={t("chat.stop")}
              onClick={onStop}
              className="flex size-8 items-center justify-center rounded-full bg-ink text-bg hover:opacity-85"
            >
              <Square size={12} fill="currentColor" />
            </button>
          ) : (
            <button
              aria-label={busy ? t("chat.queue") : t("chat.send")}
              data-tip={busy ? t("chat.queue") : t("chat.send")}
              onClick={submit}
              disabled={!text.trim() && files.length === 0}
              className="flex size-8 items-center justify-center rounded-full bg-accent text-white transition-opacity hover:bg-accent-hover disabled:opacity-30"
            >
              <ArrowUp size={18} strokeWidth={2.4} />
            </button>
          )}
        </div>
        </div>
      </div>
    </div>
  );
}
