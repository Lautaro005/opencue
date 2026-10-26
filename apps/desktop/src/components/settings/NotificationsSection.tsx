import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Bell } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { Button, Toggle } from "../ui";
import { Card, CardRow, PageTitle } from "./SettingsModal";

/** Which system notifications to show. All default to on, only while the window is out of view. */
export function NotificationsSection() {
  const { t } = useTranslation();
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const [note, setNote] = useState<string | null>(null);

  const on = (key: string) => settings[key] !== "false";
  const master = on("notify_enabled");
  const channel = settings.notify_channel === "notch" ? "notch" : "system";
  const row = (key: string, label: string, hint: string) => (
    <CardRow label={t(label)} hint={t(hint)}>
      <Toggle checked={on(key)} onChange={(v) => void setSetting(key, String(v))} label={t(label)} disabled={!master} />
    </CardRow>
  );

  const test = async () => {
    setNote(null);
    try {
      await api.testNotification(t("settings.notifications.testTitle"), t("settings.notifications.testBody"));
      setNote(t("settings.notifications.sent"));
    } catch (e) {
      setNote(String(e));
    }
  };

  return (
    <>
      <PageTitle title={t("settings.nav.notifications")} hint={t("settings.notifications.hint")} />
      <Card>
        <CardRow label={t("settings.notifications.enable")} hint={t("settings.notifications.enableHint")}>
          <Toggle checked={master} onChange={(v) => void setSetting("notify_enabled", String(v))} label={t("settings.notifications.enable")} />
        </CardRow>
      </Card>
      <Card>
        <CardRow label={t("settings.notifications.where")} hint={t("settings.notifications.whereHint")}>
          <div className="flex gap-1 rounded-lg bg-hover p-0.5" role="radiogroup" aria-label={t("settings.notifications.where")}>
            {(["system", "notch"] as const).map((k) => (
              <button
                key={k}
                role="radio"
                aria-checked={channel === k}
                disabled={!master}
                onClick={() => void setSetting("notify_channel", k)}
                className={`rounded-md px-3 py-1 text-[12.5px] font-medium transition-colors ${channel === k ? "bg-elevated shadow-sm" : "text-muted hover:text-ink"}`}
              >
                {t(k === "system" ? "settings.notifications.whereSystem" : "settings.notifications.whereNotch")}
              </button>
            ))}
          </div>
        </CardRow>
        {channel === "notch" && row("notify_working", "settings.notifications.working", "settings.notifications.workingHint")}
        {row("notify_replies", "settings.notifications.replies", "settings.notifications.repliesHint")}
        {row("notify_approvals", "settings.notifications.approvals", "settings.notifications.approvalsHint")}
        {row("notify_background_only", "settings.notifications.backgroundOnly", "settings.notifications.backgroundOnlyHint")}
        {row("notify_preview", "settings.notifications.preview", "settings.notifications.previewHint")}
      </Card>
      <Button size="sm" onClick={() => void test()}>
        <Bell size={14} /> {t("settings.notifications.test")}
      </Button>
      {note && <p className="selectable mt-2 text-[12px] text-muted">{note}</p>}
    </>
  );
}
