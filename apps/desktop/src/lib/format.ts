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

export function formatBytes(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
  if (n >= 1024) return `${Math.round(n / 1024)} KB`;
  return `${n} B`;
}

/** Plain-language description of common 5-field cron expressions; falls back to the expression. */
export function describeCron(cron: string): string {
  const t = i18n.t.bind(i18n);
  const parts = cron.trim().split(/\s+/);
  if (parts.length !== 5) return cron;
  const [min, hour, dom, mon, dow] = parts;
  const num = (s: string) => (/^\d+$/.test(s) ? Number(s) : null);
  const time = (h: number, m: number) => new Date(2000, 0, 1, h, m).toLocaleTimeString(i18n.language, { hour: "numeric", minute: "2-digit" });
  const dayName = (d: number) => new Date(2000, 0, 2 + (d % 7)).toLocaleDateString(i18n.language, { weekday: "long" }); // 2000-01-02 was a Sunday
  const step = (s: string) => s.match(/^\*\/(\d+)$/)?.[1];

  if (step(min) && hour === "*" && dom === "*" && mon === "*" && dow === "*") return t("cron.everyMinutes", { n: step(min) });
  if (num(min) !== null && step(hour) && dom === "*" && mon === "*" && dow === "*") return t("cron.everyHours", { n: step(hour) });
  if (num(min) !== null && hour === "*" && dom === "*" && mon === "*" && dow === "*") return t("cron.hourly", { minute: String(num(min)).padStart(2, "0") });
  const h = num(hour);
  const m = num(min);
  if (h === null || m === null || mon !== "*") return cron;
  const at = time(h, m);
  if (dom === "*" && dow === "*") return t("cron.daily", { time: at });
  if (dom === "*" && (dow === "1-5" || dow === "MON-FRI")) return t("cron.weekdays", { time: at });
  if (dom === "*" && /^[0-7](,[0-7])*$/.test(dow)) {
    const days = dow.split(",").map((d) => dayName(Number(d)));
    return t("cron.weekly", { days: new Intl.ListFormat(i18n.language, { type: "conjunction" }).format(days), time: at });
  }
  if (num(dom) !== null && dow === "*") return t("cron.monthly", { day: num(dom), time: at });
  return cron;
}
