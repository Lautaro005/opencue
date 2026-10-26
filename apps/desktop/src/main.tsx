import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import "./i18n";
import { applyTheme } from "./lib/theme";
import App from "./App";
import { FloatApp } from "./components/FloatApp";

applyTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {window.__FLOAT__ ? <FloatApp /> : <App />}
  </StrictMode>,
);
