import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import type {
  CommandError,
  IdentityDecision,
  ImportPreview,
  NormalizationWarning,
  ProductCode,
} from "./types";

const productLabels: Record<ProductCode, string> = {
  FUE_MICROMOTOR_SYSTEMS: "FUE Micromotor",
  LONG_HAIR_FUE_SOLUTIONS: "Long Hair FUE",
  FUE_PUNCHES: "FUE Punches",
  IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS: "Implanter / Forceps",
  MEDICAL_CHAIRS_CLINIC_FURNITURE: "Medikal Mobilya",
  OTHER_GENERAL_INFORMATION: "Diğer / Genel Bilgi",
  UNKNOWN: "Bilinmiyor",
};

const warningLabels: Record<NormalizationWarning, string> = {
  INVALID_EMAIL: "Geçersiz e-posta",
  INVALID_PHONE: "Geçersiz telefon",
  INVALID_COUNTRY: "Geçersiz ülke",
  INVALID_TIMESTAMP: "Geçersiz tarih",
  MISSING_CONTACT_METHOD: "İletişim bilgisi yok",
  UNKNOWN_PRODUCT: "Ürün eşleşmedi",
};

function decisionLabel(decision: IdentityDecision) {
  switch (decision.outcome) {
    case "NEW_CONTACT":
      return { label: "Yeni", tone: "new" };
    case "REPEAT_CONTACT":
      return { label: "Repeat", tone: "repeat" };
    case "EXACT_DUPLICATE_SUBMISSION":
      return { label: "Duplicate", tone: "duplicate" };
    case "IDENTITY_CONFLICT_REVIEW":
      return { label: "Çakışma", tone: "conflict" };
    case "ROW_ERROR":
      return { label: "Hata", tone: "error" };
  }
}

function formatBytes(value: number | null) {
  if (value === null) return "—";
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Dosya önizlemesi hazırlanamadı.";
}

export function ImportsPage() {
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function chooseAndPreview() {
    setError(null);

    const selection = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Lead dosyaları", extensions: ["xlsx", "csv"] }],
    });

    const path = Array.isArray(selection) ? selection[0] : selection;
    if (!path) return;

    setSelectedPath(path);
    setLoading(true);

    try {
      const nextPreview = await invoke<ImportPreview>("preview_import", { path });
      setPreview(nextPreview);
    } catch (previewError) {
      setPreview(null);
      setError(commandErrorMessage(previewError));
    } finally {
      setLoading(false);
    }
  }

  return (
    <section className="page-stack import-page">
      <div className="page-heading">
        <div>
          <div className="eyebrow">DATA INGESTION</div>
          <h1>İçe Aktarımlar</h1>
          <p>XLSX veya CSV lead dosyasını önce analiz et; veritabanına yazmadan etkisini gör.</p>
        </div>
        <button className="primary-button" type="button" onClick={chooseAndPreview} disabled={loading}>
          {loading ? "Analiz ediliyor…" : preview ? "Başka Dosya Seç" : "Dosya Seç"}
        </button>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}

      {!preview ? (
        <article className="panel import-start-panel">
          <div className="empty-icon">IA</div>
          <div>
            <h2>Manuel lead dosyası seçin</h2>
            <p>
              `.xlsx` ve `.csv` desteklenir. Önizleme duplicate, repeat, kimlik çakışması ve veri kalite
              uyarılarını DB değişmeden hesaplar.
            </p>
          </div>
        </article>
      ) : (
        <>
          <article className="panel import-source-panel">
            <div>
              <div className="eyebrow">SEÇİLEN KAYNAK</div>
              <h2>{preview.source.fileName}</h2>
              <p className="import-source-meta">
                {preview.source.format} · {formatBytes(preview.source.fileSize)} · {preview.source.columnCount} kolon
                {preview.source.sheetName ? ` · ${preview.source.sheetName}` : ""}
              </p>
            </div>
            <div className="placeholder-pill">Salt okunur önizleme</div>
          </article>

          {preview.source.ignoredAgencyColumns.length > 0 ? (
            <div className="notice">
              Ajans kolonları CRM durumuna aktarılmayacak: {preview.source.ignoredAgencyColumns.join(", ")}.
            </div>
          ) : null}

          <div className="import-summary-grid">
            <SummaryCard label="Toplam Satır" value={preview.summary.totalRows} />
            <SummaryCard label="İçe Aktarılabilir" value={preview.summary.importableSubmissions} />
            <SummaryCard label="Yeni" value={preview.summary.newContacts} />
            <SummaryCard label="Repeat" value={preview.summary.repeatSubmissions} />
            <SummaryCard label="Duplicate" value={preview.summary.exactDuplicates} />
            <SummaryCard label="Çakışma" value={preview.summary.identityConflicts} />
            <SummaryCard label="Uyarı" value={preview.summary.warningCount} />
            <SummaryCard label="Hata" value={preview.summary.rowErrors} />
          </div>

          <article className="panel import-preview-panel">
            <div className="panel-heading">
              <div>
                <h2>Satır Önizlemesi</h2>
                <p>İlk 100 satır gösteriliyor. Bu aşamada hiçbir lead veritabanına yazılmaz.</p>
              </div>
              {selectedPath ? <span className="import-path" title={selectedPath}>{selectedPath}</span> : null}
            </div>

            <div className="import-table-wrap">
              <table className="import-table">
                <thead>
                  <tr>
                    <th>Satır</th>
                    <th>Lead</th>
                    <th>Ülke</th>
                    <th>Ürün İlgisi</th>
                    <th>Karar</th>
                    <th>Uyarılar</th>
                  </tr>
                </thead>
                <tbody>
                  {preview.rows.slice(0, 100).map((row) => {
                    const decision = decisionLabel(row.decision);
                    return (
                      <tr key={`${row.rowNumber}-${row.normalized.externalLeadId}`}>
                        <td className="import-row-number">{row.rowNumber}</td>
                        <td>
                          <div className="import-lead-name">{row.fullName || "İsimsiz lead"}</div>
                          <div className="import-lead-contact">{row.rawEmail || row.rawPhone || "İletişim bilgisi yok"}</div>
                        </td>
                        <td>{row.normalized.countryCode ?? row.rawCountry ?? "—"}</td>
                        <td>
                          <div className="import-chip-list">
                            {row.normalized.productInterests.length > 0 ? (
                              row.normalized.productInterests.map((product) => (
                                <span className="import-chip" key={product}>{productLabels[product]}</span>
                              ))
                            ) : (
                              <span className="import-muted">—</span>
                            )}
                          </div>
                        </td>
                        <td>
                          <span className={`decision-badge decision-${decision.tone}`}>{decision.label}</span>
                        </td>
                        <td>
                          {row.normalized.warnings.length > 0 ? (
                            <div className="import-warning-list">
                              {row.normalized.warnings.map((warning) => (
                                <span key={warning}>{warningLabels[warning]}</span>
                              ))}
                            </div>
                          ) : (
                            <span className="import-muted">—</span>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </article>
        </>
      )}
    </section>
  );
}

function SummaryCard({ label, value }: { label: string; value: number }) {
  return (
    <article className="kpi-card">
      <div className="kpi-label">{label}</div>
      <div className="kpi-value">{value}</div>
    </article>
  );
}
