import { invoke } from "@tauri-apps/api/core";
import type { AppDiagnostics } from "../../types/diagnostics";

export async function loadAppDiagnostics(): Promise<AppDiagnostics> {
  return invoke<AppDiagnostics>("get_app_diagnostics");
}
