import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Bot, Eraser, LayoutTemplate, MessageSquarePlus, Pencil, Pin, PinOff, Plus, RotateCcw, Search, Settings, Sparkles, Trash2, Users } from "lucide-react";
import { useStore } from "../lib/store";
import { cx, shortTime, splitThink } from "../lib/format";
import { api } from "../lib/api";
import { confirmDialog } from "../lib/confirm";
import { AvatarView, GroupAvatar, UserAvatar } from "./AvatarView";
import { IconButton, Typing, useClickOutside } from "./ui";
import { useResizable } from "./ui/Resizer";

export function Sidebar() {
  const { t } = useTranslation();
  const allThreads = useStore((s) => s.threads);
  const selected = useStore((s) => s.selected);
  const select = useStore((s) => s.select);
  const setDialog = useStore((s) => s.setDialog);
  const setSearchOpen = useStore((s) => s.setSearchOpen);
  const openSettings = useStore((s) => s.openSettings);
  const update = useStore((s) => s.update);
  const settings = useStore((s) => s.settings);
  const working = useStore((s) => s.working);
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, () => setMenu(false), menu);

  const setSetting = useStore((s) => s.setSetting);
  const pinned = useMemo(() => {
    try {
      const v = JSON.parse(settings.pinned_threads || "[]");
      return Array.isArray(v) ? (v as string[]) : [];
    } catch {
      return [];
    }
  }, [settings.pinned_threads]);
  // Pinned chats go first, in the order they were pinned; the rest keep the usual order.
  const threads = useMemo(() => {
    const pins = allThreads.filter((th) => pinned.includes(th.id)).sort((a, b) => pinned.indexOf(a.id) - pinned.indexOf(b.id));
    return [...pins, ...allThreads.filter((th) => !pinned.includes(th.id))];
  }, [allThreads, pinned]);
  const [tmenu, setThreadMenu] = useState<{ id: string; x: number; y: number } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const togglePin = (id: string) => void setSetting("pinned_threads", JSON.stringify(pinned.includes(id) ? pinned.filter((x) => x !== id) : [...pinned, id]));
  const saveName = async (id: string, kind: "agent" | "group", name: string) => {
    setRenaming(null);
    const n = name.trim();
    if (!n) return;
    if (kind === "agent") await api.updateAgent(id, { name: n });
    else await api.updateGroup(id, n);
    void useStore.getState().refresh();
  };

  const userName = settings.user_name?.trim() || t("sidebar.you");
  const { width, handle } = useResizable("opencue:sidebar-width", 280, 220, 460, "right");

  return (
    <aside style={{ width }} className="relative flex h-full shrink-0 flex-col border-r border-line bg-sidebar">
      {handle}
      <div className="flex h-13 items-center justify-between pl-4 pr-2.5" data-tauri-drag-region>
        {/* The logo is the way home: it opens the dashboard. */}
        <a
          href="#dashboard"
          onClick={(e) => {
            e.preventDefault();
            select(null);
          }}
          aria-label={t("dashboard.title")}
          data-tip={t("dashboard.title")}
          aria-current={selected === null ? "page" : undefined}
          className="flex items-center gap-2 rounded-lg py-1 pr-2 pl-1 -ml-1 hover:bg-hover"
        >
          <Logo />
          <span className="text-[15px] font-semibold tracking-tight">OpenCue</span>
        </a>
        <div className="relative flex items-center" ref={menuRef}>
          <IconButton label={t("search.title")} onClick={() => setSearchOpen(true)}>
            <Search size={17} />
          </IconButton>
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
              <MenuItem
                icon={<LayoutTemplate size={16} />}
                label={t("sidebar.templates")}
                onClick={() => {
                  setMenu(false);
                  setDialog("templates");
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
                <li
                  key={th.id}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setThreadMenu({ id: th.id, x: e.clientX, y: e.clientY });
                  }}
                >
                  <button
                    onClick={() => renaming !== th.id && select(th.id)}
                    className={cx(
                      "group flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-colors",
                      selected === th.id ? "bg-active" : "hover:bg-hover",
                    )}
                  >
                    {th.kind === "group" ? <GroupAvatar avatars={th.avatars} size={40} /> : <AvatarView avatar={th.avatars[0]} name={th.name} size={40} />}
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline justify-between gap-2">
                        {renaming === th.id ? (
                          <input
                            autoFocus
                            defaultValue={th.name}
                            onClick={(e) => e.stopPropagation()}
                            onBlur={(e) => void saveName(th.id, th.kind, e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === "Enter") e.currentTarget.blur();
                              if (e.key === "Escape") setRenaming(null);
                            }}
                            className="min-w-0 flex-1 rounded-md border border-accent bg-elevated px-1.5 py-0.5 text-[14px] font-medium outline-none"
                          />
                        ) : (
                          <span className="flex min-w-0 items-center gap-1.5">
                            {pinned.includes(th.id) && <Pin size={11} className="shrink-0 text-faint" aria-label={t("sidebar.menu.pinned")} />}
                            <span className="truncate text-[14px] font-medium">{th.name}</span>
                          </span>
                        )}
                        <span className="shrink-0 text-[11.5px] text-faint">{shortTime(th.last_at)}</span>
                      </span>
                      <span className="mt-0.5 block truncate text-[12.5px] text-muted">
                        {isWorking ? (
                          <span className="inline-flex items-center gap-1.5 text-accent">
                            <Typing /> {t("sidebar.working")}
                          </span>
                        ) : (
                          (th.last_message ? splitThink(th.last_message).text || " " : " ")
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

      {tmenu && (
        <ThreadMenu
          x={tmenu.x}
          y={tmenu.y}
          pinned={pinned.includes(tmenu.id)}
          onClose={() => setThreadMenu(null)}
          onRename={() => setRenaming(tmenu.id)}
          onPin={() => togglePin(tmenu.id)}
          onClear={async () => {
            if (await confirmDialog(t("chat.clearConfirm"), { confirmLabel: t("chat.clear") })) {
              await api.clearThread(tmenu.id);
              void useStore.getState().loadMessages(tmenu.id);
            }
          }}
          onReset={async () => {
            if (await confirmDialog(t("sidebar.menu.resetConfirm"), { confirmLabel: t("sidebar.menu.reset") })) {
              await api.resetThread(tmenu.id);
              void useStore.getState().loadMessages(tmenu.id);
            }
          }}
          onDelete={async () => {
            const th = allThreads.find((x) => x.id === tmenu.id);
            if (!th) return;
            const msg = th.kind === "group" ? t("group.deleteConfirm", { name: th.name }) : t("profile.deleteConfirm", { name: th.name });
            if (await confirmDialog(msg, { confirmLabel: t("common.delete") })) {
              if (pinned.includes(th.id)) togglePin(th.id);
              if (useStore.getState().selected === th.id) useStore.getState().setPanel(null);
              if (th.kind === "group") await api.deleteGroup(th.id);
              else await api.deleteAgent(th.id);
              void useStore.getState().refresh();
            }
          }}
        />
      )}
      {update && (
        <button onClick={() => openSettings("about")} className="mx-3 mb-2 flex items-center gap-2 rounded-xl bg-accent-soft px-3 py-2 text-left text-[12.5px] font-medium text-accent hover:opacity-90">
          <Sparkles size={14} /> {t("update.available", { version: update.version })}
        </button>
      )}
      <div className="flex items-center gap-2 border-t border-line px-3 py-2.5">
        <button onClick={() => openSettings("account")} className="flex min-w-0 flex-1 items-center gap-2.5 rounded-lg px-1 py-1 text-left hover:bg-hover">
          <UserAvatar name={userName} size={30} />
          <span className="min-w-0">
            <span className="block truncate text-[13px] font-medium">{userName}</span>
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

/** The right-click menu of an agent or group in the list. */
function ThreadMenu({
  x,
  y,
  pinned,
  onClose,
  onRename,
  onPin,
  onClear,
  onReset,
  onDelete,
}: {
  x: number;
  y: number;
  pinned: boolean;
  onClose: () => void;
  onRename: () => void;
  onPin: () => void;
  onClear: () => void;
  onReset: () => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const node = ref.current;
    if (!node) return;
    node.style.left = Math.min(x, window.innerWidth - node.offsetWidth - 8) + "px";
    node.style.top = Math.min(y, window.innerHeight - node.offsetHeight - 8) + "px";
  }, [x, y]);
  useEffect(() => {
    const away = (e: Event) => !ref.current?.contains(e.target as Node) && onClose();
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("mousedown", away, true);
    document.addEventListener("contextmenu", away, true);
    window.addEventListener("keydown", key);
    window.addEventListener("blur", onClose);
    return () => {
      document.removeEventListener("mousedown", away, true);
      document.removeEventListener("contextmenu", away, true);
      window.removeEventListener("keydown", key);
      window.removeEventListener("blur", onClose);
    };
  }, [onClose]);
  const items: { label: string; icon: React.ReactNode; run: () => void; danger?: boolean }[] = [
    { label: t("sidebar.menu.rename"), icon: <Pencil size={14} />, run: onRename },
    { label: pinned ? t("sidebar.menu.unpin") : t("sidebar.menu.pin"), icon: pinned ? <PinOff size={14} /> : <Pin size={14} />, run: onPin },
    { label: t("sidebar.menu.clear"), icon: <Eraser size={14} />, run: onClear },
    { label: t("sidebar.menu.reset"), icon: <RotateCcw size={14} />, run: onReset },
    { label: t("sidebar.menu.delete"), icon: <Trash2 size={14} />, run: onDelete, danger: true },
  ];
  return createPortal(
    <div ref={ref} role="menu" style={{ position: "fixed", left: x, top: y }} className="animate-pop z-[95] w-48 rounded-xl border border-line bg-elevated p-1 shadow-pop">
      {items.map((i) => (
        <button
          key={i.label}
          role="menuitem"
          onClick={() => {
            onClose();
            i.run();
          }}
          className={cx("flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover", i.danger && "text-danger")}
        >
          <span className={i.danger ? "" : "text-muted"}>{i.icon}</span>
          {i.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
