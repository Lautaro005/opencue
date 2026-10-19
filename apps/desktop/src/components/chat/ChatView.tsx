import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Monitor, Phone, SquareUser, Users } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { AvatarView, GroupAvatar } from "../AvatarView";
import { IconButton, Typing } from "../ui";
import { Logo } from "../Sidebar";
import { Composer } from "./Composer";
import { MessageList } from "./MessageList";
import { DmView } from "./DmView";

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
  const setDialog = useStore((s) => s.setDialog);
  const working = useStore((s) => (selected ? s.working[selected] ?? NO_WORKERS : NO_WORKERS));
  const messageCount = useStore((s) => (selected ? s.messages[selected]?.length ?? 0 : 0));
  const dm = useStore((s) => s.dm);
  const startCall = useStore((s) => s.startCall);
  const calling = useStore((s) => s.call?.threadId === s.selected);

  const agent = selected ? agents[selected] : undefined;
  const group = selected ? groups[selected] : undefined;
  const thread = threads.find((th) => th.id === selected);
  const members = useMemo(() => (group ? group.members.map((m) => agents[m.agent_id]).filter(Boolean) : null), [group, agents]);
  // In a 1:1 chat any other agent can be @mentioned: the agent asks them without a group.
  const others = useMemo(() => (agent ? Object.values(agents).filter((a) => a.id !== agent.id) : undefined), [agent, agents]);

  if (!selected || (!agent && !group)) {
    return (
      <main className="flex flex-1 flex-col items-center justify-center gap-3 bg-bg text-center">
        <Logo size={48} />
        <h1 className="text-[19px] font-semibold tracking-tight">{t("chat.selectTitle")}</h1>
        <p className="max-w-xs text-[13.5px] text-muted">{t("chat.selectHint")}</p>
        <button onClick={() => setDialog("create-agent")} className="mt-1 rounded-lg bg-accent px-3.5 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
          {t("sidebar.createAgent")}
        </button>
      </main>
    );
  }

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
            <IconButton label={t("call.start")} active={calling} onClick={() => startCall(agent.id)}>
              <Phone size={18} />
            </IconButton>
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
