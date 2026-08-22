import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import "./features/leads/lead-list-link.css";
import "./styles.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root element bulunamadı.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
