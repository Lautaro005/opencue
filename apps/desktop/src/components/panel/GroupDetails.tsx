import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderOpen, Repeat, Trash2, UserPlus, Users, X } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Group } from "../../lib/types";
import { AvatarView, GroupAvatar } from "../AvatarView";
import { Badge, Button, IconButton, Input, useClickOutside } from "../ui";
import { Section } from "./Section";

export function GroupDetails({ group }: { group: Group }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const setPanel = useStore((s) => s.setPanel);
  const select = useStore((s) => s.select);
  const refresh = useStore((s) => s.refresh);
  const [name, setName] = useState(group.name);
  const [adding, setAdding] = useState(false);
  const addRef = useRef<HTMLDivElement>(null);
  useClickOutside(addRef, () => setAdding(false), adding);
  useEffect(() => setName(group.name), [group.id, group.name]);

  const members = group.members.map((m) => ({ ...m, agent: agents[m.agent_id] })).filter((m) => m.agent);
  const candidates = Object.values(agents).filter((a) => !group.members.some((m) => m.agent_id === a.id));

  return (
    <aside className="animate-slide flex h-full w-[340px] shrink-0 flex-col border-l border-line bg-panel">
      <div className="flex h-13 shrink-0 items-center justify-between px-4">
        <h2 className="text-[14px] font-semibold">{t("group.details")}</h2>
        <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
          <X size={17} />
        </IconButton>
      </div>
      <div className="flex-1 overflow-y-auto">
        <div className="flex flex-col items-center px-5 pt-2 pb-5">
          <GroupAvatar avatars={members.map((m) => m.agent.avatar)} size={88} />
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onBlur={() => name.trim() && name !== group.name && void api.updateGroup(group.id, name).then(refresh)}
            onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
            aria-label={t("group.name")}
            className="mt-3 w-full rounded-lg bg-transparent px-2 py-1 text-center text-[18px] font-semibold tracking-tight outline-none hover:bg-hover focus:bg-hover"
          />
        </div>

        <Section title={`${t("group.members")} · ${members.length}`} icon={<Users size={15} />}>
          <ul className="space-y-0.5">
            {members.map(({ agent, is_owner }) => (
              <li key={agent.id} className="group flex items-center gap-2.5 rounded-lg px-2 py-1.5 hover:bg-hover">
                <button onClick={() => select(agent.id)} className="flex min-w-0 flex-1 items-center gap-2.5 text-left">
                  <AvatarView avatar={agent.avatar} name={agent.name} size={30} />
                  <span className="truncate text-[13.5px] font-medium">{agent.name}</span>
                  {is_owner && <Badge tone="accent">{t("group.owner")}</Badge>}
                </button>
                {!is_owner && (
                  <button
                    onClick={() => void api.removeGroupMember(group.id, agent.id).then(refresh)}
                    className="text-faint opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                    aria-label={t("group.removeMember")}
                    title={t("group.removeMember")}
                  >
                    <X size={14} />
                  </button>
                )}
              </li>
            ))}
          </ul>
          <div className="relative mt-1" ref={addRef}>
            <button onClick={() => setAdding((v) => !v)} className="flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left text-[13.5px] text-accent hover:bg-hover">
              <span className="flex size-7.5 items-center justify-center rounded-full bg-accent-soft">
                <UserPlus size={15} />
              </span>
              {t("group.addMember")}
            </button>
            {adding && (
              <div className="animate-pop absolute left-0 top-full z-30 mt-1 w-full rounded-xl border border-line bg-elevated p-1 shadow-pop">
                {candidates.length === 0 ? (
                  <p className="px-2.5 py-2 text-[12.5px] text-muted">{t("group.noOneToAdd")}</p>
                ) : (
                  candidates.map((a) => (
                    <button
                      key={a.id}
                      onClick={() => {
                        setAdding(false);
                        void api.addGroupMember(group.id, a.id).then(refresh);
                      }}
                      className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[13px] hover:bg-hover"
                    >
                      <AvatarView avatar={a.avatar} name={a.name} size={22} /> {a.name}
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
        </Section>

        <Section title={t("group.autoTurns")} icon={<Repeat size={15} />}>
          <div className="flex items-center gap-3">
            <Input
              type="number"
              min={0}
              max={50}
              defaultValue={group.max_auto_turns}
              key={group.id}
              onBlur={(e) => void api.updateGroup(group.id, undefined, Number(e.target.value)).then(refresh)}
              className="w-20"
            />
            <p className="text-[11.5px] text-muted">{t("group.autoTurnsHint")}</p>
          </div>
        </Section>

        <Section title={t("group.files")} icon={<FolderOpen size={15} />} action={<Badge>{t("common.nextPhase")}</Badge>}>
          <div className="rounded-xl border border-dashed border-line-strong px-4 py-5 text-center text-[12.5px] text-muted">{t("group.filesEmpty")}</div>
        </Section>

        <div className="border-t border-line px-5 py-4">
          <Button
            variant="danger"
            className="w-full"
            onClick={() => {
              if (confirm(t("group.deleteConfirm", { name: group.name }))) {
                setPanel(null);
                void api.deleteGroup(group.id);
              }
            }}
          >
            <Trash2 size={15} /> {t("group.deleteGroup")}
          </Button>
        </div>
      </div>
    </aside>
  );
}
