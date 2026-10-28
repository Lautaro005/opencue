import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, CircleHelp, Send, ShieldAlert, X } from "lucide-react";
import { api } from "../lib/api";
import { useStore } from "../lib/store";
import type { Agent, ApprovalRequest, Question } from "../lib/types";
import { AvatarView } from "./AvatarView";

declare global {
  interface Window {
    __NOTCH__?: boolean;
  }
}

type Ask = { kind: "approval"; a: ApprovalRequest } | { kind: "question"; q: Question };

/**
 * The page of the notch window (Settings → Notifications → Where → Notch). A black pill at the top
 * centre of the screen, grown out of the camera housing on a Mac that has one:
 * - an agent needs you (a permission, a question): the pill opens and you answer there;
 * - agents are working: a thin pill with their faces hopping, one after another;
 * - an agent finished: a short "done" before it goes away.
 * It stays hidden while the app itself is in front, because the chat shows all this already.
 */
export function NotchApp() {
  const { t } = useTranslation();
  const ready = useStore((s) => s.ready);
  const init = useStore((s) => s.init);
  const settings = useStore((s) => s.settings);
  const agents = useStore((s) => s.agents);
  const working = useStore((s) => s.working);
  const questions = useStore((s) => s.questions);
  const approvals = useStore((s) => s.approvals);
  const [notch, setNotch] = useState({ macos: false, notch_height: 0 });
  const [inView, setInView] = useState(false);
  const [done, setDone] = useState<Agent | null>(null);
  const pill = useRef<HTMLDivElement>(null);
  const prevBusy = useRef<Set<string>>(new Set());

  useEffect(() => {
    // The window sits over the desktop: no page background of its own.
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
    document.documentElement.classList.add("dark");
    void init();
    void api.notchInfo().then(setNotch).catch(() => {});
  }, [init]);

  // The settings and the agents live in the main window: look again now and then.
  useEffect(() => {
    const timer = window.setInterval(() => {
      void api.getSettings().then((s) => useStore.setState({ settings: s })).catch(() => {});
      void useStore.getState().refresh().catch(() => {});
      void api.mainInView().then(setInView).catch(() => {});
    }, 1500);
    return () => window.clearInterval(timer);
  }, []);

  const on = (key: string) => settings[key] !== "false";
  const enabled = settings.notify_channel === "notch" && on("notify_enabled");
  const quiet = on("notify_background_only") && inView;

  const asks: Ask[] = useMemo(
    () => (on("notify_approvals") ? [...Object.values(approvals).map((a) => ({ kind: "approval", a }) as Ask), ...Object.values(questions).map((q) => ({ kind: "question", q }) as Ask)] : []),
    [approvals, questions, settings], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const busyIds = useMemo(() => [...new Set(Object.values(working).flat())], [working]);
  const workers = busyIds.map((id) => agents[id]).filter(Boolean);

  // An agent that stopped working is shown as finished for a moment.
  useEffect(() => {
    const now = new Set(busyIds);
    for (const id of prevBusy.current) {
      if (!now.has(id) && agents[id] && on("notify_replies")) {
        setDone(agents[id]);
        const timer = window.setTimeout(() => setDone(null), 2600);
        prevBusy.current = now;
        return () => window.clearTimeout(timer);
      }
    }
    prevBusy.current = now;
  }, [busyIds]); // eslint-disable-line react-hooks/exhaustive-deps

  const mode: "ask" | "work" | "done" | "none" = !enabled || quiet ? "none" : asks.length ? "ask" : workers.length && on("notify_working") ? "work" : done ? "done" : "none";

  // The window follows the size of the pill.
  useEffect(() => {
    const el = pill.current;
    if (mode === "none" || !el) {
      void api.notchHide();
      return;
    }
    const send = () => void api.notchPlace(el.offsetWidth, el.offsetHeight);
    send();
    const ro = new ResizeObserver(send);
    ro.observe(el);
    return () => ro.disconnect();
  }, [mode, asks.length, workers.length]);

  if (!ready || mode === "none") return null;
  const strip = Math.max(notch.notch_height, 0);
  const ask = asks[0];
  return (
    <div className="flex w-full justify-center">
      <div ref={pill} className="notch-in inline-block overflow-hidden bg-black text-white" style={{ borderRadius: "0 0 26px 26px", minWidth: mode === "ask" ? 420 : undefined }}>
        {mode === "ask" && ask && <AskPanel ask={ask} strip={strip} agents={agents} count={asks.length} />}
        {mode === "work" && <WorkPill workers={workers} strip={strip} />}
        {mode === "done" && done && (
          <div className="flex items-center gap-2 px-5 text-[12.5px]" style={{ height: Math.max(strip, 34) }}>
            <AvatarView avatar={done.avatar} name={done.name} size={18} />
            <span className="truncate font-medium">{done.name}</span>
            <Check size={14} className="text-[#4ade80]" strokeWidth={3} />
            <span className="text-white/60">{t("notch.finished")}</span>
          </div>
        )}
      </div>
    </div>
  );
}

/** Faces that hop one after another: quiet, and a little fun. */
function WorkPill({ workers, strip }: { workers: Agent[]; strip: number }) {
  const { t } = useTranslation();
  const shown = workers.slice(0, 4);
  // Around the hardware notch the faces go on its left and the dots on its right.
  const gap = strip > 0 ? 200 : 0;
  return (
    <div className="flex items-center justify-between gap-4 px-5" style={{ height: Math.max(strip, 34), minWidth: gap + 150 }}>
      <span className="flex items-center gap-1">
        {shown.map((a, i) => (
          <span key={a.id} className="notch-hop flex rounded-full" style={{ animationDelay: `${i * 130}ms` }}>
            <AvatarView avatar={a.avatar} name={a.name} size={20} />
          </span>
        ))}
        {workers.length > shown.length && <span className="pl-2.5 text-[11px] text-white/60">+{workers.length - shown.length}</span>}
      </span>
      {strip > 0 && <span style={{ width: gap }} aria-hidden />}
      <span className="flex items-center gap-2 text-[11.5px] text-white/65">
        {strip === 0 && <span className="max-w-[160px] truncate">{workers.length === 1 ? workers[0].name : t("notch.working", { count: workers.length })}</span>}
        <span className="flex gap-1" aria-hidden>
          {[0, 1, 2].map((i) => (
            <span key={i} className="notch-dot size-1.5 rounded-full bg-white/70" style={{ animationDelay: `${i * 160}ms` }} />
          ))}
        </span>
      </span>
    </div>
  );
}

/** A permission or a question, answered right here. */
function AskPanel({ ask, strip, agents, count }: { ask: Ask; strip: number; agents: Record<string, Agent>; count: number }) {
  const { t } = useTranslation();
  const [text, setText] = useState("");
  const agent = agents[ask.kind === "approval" ? ask.a.agent_id : ask.q.agent_id];
  const permission = ask.kind === "approval" || ask.q.kind === "permission";
  const id = ask.kind === "approval" ? ask.a.id : ask.q.id;
  const body = ask.kind === "approval" ? ask.a.reason : ask.q.kind === "permission" ? ask.q.body : ask.q.title || ask.q.body;
  const sub = ask.kind === "question" && ask.q.kind !== "permission" && ask.q.title ? ask.q.body : "";
  const options = ask.kind === "question" && ask.q.kind !== "permission" ? ask.q.options.slice(0, 3) : [];
  const answer = (a: string) => void api.answerQuestion(id, a);
  const decide = (yes: boolean) => (ask.kind === "approval" ? void api.resolveApproval(id, yes) : answer(yes ? "approved" : "rejected"));
  return (
    <div style={{ paddingTop: strip }}>
      <div className="px-5 pt-3 pb-4">
        <div className="flex items-center gap-2.5">
          {agent && <AvatarView avatar={agent.avatar} name={agent.name} size={26} />}
          <span className="min-w-0 flex-1 truncate text-[13px] font-semibold">{agent ? (permission ? t("ask.permissionTitle", { name: agent.name }) : t("ask.questionTitle", { name: agent.name })) : ""}</span>
          {count > 1 && <span className="rounded-full bg-white/15 px-2 py-0.5 text-[11px]">+{count - 1}</span>}
          {permission ? <ShieldAlert size={16} className="text-[#fbbf24]" /> : <CircleHelp size={16} className="text-[#fbbf24]" />}
        </div>
        <p className="selectable mt-2 line-clamp-5 text-[13px] leading-snug whitespace-pre-wrap break-words text-white/90">{body}</p>
        {sub && <p className="selectable mt-1 line-clamp-3 text-[12px] leading-snug text-white/55">{sub}</p>}
        {permission ? (
          <div className="mt-3 flex gap-2">
            <button onClick={() => decide(true)} className="inline-flex items-center gap-1 rounded-full bg-white px-4 py-1.5 text-[12.5px] font-semibold text-black hover:bg-white/90">
              <Check size={14} strokeWidth={3} /> {t("ask.accept")}
            </button>
            <button onClick={() => decide(false)} className="inline-flex items-center gap-1 rounded-full bg-white/15 px-4 py-1.5 text-[12.5px] font-medium hover:bg-white/25">
              <X size={14} /> {t("ask.reject")}
            </button>
          </div>
        ) : (
          <div className="mt-3 space-y-1.5">
            {options.map((o) => (
              <button key={o.title} onClick={() => answer(o.title)} className="block w-full rounded-xl bg-white/10 px-3 py-2 text-left hover:bg-white/20">
                <span className="block text-[12.5px] font-medium">{o.title}</span>
                {o.description && <span className="block text-[11.5px] leading-snug text-white/55">{o.description}</span>}
              </button>
            ))}
            <div className="flex items-center gap-2">
              <input
                value={text}
                onChange={(e) => setText(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && text.trim() && answer(text.trim())}
                placeholder={t("ask.other")}
                className="min-w-0 flex-1 rounded-full bg-white/10 px-3.5 py-1.5 text-[12.5px] text-white outline-none placeholder:text-white/40 focus:bg-white/15"
              />
              <button onClick={() => text.trim() && answer(text.trim())} disabled={!text.trim()} className="flex size-7 items-center justify-center rounded-full bg-white text-black disabled:opacity-30" aria-label={t("ask.send")}>
                <Send size={13} />
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
