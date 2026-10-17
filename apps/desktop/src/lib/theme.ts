export type Theme = "system" | "light" | "dark";

const media = window.matchMedia("(prefers-color-scheme: dark)");

export function getTheme(): Theme {
  try {
    return (localStorage.getItem("opencue.theme") as Theme) || "system";
  } catch {
    return "system";
  }
}

export function applyTheme(theme: Theme = getTheme()) {
  const dark = theme === "dark" || (theme === "system" && media.matches);
  document.documentElement.classList.toggle("dark", dark);
}

export function setTheme(theme: Theme) {
  try {
    localStorage.setItem("opencue.theme", theme);
  } catch {}
  applyTheme(theme);
}

media.addEventListener("change", () => applyTheme());
