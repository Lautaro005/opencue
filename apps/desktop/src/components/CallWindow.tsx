import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Mic, MicOff, PhoneOff, Volume2, VolumeX } from "lucide-react";
import { useStore } from "../lib/store";
import { Call, type CallState } from "../lib/call";
import { cx } from "../lib/format";
import { AvatarView } from "./AvatarView";

interface Line {
  who: "me" | "agent";
  text: string;
}

function clock(seconds: number) {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** A phone-sized floating window to talk to an agent as if on a call. */
export function CallWindow() {
  const call = useStore((s) => s.call);
  if (!call) return null;
  return <CallView key={call.threadId} agentId={call.agentId} threadId={call.threadId} />;
}

function CallView({ agentId, threadId }: { agentId: string; threadId: string }) {
  const { t } = useTranslation();
  const agent = useStore((s) => s.agents[agentId]);
  const endCall = useStore((s) => s.endCall);
  const openSettings = useStore((s) => s.openSettings);
  const [state, setState] = useState<CallState>("connecting");
  const [lines, setLines] = useState<Line[]>([]);
  const [level, setLevel] = useState(0);
  const [muted, setMuted] = useState(false);
  const [speaker, setSpeaker] = useState(true);
  const [seconds, setSeconds] = useState(0);
  const [problem, setProblem] = useState<string | null>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const engine = useRef<Call | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);

  useEffect(() => {
    let alive = true;
    const c = new Call(threadId, agentId, {
      state: (s) => alive && setState(s),
      heard: (text) => alive && setLines((l) => [...l, { who: "me", text }]),
      saying: (text) => alive && setLines((l) => [...l, { who: "agent", text }]),
      level: (v) => alive && setLevel(v),
      error: (m) => alive && useStore.getState().showToast(m),
    });
    engine.current = c;
    c.start().catch((e) => {
      if (!alive) return;
      const msg = String(e);
      setProblem(msg.includes("noVoice") ? "noVoice" : msg.includes("Permission") || msg.includes("NotAllowed") ? "mic" : msg);
    });
    return () => {
      alive = false;
      void c.end();
    };
  }, [threadId, agentId]);

  useEffect(() => {
    const id = setInterval(() => setSeconds((s) => s + 1), 1000);
    return () => clearInterval(id);
  }, []);

  useEffect(() => {
    scroller.current?.scrollTo({ top: scroller.current.scrollHeight });
  }, [lines]);

  if (!agent) return null;
  const label = problem ? "" : t(`call.state.${state}`);
  const active = state === "hearing" || state === "speaking";

  return (
    <div
      role="dialog"
      aria-label={t("call.title", { name: agent.name })}
      style={{ transform: `translate(${pos.x}px, ${pos.y}px)` }}
      className="animate-pop fixed right-6 bottom-6 z-[80] flex h-[min(700px,calc(100vh-48px))] w-[360px] flex-col overflow-hidden rounded-[42px] border-[7px] border-[#1c1d21] bg-gradient-to-b from-[#26334d] via-[#1b2236] to-[#11131c] text-white shadow-pop"
    >
      <div
        className="flex h-9 shrink-0 cursor-grab items-start justify-center active:cursor-grabbing"
        onPointerDown={(e) => {
          (e.target as HTMLElement).setPointerCapture(e.pointerId);
          drag.current = { x: e.clientX, y: e.clientY, px: pos.x, py: pos.y };
        }}
        onPointerMove={(e) => drag.current && setPos({ x: drag.current.px + e.clientX - drag.current.x, y: drag.current.py + e.clientY - drag.current.y })}
        onPointerUp={() => (drag.current = null)}
      >
        <div className="mt-2 h-5 w-24 rounded-full bg-black/70" />
      </div>

      <div className="flex flex-col items-center px-6 pt-4">
        <div className="relative flex size-36 items-center justify-center">
          <span className="absolute inset-0 rounded-full bg-white/10 transition-transform duration-100" style={{ transform: `scale(${1 + (active ? 0.18 + level * 0.25 : 0.04)})` }} />
          <span className="absolute inset-3 rounded-full bg-white/10 transition-transform duration-100" style={{ transform: `scale(${1 + (active ? 0.08 + level * 0.15 : 0.02)})` }} />
          <div className="relative rounded-full ring-4 ring-white/20">
            <AvatarView avatar={agent.avatar} name={agent.name} size={104} />
          </div>
        </div>
        <h2 className="mt-4 text-[22px] font-semibold tracking-tight">{agent.name}</h2>
        <p className="mt-0.5 h-5 text-[13px] text-white/70">{problem ? "" : state === "connecting" ? t("call.state.connecting") : `${label} · ${clock(seconds)}`}</p>
      </div>

      <div ref={scroller} className="mt-4 min-h-0 flex-1 space-y-2 overflow-y-auto px-5 pb-3">
        {problem ? (
          <div className="rounded-2xl bg-white/10 px-4 py-3 text-[13px] leading-relaxed">
            {problem === "noVoice" ? t("call.noVoice") : problem === "mic" ? t("call.micBlocked") : problem}
            {problem === "noVoice" && (
              <button
                onClick={() => {
                  endCall();
                  openSettings("voice");
                }}
                className="mt-2 block font-medium text-[#8fc2ff] hover:underline"
              >
                {t("call.openVoice")}
              </button>
            )}
          </div>
        ) : lines.length === 0 ? (
          <p className="pt-6 text-center text-[13px] text-white/50">{t("call.speakNow")}</p>
        ) : (
          lines.slice(-6).map((l, i) => (
            <p key={i} className={cx("max-w-[88%] rounded-2xl px-3.5 py-2 text-[13.5px] leading-snug", l.who === "me" ? "ml-auto bg-[#2f86eb] text-white" : "bg-white/12 text-white/95")}>
              {l.text}
            </p>
          ))
        )}
      </div>

      <div className="flex shrink-0 items-center justify-center gap-6 px-6 pt-2 pb-9">
        <RoundButton
          label={muted ? t("call.unmute") : t("call.mute")}
          on={muted}
          onClick={() => {
            setMuted(!muted);
            engine.current?.setMuted(!muted);
          }}
        >
          {muted ? <MicOff size={22} /> : <Mic size={22} />}
        </RoundButton>
        <button onClick={endCall} aria-label={t("call.end")} data-tip={t("call.end")} className="flex size-16 items-center justify-center rounded-full bg-[#e5484d] shadow-lg hover:bg-[#f2555a]">
          <PhoneOff size={26} />
        </button>
        <RoundButton
          label={speaker ? t("call.speakerOff") : t("call.speakerOn")}
          on={!speaker}
          onClick={() => {
            setSpeaker(!speaker);
            engine.current?.setSpeaker(!speaker);
          }}
        >
          {speaker ? <Volume2 size={22} /> : <VolumeX size={22} />}
        </RoundButton>
      </div>
    </div>
  );
}

function RoundButton({ label, on, onClick, children }: { label: string; on: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick} aria-label={label} aria-pressed={on} data-tip={label} className={cx("flex size-14 items-center justify-center rounded-full transition-colors", on ? "bg-white text-[#11131c]" : "bg-white/15 hover:bg-white/25")}>
      {children}
    </button>
  );
}
