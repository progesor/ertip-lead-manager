import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import "./features/leads/lead-detail-production.css";
import "./features/leads/lead-list-link.css";
import "./features/leads/product-overrides.css";
import "./features/pipeline/pipeline-follow-up.css";
import "./styles.css";
import "./theme.css";
import "./readability-patch.css";
import "./dark-mode-polish.css";

const storedTheme = window.localStorage.getItem("ertip-lead-manager-theme");
const initialTheme =
  storedTheme === "light" || storedTheme === "dark"
    ? storedTheme
    : window.matchMedia?.("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";

document.documentElement.dataset.theme = initialTheme;
document.documentElement.style.colorScheme = initialTheme;

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root element bulunamadı.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
