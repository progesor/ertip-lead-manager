import { Route, Routes } from "react-router-dom";
import { AppShell } from "../app/AppShell";
import { AnalyticsPage } from "../features/analytics/AnalyticsPage";
import { DashboardPage } from "../features/dashboard/DashboardPage";
import { ImportsPage } from "../features/imports/ImportsPage";
import { LeadsPage } from "../features/leads/LeadsPage";
import { PipelinePage } from "../features/pipeline/PipelinePage";
import { SettingsPage } from "../features/settings/SettingsPage";

export function AppRoutes() {
  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<DashboardPage />} />
        <Route path="leads" element={<LeadsPage />} />
        <Route path="pipeline" element={<PipelinePage />} />
        <Route path="analytics" element={<AnalyticsPage />} />
        <Route path="imports" element={<ImportsPage />} />
        <Route path="settings" element={<SettingsPage />} />
      </Route>
    </Routes>
  );
}
