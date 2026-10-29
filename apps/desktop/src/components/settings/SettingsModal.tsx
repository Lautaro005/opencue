import { useTranslation } from "react-i18next";
import { Bell, AppWindow, AudioLines, BarChart3, Cpu, Globe, HardDrive, Info, MessageCircle, Monitor, Settings2, Sparkles, User, X } from "lucide-react";
import { useStore, type SettingsSection } from "../../lib/store";
import { cx } from "../../lib/format";
import { IconButton, Modal } from "../ui";
import { AboutSection, AccountSection, GeneralSection, InternetSection, UsageSection } from "./sections";
import { ComputersSection, SkillsSection, VoiceSection } from "./Phase2Sections";
import { ModelsSection } from "./ModelsSection";
import { AppsSection } from "./AppsSection";
import { MessengerSection } from "./MessengerSection";
import { NotificationsSection } from "./NotificationsSection";
import { StorageSection } from "./StorageSection";

/** The pages, in groups: you, the AI, what it connects to, and this computer. */
const NAV: { group?: string; items: { id: SettingsSection; icon: React.ReactNode }[] }[] = [
  {
    items: [
      { id: "general", icon: <Settings2 size={16} /> },
      { id: "account", icon: <User size={16} /> },
      { id: "usage", icon: <BarChart3 size={16} /> },
      { id: "notifications", icon: <Bell size={16} /> },
    ],
  },
  {
    group: "ai",
    items: [
      { id: "models", icon: <Cpu size={16} /> },
      { id: "skills", icon: <Sparkles size={16} /> },
    ],
  },
  {
    group: "connections",
    items: [
      { id: "apps", icon: <AppWindow size={16} /> },
      { id: "messenger", icon: <MessageCircle size={16} /> },
      { id: "internet", icon: <Globe size={16} /> },
    ],
  },
  {
    group: "computer",
    items: [
      { id: "computers", icon: <Monitor size={16} /> },
      { id: "voice", icon: <AudioLines size={16} /> },
      { id: "storage", icon: <HardDrive size={16} /> },
    ],
  },
];

const PAGES: Record<SettingsSection, () => React.ReactNode> = {
  general: GeneralSection,
  account: AccountSection,
  usage: UsageSection,
  notifications: NotificationsSection,
  models: ModelsSection,
  skills: SkillsSection,
  apps: AppsSection,
  messenger: MessengerSection,
  internet: InternetSection,
  computers: ComputersSection,
  voice: VoiceSection,
  storage: StorageSection,
  about: AboutSection,
};

export function SettingsModal() {
  const { t } = useTranslation();
  const section = useStore((s) => s.settingsSection);
  const open = useStore((s) => s.openSettings);
  const close = useStore((s) => s.closeSettings);
  const Page = section ? PAGES[section] : null;

  const item = (id: SettingsSection, icon: React.ReactNode) => (
    <button
      key={id}
      onClick={() => open(id)}
      aria-current={section === id ? "page" : undefined}
      className={cx(
        "relative flex items-center gap-2.5 rounded-[10px] px-2.5 py-[7px] text-left text-[13.5px] transition-colors",
        section === id ? "bg-elevated font-medium text-ink shadow-card" : "text-muted hover:bg-hover hover:text-ink",
      )}
    >
      <span className={cx("transition-colors", section === id && "text-accent")}>{icon}</span>
      {t(`settings.nav.${id}`)}
    </button>
  );

  return (
    <Modal open={section !== null} onClose={close} className="flex h-[680px] w-[920px]" label={t("settings.title")}>
      <nav className="flex w-[220px] shrink-0 flex-col border-r border-line bg-sidebar p-2.5">
        <h2 className="px-2.5 pt-2 pb-3 text-[15px] font-semibold tracking-tight">{t("settings.title")}</h2>
        <div className="flex flex-1 flex-col gap-0.5 overflow-y-auto">
          {NAV.map((g, i) => (
            <div key={i} className={cx("flex flex-col gap-0.5", i > 0 && "mt-3")}>
              {g.group && <p className="px-2.5 pb-1 text-[10.5px] font-semibold tracking-[0.08em] text-faint uppercase">{t(`settings.navGroup.${g.group}`)}</p>}
              {g.items.map((n) => item(n.id, n.icon))}
            </div>
          ))}
        </div>
        <div className="mt-2 flex flex-col border-t border-line pt-2">{item("about", <Info size={16} />)}</div>
      </nav>
      <div className="relative flex min-w-0 flex-1 flex-col">
        <IconButton label={t("common.close")} onClick={close} className="absolute top-3 right-3 z-10">
          <X size={17} />
        </IconButton>
        <div key={section} className="animate-fade flex-1 overflow-y-auto px-8 py-7">
          {Page && <Page />}
        </div>
      </div>
    </Modal>
  );
}

/** Tabs at the top of a page; the choice lives in the store so other pages can open a given tab. */
export function SectionTabs<T extends string>({ tabs, fallback, label }: { tabs: { id: T; label: string; count?: number }[]; fallback: T; label: string }) {
  const current = useSectionTab(tabs.map((x) => x.id), fallback);
  return (
    <div className="mb-6 inline-flex rounded-[11px] border border-line bg-sidebar p-[3px]" role="tablist" aria-label={label}>
      {tabs.map((x) => (
        <button
          key={x.id}
          role="tab"
          aria-selected={current === x.id}
          onClick={() => useStore.setState({ settingsTab: x.id })}
          className={cx(
            "flex items-center gap-1.5 rounded-[8px] px-3.5 py-1.5 text-[12.5px] font-medium transition-all",
            current === x.id ? "bg-elevated text-ink shadow-card" : "text-muted hover:text-ink",
          )}
        >
          {x.label}
          {x.count !== undefined && x.count > 0 && <span className="rounded-full bg-hover px-1.5 text-[10.5px] text-muted tabular-nums">{x.count}</span>}
        </button>
      ))}
    </div>
  );
}

/** The tab a page with `SectionTabs` is showing. */
export function useSectionTab<T extends string>(ids: readonly T[], fallback: T): T {
  const raw = useStore((s) => s.settingsTab);
  return (ids.includes(raw as T) ? raw : fallback) as T;
}

export function PageTitle({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="mb-6 pr-8">
      <h1 className="text-[21px] font-semibold tracking-[-0.015em]">{title}</h1>
      {hint && <p className="mt-1 max-w-[560px] text-[13px] leading-relaxed text-muted">{hint}</p>}
    </div>
  );
}

export function Card({ title, children, action }: { title?: string; children: React.ReactNode; action?: React.ReactNode }) {
  return (
    <div className="mb-5">
      {(title || action) && (
        <div className="mb-2 flex items-center justify-between">
          {title && <h3 className="text-[11.5px] font-semibold tracking-[0.06em] text-muted uppercase">{title}</h3>}
          {action}
        </div>
      )}
      <div className="surface divide-y divide-line overflow-hidden rounded-2xl">{children}</div>
    </div>
  );
}

export function CardRow({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-6 px-4 py-3">
      <div className="min-w-0">
        <div className="text-[13.5px]">{label}</div>
        {hint && <div className="mt-0.5 text-[12px] text-muted">{hint}</div>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
