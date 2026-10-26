import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Bot, Eraser, LayoutTemplate, MessageSquarePlus, PanelLeftClose, PanelLeftOpen, Pencil, Pin, PinOff, Plus, RotateCcw, Search, Settings, Sparkles, SquareTerminal, Trash2, Users } from "lucide-react";
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
  const agents = useStore((s) => s.agents);
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
  const order = useMemo(() => {
    try {
      const v = JSON.parse(settings.thread_order || "[]");
      return Array.isArray(v) ? (v as string[]) : [];
    } catch {
      return [];
    }
  }, [settings.thread_order]);
  // Pinned chats go first, in the order they were pinned; the rest are in the order the user
  // dragged them to (new ones, not placed yet, come first).
  const threads = useMemo(() => {
    const pins = allThreads.filter((th) => pinned.includes(th.id)).sort((a, b) => pinned.indexOf(a.id) - pinned.indexOf(b.id));
    const rest = allThreads.filter((th) => !pinned.includes(th.id));
    const placed = rest.filter((th) => order.includes(th.id)).sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
    return [...pins, ...rest.filter((th) => !order.includes(th.id)), ...placed];
  }, [allThreads, pinned, order]);

  // Drag an agent or group to move it (pinned ones among the pinned, the others among the others).
  const [drag, setDrag] = useState<{ id: string; over: string | null; after: boolean } | null>(null);
  const press = useRef<{ id: string; x: number; y: number; moved: boolean } | null>(null);
  const justDragged = useRef(false);
  const dragRef = useRef(drag);
  dragRef.current = drag;
  const threadsRef = useRef(threads);
  threadsRef.current = threads;
  const commitMove = (id: string, over: string, after: boolean) => {
    const list = threadsRef.current;
    const isPin = pinned.includes(id);
    if (id === over || isPin !== pinned.includes(over)) return;
    const ids = list.filter((th) => pinned.includes(th.id) === isPin).map((th) => th.id).filter((x) => x !== id);
    const at = ids.indexOf(over) + (after ? 1 : 0);
    ids.splice(at, 0, id);
    void setSetting(isPin ? "pinned_threads" : "thread_order", JSON.stringify(ids));
  };
  useEffect(() => {
    const move = (e: PointerEvent) => {
      const p = press.current;
      if (!p) return;
      if (!p.moved && Math.hypot(e.clientX - p.x, e.clientY - p.y) < 6) return;
      p.moved = true;
      const row = document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>("[data-thread]");
      if (!row) return setDrag({ id: p.id, over: null, after: false });
      const r = row.getBoundingClientRect();
      setDrag({ id: p.id, over: row.dataset.thread ?? null, after: e.clientY > r.top + r.height / 2 });
    };
    const up = () => {
      const p = press.current;
      press.current = null;
      const d = dragRef.current;
      if (p?.moved) {
        justDragged.current = true;
        window.setTimeout(() => (justDragged.current = false), 0);
        if (d?.over) commitMove(d.id, d.over, d.after);
      }
      setDrag(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }); // eslint-disable-line react-hooks/exhaustive-deps

  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("opencue:sidebar-collapsed") === "1";
    } catch {
      return false;
    }
  });
  const toggleCollapsed = () =>
    setCollapsed((v) => {
      try {
        localStorage.setItem("opencue:sidebar-collapsed", v ? "0" : "1");
      } catch {
        /* storage unavailable */
      }
      return !v;
    });
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
    <aside style={{ width: collapsed ? 64 : width }} className="relative flex h-full shrink-0 flex-col border-r border-line bg-sidebar transition-[width] duration-150">
      {!collapsed && handle}
      <div className={cx("flex items-center", collapsed ? "flex-col gap-1 pt-2.5 pb-1" : "h-13 justify-between pl-4 pr-2.5")} data-tauri-drag-region>
        <div className="flex min-w-0 items-center gap-0.5">
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
            {!collapsed && <span className="text-[15px] font-semibold tracking-tight">OpenCue</span>}
          </a>
          <IconButton label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")} onClick={toggleCollapsed} className="size-7">
            {collapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          </IconButton>
        </div>
        <div className={cx("relative flex items-center", collapsed && "flex-col")} ref={menuRef}>
          <IconButton label={t("search.title")} onClick={() => setSearchOpen(true)}>
            <Search size={17} />
          </IconButton>
          <IconButton label={t("sidebar.new")} onClick={() => setMenu((v) => !v)} active={menu}>
            <Plus size={18} />
          </IconButton>
          {menu && (
            <div className={cx("animate-pop absolute top-9 z-30 w-56 rounded-xl border border-line bg-elevated p-1 shadow-pop", collapsed ? "left-0" : "right-0")}>
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
              <MenuItem
                icon={<SquareTerminal size={16} />}
                label={t("cli.title")}
                onClick={() => {
                  setMenu(false);
                  setDialog("cli-agents");
                }}
              />
            </div>
          )}
        </div>
      </div>

      <nav className={cx("flex-1 overflow-y-auto pb-2", collapsed ? "px-1.5" : "px-2")} aria-label="Agents">
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
              const workers = th.busy ? working[th.id] ?? [] : [];
              const isWorking = workers.length > 0;
              const workerAgents = workers.map((id) => agents[id]).filter(Boolean);
              const dropping = drag?.over === th.id && drag.id !== th.id;
              return (
                <li
                  key={th.id}
                  data-thread={th.id}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setThreadMenu({ id: th.id, x: e.clientX, y: e.clientY });
                  }}
                  className={cx("relative", drag?.id === th.id && "opacity-40")}
                >
                  {dropping && <span className={cx("pointer-events-none absolute right-2 left-2 z-10 h-0.5 rounded-full bg-accent", drag?.after ? "-bottom-px" : "-top-px")} />}
                  <button
                    onPointerDown={(e) => {
                      if (e.button === 0 && renaming !== th.id) press.current = { id: th.id, x: e.clientX, y: e.clientY, moved: false };
                    }}
                    onClick={() => !justDragged.current && renaming !== th.id && select(th.id)}
                    aria-label={collapsed ? th.name : undefined}
                    data-tip={collapsed ? th.name : undefined}
                    className={cx(
                      "group flex w-full items-center rounded-xl py-2 text-left transition-colors",
                      collapsed ? "justify-center px-0" : "gap-3 px-2.5",
                      selected === th.id ? "bg-active" : "hover:bg-hover",
                    )}
                  >
                    <span className="relative shrink-0">
                      {th.kind === "group" ? <GroupAvatar avatars={th.avatars} size={40} /> : <AvatarView avatar={th.avatars[0]} name={th.name} size={40} />}
                      {isWorking && <span className="absolute -right-0.5 -bottom-0.5 size-3 animate-pulse rounded-full bg-accent ring-2 ring-sidebar" />}
                    </span>
                    {!collapsed && (
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
                            <span className="inline-flex max-w-full items-center gap-1.5 text-accent">
                              {th.kind === "group" && workerAgents.length > 0 ? (
                                <>
                                  <span className="flex shrink-0 -space-x-1">
                                    {workerAgents.slice(0, 3).map((a) => (
                                      <span key={a.id} className="animate-bounce rounded-full ring-2 ring-sidebar" style={{ animationDuration: "1.1s", animationDelay: `${(a.name.length % 5) * 90}ms` }}>
                                        <AvatarView avatar={a.avatar} name={a.name} size={15} />
                                      </span>
                                    ))}
                                  </span>
                                  <span className="truncate">{workerAgents.map((a) => a.name).join(", ")}</span>
                                </>
                              ) : (
                                <>
                                  <Typing /> {t("sidebar.working")}
                                </>
                              )}
                            </span>
                          ) : (
                            (th.last_message ? splitThink(th.last_message).text || " " : " ")
                          )}
                        </span>
                      </span>
                    )}
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
      {update && !collapsed && (
        <button onClick={() => openSettings("about")} className="mx-3 mb-2 flex items-center gap-2 rounded-xl bg-accent-soft px-3 py-2 text-left text-[12.5px] font-medium text-accent hover:opacity-90">
          <Sparkles size={14} /> {t("update.available", { version: update.version })}
        </button>
      )}
      <div className={cx("flex items-center gap-2 border-t border-line py-2.5", collapsed ? "flex-col px-1" : "px-3")}>
        <button onClick={() => openSettings("account")} className="flex min-w-0 flex-1 items-center gap-2.5 rounded-lg px-1 py-1 text-left hover:bg-hover" aria-label={userName}>
          <UserAvatar name={userName} size={30} />
          {!collapsed && (
            <span className="min-w-0">
              <span className="block truncate text-[13px] font-medium">{userName}</span>
            </span>
          )}
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
