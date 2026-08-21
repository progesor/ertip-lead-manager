import { invoke } from "@tauri-apps/api/core";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import "./imports.css";
import type {
  CommandError,
  CommitImportResult,
  IdentityDecision,
  ImportHistoryItem,
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

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function commandErrorMessage(error: unknown, fallback: string) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return fallback;
}

export function ImportsPage() {
  const [preview, setPreview] = useState<ImportPreview | null>(null);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [history, setHistory] = useState<ImportHistoryItem[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [committing, setCommitting] = useState(false);
  const [historyLoading, setHistoryLoading] = useState(false);

  const loadHistory = useCallback(async () => {
    setHistoryLoading(true);
    try {
      const rows = await invoke<ImportHistoryItem[]>("list_import_history");
      setHistory(rows);
    } catch (historyError) {
      setError((current) => current ?? commandErrorMessage(historyError, "İçe aktarım geçmişi okunamadı."));
    } finally {
      setHistoryLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadHistory();
  }, [loadHistory]);

  async function previewPath(path: string) {
    const nextPreview = await invoke<ImportPreview>("preview_import", { path });
    setPreview(nextPreview);
    return nextPreview;
  }

  async function chooseAndPreview() {
    setError(null);
    setSuccess(null);

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
      await previewPath(path);
    } catch (previewError) {
      setPreview(null);
      setError(commandErrorMessage(previewError, "Dosya önizlemesi hazırlanamadı."));
    } finally {
      setLoading(false);
    }
  }

  const canCommit = Boolean(
    preview &&
      selectedPath &&
      preview.summary.importableSubmissions > 0 &&
      preview.summary.identityConflicts === 0 &&
      preview.summary.rowErrors === 0,
  );

  async function commitSelectedFile() {
    if (!preview || !selectedPath || !canCommit) return;

    const accepted = await confirm(
      `${preview.summary.importableSubmissions} submission veritabanına yazılacak. ` +
        `${preview.summary.exactDuplicates} duplicate atlanacak. Devam edilsin mi?`,
      {
        title: "İçe Aktarımı Onayla",
        kind: "warning",
        okLabel: "İçe Aktar",
        cancelLabel: "Vazgeç",
      },
    );

    if (!accepted) return;

    setCommitting(true);
    setError(null);
    setSuccess(null);

    try {
      const result = await invoke<CommitImportResult>("commit_import", { path: selectedPath });
      setSuccess(
        `${result.summary.importableSubmissions} submission başarıyla içe aktarıldı. ` +
          `Batch: ${result.batchId.slice(0, 8)}…`,
      );

      await loadHistory();
      await previewPath(selectedPath);
    } catch (commitError) {
      setError(commandErrorMessage(commitError, "İçe aktarım tamamlanamadı."));
    } finally {
      setCommitting(false);
    }
  }

  return (
    <section className="page-stack import-page">
      <div className="page-heading">
        <div>
          <div className="eyebrow">DATA INGESTION</div>
          <h1>İçe Aktarımlar</h1>
          <p>XLSX veya CSV lead dosyasını analiz et, sonucu doğrula ve tek işlemde yerel CRM'e aktar.</p>
        </div>
        <button className="primary-button" type="button" onClick={chooseAndPreview} disabled={loading || committing}>
          {loading ? "Analiz ediliyor…" : preview ? "Başka Dosya Seç" : "Dosya Seç"}
        </button>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}
      {success ? <div className="import-success" role="status">{success}</div> : null}

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
            <div className="import-source-actions">
              <div className="placeholder-pill">
                {preview.summary.importableSubmissions > 0 ? "Önizleme hazır" : "Yeni kayıt yok"}
              </div>
              <button
                className="primary-button import-commit-button"
                type="button"
                onClick={commitSelectedFile}
                disabled={!canCommit || committing || loading}
                title={
                  preview.summary.identityConflicts > 0 || preview.summary.rowErrors > 0
                    ? "Çakışma veya hata bulunan dosya içe aktarılamaz."
                    : undefined
                }
              >
                {committing
                  ? "İçe aktarılıyor…"
                  : `${preview.summary.importableSubmissions} Kaydı İçe Aktar`}
              </button>
            </div>
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
                <p>
                  İlk 100 satır gösteriliyor. İçe aktarım sırasında dosya ve veritabanı tekrar doğrulanır.
                </p>
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

      <ImportHistory history={history} loading={historyLoading} />
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

function ImportHistory({ history, loading }: { history: ImportHistoryItem[]; loading: boolean }) {
  return (
    <article className="panel import-history-panel">
      <div className="panel-heading">
        <div>
          <h2>İçe Aktarım Geçmişi</h2>
          <p>Son 50 import batch'i. Duplicate sayıları dahil olmak üzere işlem özeti saklanır.</p>
        </div>
        {loading ? <span className="placeholder-pill">Yükleniyor…</span> : null}
      </div>

      {history.length === 0 ? (
        <div className="import-history-empty">Henüz tamamlanmış bir içe aktarım yok.</div>
      ) : (
        <div className="import-history-table-wrap">
          <table className="import-table import-history-table">
            <thead>
              <tr>
                <th>Tarih</th>
                <th>Dosya</th>
                <th>Format</th>
                <th>Toplam</th>
                <th>Eklenen</th>
                <th>Repeat</th>
                <th>Duplicate</th>
                <th>Uyarı</th>
                <th>Durum</th>
              </tr>
            </thead>
            <tbody>
              {history.map((item) => (
                <tr key={item.batchId}>
                  <td>{formatDate(item.completedAt)}</td>
                  <td>
                    <div className="import-history-file">{item.fileName}</div>
                    <div className="import-history-sheet">{item.sheetName}</div>
                  </td>
                  <td>{item.format}</td>
                  <td>{item.totalRows}</td>
                  <td>{item.importedSubmissions}</td>
                  <td>{item.repeatSubmissions}</td>
                  <td>{item.exactDuplicates}</td>
                  <td>{item.warningCount}</td>
                  <td>
                    <span className="decision-badge decision-new">{item.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </article>
  );
}
