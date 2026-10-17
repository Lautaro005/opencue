import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { en } from "./en";
import { es } from "./es";

export const LANGUAGES = [
  { code: "en", label: "English" },
  { code: "es", label: "Español" },
] as const;

function initialLanguage() {
  try {
    const saved = localStorage.getItem("opencue.lang");
    if (saved) return saved;
  } catch {}
  return navigator.language?.toLowerCase().startsWith("es") ? "es" : "en";
}

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, es: { translation: es } },
  lng: initialLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export function setLanguage(code: string) {
  void i18n.changeLanguage(code);
  try {
    localStorage.setItem("opencue.lang", code);
  } catch {}
  document.documentElement.lang = code;
}

export default i18n;
