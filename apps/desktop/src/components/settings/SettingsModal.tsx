import { useTranslation } from "react-i18next";
import { Bell, AppWindow, ArrowUpRight, AudioLines, BarChart3, Cpu, Globe, KeyRound, Info, LifeBuoy, MessageCircle, Monitor, Settings2, Sparkles, User, X } from "lucide-react";
import { useStore, type SettingsSection } from "../../lib/store";
import { cx } from "../../lib/format";
import { IconButton, Modal } from "../ui";
import { AboutSection, AccountSection, GeneralSection, InternetSection, UsageSection } from "./sections";
import { ComputersSection, SkillsSection, VoiceSection } from "./Phase2Sections";
import { openUrl } from "../../lib/api";
import { ModelsSection } from "./ModelsSection";
import { ProvidersSection } from "./ProvidersSection";
import { AppsSection } from "./AppsSection";
import { MessengerSection } from "./MessengerSection";
import { NotificationsSection } from "./NotificationsSection";

const NAV: ({ id: SettingsSection; icon: React.ReactNode } | "divider")[] = [
  { id: "general", icon: <Settings2 size={16} /> },
  { id: "account", icon: <User size={16} /> },
  { id: "usage", icon: <BarChart3 size={16} /> },
  { id: "notifications", icon: <Bell size={16} /> },
  { id: "providers", icon: <KeyRound size={16} /> },
  { id: "models", icon: <Cpu size={16} /> },
  "divider",
  { id: "apps", icon: <AppWindow size={16} /> },
  { id: "messenger", icon: <MessageCircle size={16} /> },
  { id: "skills", icon: <Sparkles size={16} /> },
  "divider",
  { id: "computers", icon: <Monitor size={16} /> },
  { id: "voice", icon: <AudioLines size={16} /> },
  { id: "internet", icon: <Globe size={16} /> },
  "divider",
  { id: "about", icon: <Info size={16} /> },
];

export function SettingsModal() {
  const { t } = useTranslation();
  const section = useStore((s) => s.settingsSection);
  const open = useStore((s) => s.openSettings);
  const close = useStore((s) => s.closeSettings);

  return (
    <Modal open={section !== null} onClose={close} className="flex h-[640px] w-[880px]" label={t("settings.title")}>
      <nav className="flex w-[210px] shrink-0 flex-col gap-0.5 border-r border-line bg-sidebar p-2.5">
        <h2 className="px-2.5 pt-2 pb-3 text-[15px] font-semibold">{t("settings.title")}</h2>
        {NAV.map((n, i) =>
          n === "divider" ? (
            <div key={i} className="mx-2.5 my-1.5 border-t border-line" />
          ) : (
          <button
            key={n.id}
            onClick={() => open(n.id)}
            className={cx("flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13.5px]", section === n.id ? "bg-active font-medium text-ink" : "text-muted hover:bg-hover hover:text-ink")}
          >
            {n.icon}
            {t(`settings.nav.${n.id}`)}
          </button>
          ),
        )}
        <button
          onClick={() => void openUrl("https://github.com/Lautaro005/opencue/issues")}
          className="flex items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13.5px] text-muted hover:bg-hover hover:text-ink"
        >
          <LifeBuoy size={16} />
          <span className="flex-1">{t("settings.nav.help")}</span>
          <ArrowUpRight size={14} />
        </button>
      </nav>
      <div className="relative flex min-w-0 flex-1 flex-col">
        <IconButton label={t("common.close")} onClick={close} className="absolute top-3 right-3 z-10">
          <X size={17} />
        </IconButton>
        <div className="flex-1 overflow-y-auto px-8 py-7">
          {section === "general" && <GeneralSection />}
          {section === "account" && <AccountSection />}
          {section === "usage" && <UsageSection />}
          {section === "notifications" && <NotificationsSection />}
          {section === "providers" && <ProvidersSection />}
          {section === "models" && <ModelsSection />}
          {section === "apps" && <AppsSection />}
          {section === "messenger" && <MessengerSection />}
          {section === "skills" && <SkillsSection />}
          {section === "computers" && <ComputersSection />}
          {section === "voice" && <VoiceSection />}
          {section === "about" && <AboutSection />}
          {section === "internet" && <InternetSection />}
        </div>
      </div>
    </Modal>
  );
}

export function PageTitle({ title, hint }: { title: string; hint?: string }) {
  return (
    <div className="mb-6 pr-8">
      <h1 className="text-[20px] font-semibold tracking-tight">{title}</h1>
      {hint && <p className="mt-1 text-[13px] text-muted">{hint}</p>}
    </div>
  );
}

export function Card({ title, children, action }: { title?: string; children: React.ReactNode; action?: React.ReactNode }) {
  return (
    <div className="mb-5">
      {(title || action) && (
        <div className="mb-2 flex items-center justify-between">
          {title && <h3 className="text-[12.5px] font-semibold tracking-wide text-muted uppercase">{title}</h3>}
          {action}
        </div>
      )}
      <div className="divide-y divide-line rounded-xl border border-line bg-bg">{children}</div>
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
