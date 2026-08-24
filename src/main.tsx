import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import "./features/leads/lead-detail-production.css";
import "./features/leads/lead-list-link.css";
import "./features/leads/product-overrides.css";
import "./features/pipeline/pipeline-follow-up.css";
import "./styles.css";
import "./theme.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root element bulunamadı.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
