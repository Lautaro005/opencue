import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AudioLines, Monitor, Phone, SquareUser, Users } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, openUrl } from "../../lib/api";
import { AvatarView, GroupAvatar } from "../AvatarView";
import { IconButton, Typing, useClickOutside } from "../ui";
import { Composer } from "./Composer";
import { MessageList } from "./MessageList";
import { DmView } from "./DmView";
import { Dashboard } from "../dashboard/Dashboard";

const NO_WORKERS: string[] = [];

export function ChatView() {
  const { t } = useTranslation();
  const selected = useStore((s) => s.selected);
  const agents = useStore((s) => s.agents);
  const groups = useStore((s) => s.groups);
  const threads = useStore((s) => s.threads);
  const panel = useStore((s) => s.panel);
  const setPanel = useStore((s) => s.setPanel);
  const send = useStore((s) => s.send);
  const working = useStore((s) => (selected ? s.working[selected] ?? NO_WORKERS : NO_WORKERS));
  const messageCount = useStore((s) => (selected ? s.messages[selected]?.length ?? 0 : 0));
  const dm = useStore((s) => s.dm);
  const startCall = useStore((s) => s.startCall);
  const calling = useStore((s) => s.call?.threadId === s.selected);
  const openSettings = useStore((s) => s.openSettings);
  const settingsOpen = useStore((s) => s.settingsSection);
  // Calls need an ElevenLabs key: the phone is gray until it is set.
  const [voiceReady, setVoiceReady] = useState(true);
  useEffect(() => {
    void api.voiceSettings().then((v) => setVoiceReady(v.configured)).catch(() => {});
  }, [settingsOpen]);

  const agent = selected ? agents[selected] : undefined;
  const group = selected ? groups[selected] : undefined;
  const thread = threads.find((th) => th.id === selected);
  const members = useMemo(() => (group ? group.members.map((m) => agents[m.agent_id]).filter(Boolean) : null), [group, agents]);
  // In a 1:1 chat any other agent can be @mentioned: the agent asks them without a group.
  const others = useMemo(() => (agent ? Object.values(agents).filter((a) => a.id !== agent.id) : undefined), [agent, agents]);

  if (!selected || (!agent && !group)) return <Dashboard />;

  if (dm && dm.a === selected && agent) return <DmView />;

  const name = agent?.name ?? group!.name;
  const busy = !!thread?.busy;
  const workerNames = working.map((id) => agents[id]?.name).filter(Boolean);
  const subtitle = busy && workerNames.length ? t("chat.working", { name: workerNames.join(", ") }) : group ? members!.map((m) => m.name).join(", ") : agent?.model ?? "";

  return (
    <main className="flex min-w-0 flex-1 flex-col bg-bg">
      <header className="flex h-13 shrink-0 items-center gap-3 border-b border-line px-4" data-tauri-drag-region>
        <button onClick={() => setPanel(panel ? null : group ? "group" : "profile")} className="flex min-w-0 items-center gap-2.5 rounded-lg py-1 pr-2 pl-1 hover:bg-hover">
          {group ? <GroupAvatar avatars={thread?.avatars ?? []} size={30} /> : <AvatarView avatar={agent!.avatar} name={name} size={30} />}
          <span className="min-w-0 text-left">
            <span className="block truncate text-[14px] font-semibold leading-tight">{name}</span>
            <span className="flex items-center gap-1.5 truncate text-[11.5px] leading-tight text-muted">
              {busy && <Typing />}
              {subtitle}
            </span>
          </span>
        </button>
        <div className="ml-auto flex items-center gap-0.5">
          {agent && (
            <CallButton
              voiceReady={voiceReady}
              calling={calling}
              onCall={() => (voiceReady ? startCall(agent.id) : openSettings("voice"))}
            />
          )}
          {agent && (
            <IconButton label={t("computer.title")} active={panel === "computer"} onClick={() => setPanel(panel === "computer" ? null : "computer")}>
              <Monitor size={18} />
            </IconButton>
          )}
          <IconButton
            label={group ? t("chat.groupDetails") : t("chat.profile")}
            active={panel === "profile" || panel === "group"}
            onClick={() => setPanel(panel === "profile" || panel === "group" ? null : group ? "group" : "profile")}
          >
            {group ? <Users size={18} /> : <SquareUser size={18} />}
          </IconButton>
        </div>
      </header>

      {messageCount === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
          {group ? <GroupAvatar avatars={thread?.avatars ?? []} size={64} /> : <AvatarView avatar={agent!.avatar} name={name} size={64} />}
          <h2 className="mt-2 text-[18px] font-semibold tracking-tight">{t("chat.emptyTitle", { name })}</h2>
          <p className="max-w-sm text-[13.5px] text-muted">{group ? t("chat.groupEmptyHint") : t("chat.emptyHint", { name })}</p>
        </div>
      ) : (
        <MessageList threadId={selected} isGroup={!!group} />
      )}

      <Composer threadId={selected} name={name} members={members} mentionable={others} busy={busy} onSend={(text, files) => void send(selected, text, files)} onStop={() => void api.stopThread(selected)} />
    </main>
  );
}

/** The phone: a normal call, or (when PersonaPlex is running) a real-time voice call with it. */
function CallButton({ voiceReady, calling, onCall }: { voiceReady: boolean; calling: boolean; onCall: () => void }) {
  const { t } = useTranslation();
  const [menu, setMenu] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useClickOutside(ref, () => setMenu(null), menu !== null);
  const click = async () => {
    const p = await api.personaStatus().catch(() => null);
    if (p?.running && p.ready && p.url) setMenu(p.url);
    else onCall();
  };
  return (
    <div className="relative" ref={ref}>
      <IconButton label={voiceReady ? t("call.start") : t("call.needKey")} active={calling} onClick={() => void click()} className={voiceReady ? undefined : "opacity-40"}>
        <Phone size={18} />
      </IconButton>
      {menu && (
        <div className="animate-pop absolute top-9 right-0 z-30 w-60 rounded-xl border border-line bg-elevated p-1 shadow-pop">
          <button
            onClick={() => {
              setMenu(null);
              onCall();
            }}
            className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] hover:bg-hover"
          >
            <Phone size={15} className="text-muted" /> {t("call.normal")}
          </button>
          <button
            onClick={() => {
              void openUrl(menu);
              setMenu(null);
            }}
            className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13px] hover:bg-hover"
          >
            <AudioLines size={15} className="text-muted" />
            <span>
              {t("call.persona")}
              <span className="block text-[11.5px] text-muted">{t("call.personaHint")}</span>
            </span>
          </button>
        </div>
      )}
    </div>
  );
}
