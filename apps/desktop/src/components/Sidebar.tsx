import { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bot, MessageSquarePlus, Plus, Settings, Users } from "lucide-react";
import { useStore } from "../lib/store";
import { cx, shortTime } from "../lib/format";
import { openUrl } from "../lib/api";
import { AvatarView, GroupAvatar, UserAvatar } from "./AvatarView";
import { IconButton, Typing, useClickOutside } from "./ui";

export function Sidebar() {
  const { t } = useTranslation();
  const threads = useStore((s) => s.threads);
  const selected = useStore((s) => s.selected);
  const select = useStore((s) => s.select);
  const setDialog = useStore((s) => s.setDialog);
  const openSettings = useStore((s) => s.openSettings);
  const settings = useStore((s) => s.settings);
  const working = useStore((s) => s.working);
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setMenu(false), menu);

  const userName = settings.user_name?.trim() || t("sidebar.you");

  return (
    <aside className="flex h-full w-[280px] shrink-0 flex-col border-r border-line bg-sidebar">
      <div className="flex h-13 items-center justify-between pl-4 pr-2.5" data-tauri-drag-region>
        <div className="flex items-center gap-2" data-tauri-drag-region>
          <Logo />
          <span className="text-[15px] font-semibold tracking-tight">OpenCue</span>
        </div>
        <div className="relative" ref={menuRef}>
          <IconButton label={t("sidebar.new")} onClick={() => setMenu((v) => !v)} active={menu}>
            <Plus size={18} />
          </IconButton>
          {menu && (
            <div className="animate-pop absolute right-0 top-9 z-30 w-52 rounded-xl border border-line bg-elevated p-1 shadow-pop">
              <MenuItem
                icon={<Bot size={16} />}
                label={t("sidebar.createAgent")}
                onClick={() => {
                  setMenu(false);
                  setDialog("create-agent");
                }}
              />
              <MenuItem
                icon={<Users size={16} />}
                label={t("sidebar.createGroup")}
                onClick={() => {
                  setMenu(false);
                  setDialog("create-group");
                }}
              />
            </div>
          )}
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 pb-2" aria-label="Agents">
        {threads.length === 0 ? (
          <div className="mx-2 mt-6 rounded-xl border border-dashed border-line-strong px-4 py-6 text-center">
            <p className="font-medium">{t("sidebar.empty")}</p>
            <p className="mt-1 text-[12.5px] text-muted">{t("sidebar.emptyHint")}</p>
            <button
              onClick={() => setDialog("create-agent")}
              className="mt-3 inline-flex items-center gap-1.5 rounded-lg bg-accent px-3 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover"
            >
              <MessageSquarePlus size={15} /> {t("sidebar.createAgent")}
            </button>
          </div>
        ) : (
          <ul className="space-y-0.5">
            {threads.map((th) => {
              const isWorking = th.busy && (working[th.id]?.length ?? 0) > 0;
              return (
                <li key={th.id}>
                  <button
                    onClick={() => select(th.id)}
                    className={cx(
                      "group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-colors",
                      selected === th.id ? "bg-active" : "hover:bg-hover",
                    )}
                  >
                    {th.kind === "group" ? <GroupAvatar avatars={th.avatars} size={40} /> : <AvatarView avatar={th.avatars[0]} name={th.name} size={40} />}
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline justify-between gap-2">
                        <span className="truncate text-[14px] font-medium">{th.name}</span>
                        <span className="shrink-0 text-[11.5px] text-faint">{shortTime(th.last_at)}</span>
                      </span>
                      <span className="mt-0.5 block truncate text-[12.5px] text-muted">
                        {isWorking ? (
                          <span className="inline-flex items-center gap-1.5 text-accent">
                            <Typing /> {t("sidebar.working")}
                          </span>
                        ) : (
                          th.last_message ?? " "
                        )}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </nav>

      <div className="flex items-center gap-2 border-t border-line px-3 py-2.5">
        <button onClick={() => openSettings("account")} className="flex min-w-0 flex-1 items-center gap-2.5 rounded-lg px-1 py-1 text-left hover:bg-hover">
          <UserAvatar name={userName} size={30} />
          <span className="min-w-0">
            <span className="block truncate text-[13px] font-medium">{userName}</span>
            <span
              role="link"
              onClick={(e) => {
                e.stopPropagation();
                void openUrl("https://lautaro005.github.io/opencue/");
              }}
              className="block truncate text-[11.5px] text-muted hover:text-accent"
            >
              {t("sidebar.seeHow")}
            </span>
          </span>
        </button>
        <IconButton label={t("sidebar.settings")} onClick={() => openSettings()}>
          <Settings size={17} />
        </IconButton>
      </div>
    </aside>
  );
}

function MenuItem({ icon, label, onClick }: { icon: React.ReactNode; label: string; onClick: () => void }) {
  return (
    <button onClick={onClick} className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-[13.5px] hover:bg-hover">
      <span className="text-muted">{icon}</span>
      {label}
    </button>
  );
}

export function Logo({ size = 22 }: { size?: number }) {
  return <AvatarView avatar={{ face: "googly", shape: "bubble", color: "#2f86eb" }} size={size} />;
}
