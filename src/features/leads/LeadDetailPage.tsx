import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import type { ReactNode } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { LeadHistoryPanel } from "./LeadHistoryPanel";
import "./lead-detail.css";
import type {
  CommandError,
  DataQualityIssueType,
  LeadDetailNote,
  LeadDetailResponse,
  LeadStatus,
  ProductCode,
} from "./types";

const statusLabels: Record<LeadStatus, string> = {
  NEW: "Yeni",
  CONTACTED: "İletişime Geçildi",
  REPLIED: "Yanıtladı",
  QUALIFIED: "Nitelikli",
  QUOTE_SENT: "Teklif Gönderildi",
  WON: "Kazanıldı",
  LOST: "Kaybedildi",
  INVALID: "Geçersiz",
};

const statusOptions = Object.entries(statusLabels) as Array<[LeadStatus, string]>;

const productLabels: Record<ProductCode, string> = {
  FUE_MICROMOTOR_SYSTEMS: "FUE Micromotor",
  LONG_HAIR_FUE_SOLUTIONS: "Long Hair FUE",
  FUE_PUNCHES: "FUE Punches",
  IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS: "Implanter / Forceps",
  MEDICAL_CHAIRS_CLINIC_FURNITURE: "Medikal Mobilya",
  OTHER_GENERAL_INFORMATION: "Diğer / Genel Bilgi",
  UNKNOWN: "Bilinmiyor / Legacy",
};

const productOptions = Object.entries(productLabels) as Array<[ProductCode, string]>;

const warningLabels: Record<DataQualityIssueType, string> = {
  INVALID_EMAIL: "Geçersiz e-posta",
  INVALID_PHONE: "Geçersiz telefon",
  INVALID_COUNTRY: "Geçersiz ülke kodu",
  INVALID_TIMESTAMP: "Geçersiz lead tarihi",
  MISSING_CONTACT_METHOD: "İletişim bilgisi yok",
  UNKNOWN_PRODUCT: "Ürün cevabı eşleşmedi",
};

const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

interface LeadDetailPageProps {
  backTo?: string;
  backLabel?: string;
  backState?: unknown;
  followUpPanel?: ReactNode;
}

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function formatCountry(countryCode: string | null) {
  if (!countryCode) return "—";
  const code = countryCode.trim().toUpperCase();
  if (code.length !== 2) return code;
  try {
    const name = regionNames.of(code);
    return name && name !== code ? `${code} · ${name}` : code;
  } catch {
    return code;
  }
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "CRM işlemi tamamlanamadı.";
}

export function LeadDetailPage({
  backTo = "/leads",
  backLabel = "Leadlere Dön",
  backState,
  followUpPanel,
}: LeadDetailPageProps) {
  const { leadId } = useParams();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<LeadDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [statusDraft, setStatusDraft] = useState<LeadStatus>("NEW");
  const [noteDraft, setNoteDraft] = useState("");
  const [editingNoteId, setEditingNoteId] = useState<string | null>(null);
  const [editingNoteBody, setEditingNoteBody] = useState("");
  const [mutating, setMutating] = useState<string | null>(null);

  const goBack = useCallback(() => {
    navigate(backTo, { state: backState });
  }, [backState, backTo, navigate]);

  const fetchDetail = useCallback(async () => {
    if (!leadId) throw new Error("Lead kimliği bulunamadı.");
    const response = await invoke<LeadDetailResponse | null>("get_lead_detail", { contactId: leadId });
    if (!response) throw new Error("Lead kaydı bulunamadı.");
    setDetail(response);
    setStatusDraft(response.contact.status);
    return response;
  }, [leadId]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    void fetchDetail()
      .catch((loadError) => {
        setError(commandErrorMessage(loadError));
        setDetail(null);
      })
      .finally(() => setLoading(false));
  }, [fetchDetail]);

  async function refreshDetail() {
    try {
      await fetchDetail();
    } catch (refreshError) {
      setError(commandErrorMessage(refreshError));
    }
  }

  async function saveStatus() {
    if (!detail || statusDraft === detail.contact.status) return;
    setMutating("status");
    setError(null);
    setNotice(null);
    try {
      const changed = await invoke<boolean>("change_lead_status", {
        contactId: detail.contact.id,
        newStatus: statusDraft,
      });
      setNotice(
        changed
          ? `Durum ${statusLabels[statusDraft]} olarak güncellendi.`
          : "Durum zaten günceldi.",
      );
      await refreshDetail();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function setProductInterest(productCode: ProductCode, included: boolean) {
    if (!detail) return;
    setMutating(`product:${productCode}`);
    setError(null);
    setNotice(null);
    try {
      const changed = await invoke<boolean>("set_lead_product_interest", {
        contactId: detail.contact.id,
        productCode,
        included,
      });
      setNotice(
        changed
          ? `${productLabels[productCode]} ${included ? "etkin ürün ilgilerine eklendi" : "etkin ürün ilgilerinden kaldırıldı"}.`
          : "Ürün ilgisi zaten bu durumdaydı.",
      );
      await refreshDetail();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function addNote() {
    if (!detail || !noteDraft.trim()) return;
    setMutating("note-create");
    setError(null);
    setNotice(null);
    try {
      await invoke<string>("create_lead_note", {
        contactId: detail.contact.id,
        body: noteDraft,
      });
      setNoteDraft("");
      setNotice("Not eklendi.");
      await refreshDetail();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  function beginEditNote(note: LeadDetailNote) {
    setEditingNoteId(note.id);
    setEditingNoteBody(note.body);
    setNotice(null);
    setError(null);
  }

  async function saveEditedNote() {
    if (!detail || !editingNoteId || !editingNoteBody.trim()) return;
    setMutating(`note-update:${editingNoteId}`);
    setError(null);
    setNotice(null);
    try {
      await invoke<boolean>("update_lead_note", {
        contactId: detail.contact.id,
        noteId: editingNoteId,
        body: editingNoteBody,
      });
      setEditingNoteId(null);
      setEditingNoteBody("");
      setNotice("Not güncellendi.");
      await refreshDetail();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function removeNote(note: LeadDetailNote) {
    if (!detail) return;
    const accepted = await confirm(
      "Bu not silinecek. İşlem activity geçmişinde kayıtlı kalacak. Devam edilsin mi?",
      {
        title: "Notu Sil",
        kind: "warning",
        okLabel: "Sil",
        cancelLabel: "Vazgeç",
      },
    );
    if (!accepted) return;

    setMutating(`note-delete:${note.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke("delete_lead_note", {
        contactId: detail.contact.id,
        noteId: note.id,
      });
      if (editingNoteId === note.id) {
        setEditingNoteId(null);
        setEditingNoteBody("");
      }
      setNotice("Not silindi.");
      await refreshDetail();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  if (loading) {
    return (
      <section className="page-stack lead-detail-page">
        <button type="button" className="lead-back-button" onClick={goBack}>
          ← {backLabel}
        </button>
        <article className="panel lead-detail-loading">Lead detayı yükleniyor…</article>
      </section>
    );
  }

  if (error && !detail) {
    return (
      <section className="page-stack lead-detail-page">
        <button type="button" className="lead-back-button" onClick={goBack}>
          ← {backLabel}
        </button>
        <div className="import-error" role="alert">{error}</div>
      </section>
    );
  }

  if (!detail) return null;

  const { contact } = detail;
  const openIssues = detail.qualityIssues.filter((issue) => issue.status === "OPEN");
  const overrideByProduct = new Map(
    contact.productOverrides.map((override) => [override.productCode, override]),
  );

  return (
    <section className="page-stack lead-detail-page">
      <div className="lead-detail-topbar">
        <button type="button" className="lead-back-button" onClick={goBack}>
          ← {backLabel}
        </button>
        <span className="lead-detail-id" title={contact.id}>{contact.id}</span>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}
      {notice ? <div className="import-success" role="status">{notice}</div> : null}

      <article className="panel lead-detail-hero lead-detail-hero-production">
        <div className="lead-detail-identity">
          <div className="eyebrow">MÜŞTERİ / LEAD</div>
          <div className="lead-detail-name-row">
            <h1>{contact.displayName}</h1>
            <span className={`lead-status lead-status-${contact.status.toLowerCase()}`}>
              {statusLabels[contact.status]}
            </span>
            {contact.submissionCount > 1 ? (
              <span className="lead-repeat-badge">Repeat ×{contact.submissionCount}</span>
            ) : null}
          </div>
          <div className="lead-detail-contact-lines">
            <span>{contact.primaryEmail ?? "E-posta yok"}</span>
            <span>{contact.primaryPhone ?? "Telefon yok"}</span>
            <span>{formatCountry(contact.countryCode)}</span>
          </div>
        </div>

        <div className="lead-detail-crm-actions">
          <div className="lead-status-editor">
            <label htmlFor="lead-status-select">Satış Aşaması</label>
            <div>
              <select
                id="lead-status-select"
                value={statusDraft}
                onChange={(event) => setStatusDraft(event.target.value as LeadStatus)}
                disabled={mutating !== null}
              >
                {statusOptions.map(([value, label]) => (
                  <option key={value} value={value}>{label}</option>
                ))}
              </select>
              <button
                type="button"
                onClick={saveStatus}
                disabled={mutating !== null || statusDraft === contact.status}
              >
                {mutating === "status" ? "Kaydediliyor…" : "Güncelle"}
              </button>
            </div>
          </div>
          <div className="lead-detail-metrics lead-detail-metrics-production">
            <div><strong>{contact.submissionCount}</strong><span>form kaydı</span></div>
            <div><strong>{openIssues.length}</strong><span>açık uyarı</span></div>
            <div><strong>{formatDate(contact.latestSubmissionAt)}</strong><span>son lead</span></div>
          </div>
        </div>
      </article>

      <div className="lead-detail-production-grid">
        <div className="lead-detail-primary-column">
          <div className="lead-detail-grid lead-detail-operational-grid">
            <article className="panel lead-detail-products">
              <div className="panel-heading">
                <div>
                  <h2>Ürün İlgileri</h2>
                  <p>CRM'de kullanılacak etkin ürün ilgilerini düzenleyin.</p>
                </div>
                {contact.productOverrides.length > 0 ? (
                  <span className="placeholder-pill">Manuel düzeltme</span>
                ) : null}
              </div>

              <div className="lead-product-list lead-effective-products">
                {contact.productInterests.length > 0 ? (
                  contact.productInterests.map((product) => (
                    <span className="lead-product-chip" key={product}>
                      {productLabels[product] ?? product}
                    </span>
                  ))
                ) : (
                  <span className="lead-muted">Etkin ürün ilgisi yok.</span>
                )}
              </div>

              <div className="lead-product-editor">
                {productOptions.map(([productCode, label]) => {
                  const included = contact.productInterests.includes(productCode);
                  const automatic = contact.automaticProductInterests.includes(productCode);
                  const override = overrideByProduct.get(productCode);
                  const busy = mutating === `product:${productCode}`;
                  let sourceLabel = automatic ? "Meta / Import" : "Kaynakta yok";
                  let sourceTone = automatic ? "source" : "none";

                  if (override?.action === "ADD") {
                    sourceLabel = "Manuel eklendi";
                    sourceTone = "added";
                  } else if (override?.action === "REMOVE") {
                    sourceLabel = "Manuel kaldırıldı";
                    sourceTone = "removed";
                  }

                  return (
                    <label
                      className={`lead-product-option ${included ? "is-included" : ""}`}
                      key={productCode}
                    >
                      <input
                        type="checkbox"
                        checked={included}
                        disabled={mutating !== null}
                        onChange={(event) =>
                          void setProductInterest(productCode, event.target.checked)
                        }
                      />
                      <span className="lead-product-option-label">{label}</span>
                      <span className={`lead-product-source lead-product-source-${sourceTone}`}>
                        {busy ? "Kaydediliyor…" : sourceLabel}
                      </span>
                    </label>
                  );
                })}
              </div>
            </article>

            <article className="panel lead-detail-warnings">
              <div className="panel-heading">
                <div>
                  <h2>Veri Kalitesi</h2>
                  <p>Operasyonu etkileyebilecek açık veri sorunları.</p>
                </div>
                <span className="placeholder-pill">{openIssues.length} açık</span>
              </div>
              {detail.qualityIssues.length > 0 ? (
                <div className="lead-detail-warning-list">
                  {detail.qualityIssues.map((issue) => (
                    <div
                      className={`lead-detail-warning ${issue.status !== "OPEN" ? "is-closed" : ""}`}
                      key={issue.id}
                    >
                      <strong>{warningLabels[issue.issueType] ?? issue.issueType}</strong>
                      <span>{issue.status} · {formatDate(issue.createdAt)}</span>
                    </div>
                  ))}
                </div>
              ) : (
                <div className="lead-detail-empty lead-detail-empty-positive">
                  Veri kalitesi sorunu yok.
                </div>
              )}
            </article>
          </div>

          {followUpPanel}

          <article className="panel lead-detail-notes-panel">
            <div className="panel-heading">
              <div>
                <h2>CRM Notları</h2>
                <p>Görüşme özeti, ihtiyaç, fiyat beklentisi ve sonraki adımlar.</p>
              </div>
              <span className="placeholder-pill">{detail.notes.length} not</span>
            </div>

            <div className="lead-note-compose">
              <textarea
                value={noteDraft}
                maxLength={5000}
                rows={3}
                onChange={(event) => setNoteDraft(event.target.value)}
                placeholder="Görüşmeyle ilgili kısa ve operasyonel bir not ekleyin…"
                disabled={mutating !== null}
              />
              <div>
                <span>{noteDraft.length} / 5000</span>
                <button
                  type="button"
                  onClick={addNote}
                  disabled={mutating !== null || !noteDraft.trim()}
                >
                  {mutating === "note-create" ? "Ekleniyor…" : "Not Ekle"}
                </button>
              </div>
            </div>

            {detail.notes.length > 0 ? (
              <div className="lead-note-list">
                {detail.notes.map((note) => {
                  const editing = editingNoteId === note.id;
                  return (
                    <article className="lead-note-card" key={note.id}>
                      {editing ? (
                        <textarea
                          value={editingNoteBody}
                          maxLength={5000}
                          rows={4}
                          onChange={(event) => setEditingNoteBody(event.target.value)}
                          disabled={mutating !== null}
                        />
                      ) : (
                        <p>{note.body}</p>
                      )}
                      <div className="lead-note-meta">
                        <span>
                          {formatDate(note.createdAt)}
                          {note.updatedAt !== note.createdAt
                            ? ` · Güncellendi ${formatDate(note.updatedAt)}`
                            : ""}
                        </span>
                        <div>
                          {editing ? (
                            <>
                              <button
                                type="button"
                                onClick={() => {
                                  setEditingNoteId(null);
                                  setEditingNoteBody("");
                                }}
                                disabled={mutating !== null}
                              >
                                Vazgeç
                              </button>
                              <button
                                type="button"
                                className="is-primary"
                                onClick={saveEditedNote}
                                disabled={mutating !== null || !editingNoteBody.trim()}
                              >
                                {mutating === `note-update:${note.id}`
                                  ? "Kaydediliyor…"
                                  : "Kaydet"}
                              </button>
                            </>
                          ) : (
                            <>
                              <button
                                type="button"
                                onClick={() => beginEditNote(note)}
                                disabled={mutating !== null}
                              >
                                Düzenle
                              </button>
                              <button
                                type="button"
                                className="is-danger"
                                onClick={() => void removeNote(note)}
                                disabled={mutating !== null}
                              >
                                Sil
                              </button>
                            </>
                          )}
                        </div>
                      </div>
                    </article>
                  );
                })}
              </div>
            ) : (
              <div className="lead-detail-empty">Henüz CRM notu yok.</div>
            )}
          </article>
        </div>

        <LeadHistoryPanel detail={detail} />
      </div>
    </section>
  );
}
