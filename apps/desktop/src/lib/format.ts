import i18n from "../i18n";

/** Sidebar-style timestamp: time today, "Yesterday", weekday this week, otherwise a short date. */
export function shortTime(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  const now = new Date();
  const lang = i18n.language;
  const diffMin = (now.getTime() - d.getTime()) / 60000;
  if (diffMin < 1) return i18n.t("time.now");
  const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((startOfDay(now) - startOfDay(d)) / 86400000);
  if (days === 0) return d.toLocaleTimeString(lang, { hour: "numeric", minute: "2-digit" });
  if (days === 1) return i18n.t("time.yesterday");
  if (days < 7) return d.toLocaleDateString(lang, { weekday: "short" });
  return d.toLocaleDateString(lang, { day: "numeric", month: "short" });
}

export function clockTime(iso: string): string {
  return new Date(iso).toLocaleTimeString(i18n.language, { hour: "numeric", minute: "2-digit" });
}

export function compact(n: number): string {
  return new Intl.NumberFormat(i18n.language, { notation: "compact", maximumFractionDigits: 1 }).format(n);
}

export function cx(...parts: (string | false | null | undefined)[]) {
  return parts.filter(Boolean).join(" ");
}

/** "Mon 9:00" style, for past and upcoming runs. */
export function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleString(i18n.language, { weekday: "short", day: "numeric", month: "short", hour: "numeric", minute: "2-digit" });
}
