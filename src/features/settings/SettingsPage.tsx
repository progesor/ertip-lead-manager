import { useEffect, useState } from "react";
import { TeamSettingsPanel } from "../team/TeamSettingsPanel";
import { loadAppDiagnostics } from "../../lib/tauri/diagnostics";
import type { AppDiagnostics } from "../../types/diagnostics";

export function SettingsPage() {
  const [diagnostics, setDiagnostics] = useState<AppDiagnostics | null>(null);
  const [runtimeUnavailable, setRuntimeUnavailable] = useState(false);

  useEffect(() => {
    let active = true;

    loadAppDiagnostics()
      .then((result) => {
        if (active) {
          setDiagnostics(result);
        }
      })
      .catch(() => {
        if (active) {
          setRuntimeUnavailable(true);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return (
    <section className="page-stack">
      <div className="page-heading">
        <div>
          <div className="eyebrow">SYSTEM</div>
          <h1>Ayarlar</h1>
          <p>Ekip, yerel veri yolu ve sistem çalışma bilgilerini yönetin.</p>
        </div>
      </div>

      <TeamSettingsPanel />

      <article className="panel diagnostics-panel">
        <div className="panel-heading">
          <div>
            <h2>Sistem Bilgisi</h2>
            <p>Bu bilgiler müşteri verisi içermez.</p>
          </div>
        </div>

        {runtimeUnavailable ? (
          <div className="notice">
            Tauri çalışma zamanı bulunamadı. Frontend-only Vite modunda bu durum beklenir; `npm run tauri:dev`
            ile gerçek tanılama bilgileri görünür.
          </div>
        ) : null}

        <dl className="diagnostics-grid">
          <div>
            <dt>Uygulama sürümü</dt>
            <dd>{diagnostics?.appVersion ?? "Yükleniyor…"}</dd>
          </div>
          <div>
            <dt>Şema sürümü</dt>
            <dd>{diagnostics?.schemaVersion ?? "—"}</dd>
          </div>
          <div className="diagnostics-wide">
            <dt>SQLite veri yolu</dt>
            <dd className="mono-value">{diagnostics?.databasePath ?? "Yükleniyor…"}</dd>
          </div>
        </dl>
      </article>
    </section>
  );
}
