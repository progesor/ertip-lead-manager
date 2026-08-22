import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import "./lead-detail.css";
import type {
  CommandError,
  DataQualityIssueType,
  LeadDetailActivity,
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

const activityLabels: Record<string, string> = {
  LEAD_CREATED: "Lead oluşturuldu",
  SUBMISSION_IMPORTED: "Submission içe aktarıldı",
  STATUS_CHANGED: "Durum değiştirildi",
  NOTE_CREATED: "Not eklendi",
  NOTE_UPDATED: "Not güncellendi",
  NOTE_DELETED: "Not silindi",
  PRODUCT_INTEREST_CHANGED: "Ürün ilgisi düzeltildi",
};

const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

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

function formatPlatform(platform: string | null) {
  if (!platform) return "—";
  const value = platform.trim().toLowerCase();
  if (value === "facebook") return "Facebook";
  if (value === "instagram") return "Instagram";
  if (value === "messenger") return "Messenger";
  return value.replaceAll("_", " ");
}

function prettyJson(value: string) {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "CRM işlemi tamamlanamadı.";
}

function activityDetail(activity: LeadDetailActivity) {
  try {
    const payload = JSON.parse(activity.payloadJson) as {
      fromStatus?: LeadStatus;
      toStatus?: LeadStatus;
      productCode?: ProductCode;
      included?: boolean;
    };

    if (activity.activityType === "STATUS_CHANGED" && payload.fromStatus && payload.toStatus) {
      return `${statusLabels[payload.fromStatus] ?? payload.fromStatus} → ${statusLabels[payload.toStatus] ?? payload.toStatus}`;
    }

    if (activity.activityType === "PRODUCT_INTEREST_CHANGED" && payload.productCode) {
      const label = productLabels[payload.productCode] ?? payload.productCode;
      return `${label} · ${payload.included ? "eklendi" : "kaldırıldı"}`;
    }
  } catch {
    return null;
  }

  return null;
}

export function LeadDetailPage() {
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
      setNotice(changed ? `Durum ${statusLabels[statusDraft]} olarak güncellendi.` : "Durum zaten günceldi.");
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
    const accepted = await confirm("Bu not silinecek. İşlem activity geçmişinde kayıtlı kalacak. Devam edilsin mi?", {
      title: "Notu Sil",
      kind: "warning",
      okLabel: "Sil",
      cancelLabel: "Vazgeç",
    });
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
        <article className="panel lead-detail-loading">Lead detayı yükleniyor…</article>
      </section>
    );
  }

  if (error && !detail) {
    return (
      <section className="page-stack lead-detail-page">
        <button type="button" className="lead-back-button" onClick={() => navigate("/leads")}>← Leadlere Dön</button>
        <div className="import-error" role="alert">{error}</div>
      </section>
    );
  }

  if (!detail) return null;

  const { contact } = detail;
  const openIssues = detail.qualityIssues.filter((issue) => issue.status === "OPEN");
  const overrideByProduct = new Map(contact.productOverrides.map((override) => [override.productCode, override]));

  return (
    <section className="page-stack lead-detail-page">
      <div className="lead-detail-topbar">
        <button type="button" className="lead-back-button" onClick={() => navigate("/leads")}>← Leadlere Dön</button>
        <span className="lead-detail-id" title={contact.id}>{contact.id}</span>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}
      {notice ? <div className="import-success" role="status">{notice}</div> : null}

      <article className="panel lead-detail-hero">
        <div className="lead-detail-identity">
          <div className="eyebrow">LEAD DETAIL</div>
          <div className="lead-detail-name-row">
            <h1>{contact.displayName}</h1>
            <span className={`lead-status lead-status-${contact.status.toLowerCase()}`}>
              {statusLabels[contact.status]}
            </span>
            {contact.submissionCount > 1 ? <span className="lead-repeat-badge">Repeat ×{contact.submissionCount}</span> : null}
          </div>
          <div className="lead-detail-contact-lines">
            <span>{contact.primaryEmail ?? "E-posta yok"}</span>
            <span>{contact.primaryPhone ?? "Telefon yok"}</span>
            <span>{formatCountry(contact.countryCode)}</span>
          </div>
        </div>

        <div className="lead-detail-crm-actions">
          <div className="lead-status-editor">
            <label htmlFor="lead-status-select">CRM Durumu</label>
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
                {mutating === "status" ? "Kaydediliyor…" : "Durumu Kaydet"}
              </button>
            </div>
          </div>
          <div className="lead-detail-metrics">
            <div><strong>{contact.submissionCount}</strong><span>submission</span></div>
            <div><strong>{openIssues.length}</strong><span>açık uyarı</span></div>
            <div><strong>{formatDate(contact.latestSubmissionAt)}</strong><span>son lead</span></div>
          </div>
        </div>
      </article>

      <div className="lead-detail-grid">
        <article className="panel lead-detail-products">
          <div className="panel-heading">
            <div>
              <h2>Ürün İlgileri</h2>
              <p>Kaynak seçimleri değişmeden kalır; manuel düzeltmeler etkin görünümün üzerine uygulanır.</p>
            </div>
            {contact.productOverrides.length > 0 ? <span className="placeholder-pill">Manuel düzeltme var</span> : null}
          </div>

          <div className="lead-product-list lead-effective-products">
            {contact.productInterests.length > 0 ? (
              contact.productInterests.map((product) => (
                <span className="lead-product-chip" key={product}>{productLabels[product] ?? product}</span>
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
                <label className={`lead-product-option ${included ? "is-included" : ""}`} key={productCode}>
                  <input
                    type="checkbox"
                    checked={included}
                    disabled={mutating !== null}
                    onChange={(event) => void setProductInterest(productCode, event.target.checked)}
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
              <p>İçe aktarım sırasında üretilen uyarılar.</p>
            </div>
            <span className="placeholder-pill">{openIssues.length} açık</span>
          </div>
          {detail.qualityIssues.length > 0 ? (
            <div className="lead-detail-warning-list">
              {detail.qualityIssues.map((issue) => (
                <div className={`lead-detail-warning ${issue.status !== "OPEN" ? "is-closed" : ""}`} key={issue.id}>
                  <strong>{warningLabels[issue.issueType] ?? issue.issueType}</strong>
                  <span>{issue.status} · {formatDate(issue.createdAt)}</span>
                </div>
              ))}
            </div>
          ) : (
            <div className="lead-detail-empty">Veri kalitesi uyarısı yok.</div>
          )}
        </article>
      </div>

      <article className="panel lead-detail-notes-panel">
        <div className="panel-heading">
          <div>
            <h2>CRM Notları</h2>
            <p>İçe aktarımdan bağımsız, kullanıcı tarafından yönetilen satış notları.</p>
          </div>
          <span className="placeholder-pill">{detail.notes.length} not</span>
        </div>

        <div className="lead-note-compose">
          <textarea
            value={noteDraft}
            maxLength={5000}
            rows={3}
            onChange={(event) => setNoteDraft(event.target.value)}
            placeholder="Görüşme, talep, fiyat beklentisi veya sonraki adımla ilgili not ekleyin…"
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
                  <div className="lead-note-footer">
                    <span>
                      {formatDate(note.createdAt)}
                      {note.updatedAt !== note.createdAt ? ` · Güncellendi ${formatDate(note.updatedAt)}` : ""}
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
                            {mutating === `note-update:${note.id}` ? "Kaydediliyor…" : "Kaydet"}
                          </button>
                        </>
                      ) : (
                        <>
                          <button type="button" onClick={() => beginEditNote(note)} disabled={mutating !== null}>Düzenle</button>
                          <button type="button" className="is-danger" onClick={() => void removeNote(note)} disabled={mutating !== null}>Sil</button>
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

      <article className="panel lead-detail-submissions-panel">
        <div className="panel-heading">
          <div>
            <h2>Submission Geçmişi</h2>
            <p>Aynı kişiye bağlanan tüm Meta lead kayıtları. En yeni kayıt üstte.</p>
          </div>
          <span className="placeholder-pill">{detail.submissions.length} kayıt</span>
        </div>

        <div className="lead-detail-submission-list">
          {detail.submissions.map((submission, index) => (
            <article className="lead-submission-card" key={submission.id}>
              <div className="lead-submission-heading">
                <div>
                  <div className="lead-submission-index">Submission #{detail.submissions.length - index}</div>
                  <strong>{formatDate(submission.sourceCreatedAtUtc)}</strong>
                  <span className="lead-submission-id">{submission.externalLeadId}</span>
                </div>
                <div className="lead-submission-heading-chips">
                  {submission.platform ? <span className={`lead-platform-chip lead-platform-${submission.platform.toLowerCase()}`}>{formatPlatform(submission.platform)}</span> : null}
                  {submission.isOrganic === true ? <span className="lead-source-chip">Organic</span> : null}
                </div>
              </div>

              <div className="lead-submission-products">
                {submission.productInterests.map((product) => (
                  <span className="lead-product-chip" key={product}>{productLabels[product] ?? product}</span>
                ))}
              </div>

              <div className="lead-submission-grid">
                <SourceValue label="Kampanya" value={submission.campaignName} secondary={submission.campaignId} />
                <SourceValue label="Ad Set" value={submission.adsetName} secondary={submission.adsetId} />
                <SourceValue label="Reklam" value={submission.adName} secondary={submission.adId} />
                <SourceValue label="Form" value={submission.formName} secondary={submission.formId} />
                <SourceValue label="Ham isim" value={submission.rawFullName} />
                <SourceValue label="Ham e-posta" value={submission.rawEmail} />
                <SourceValue label="Ham telefon" value={submission.rawPhone} />
                <SourceValue label="Ham ülke" value={submission.rawCountry} />
                <SourceValue label="Kaynak lead_status" value={submission.rawLeadStatus} />
                <SourceValue label="Prosedür cevabı" value={submission.rawProcedureAnswer} />
                <SourceValue label="Ürün cevabı" value={submission.rawProductAnswer} wide />
                <SourceValue label="Kaynak tarih" value={submission.sourceCreatedAtRaw} wide />
              </div>

              <details className="lead-raw-payload">
                <summary>Ham kaynak payload'ını göster</summary>
                <pre>{prettyJson(submission.rawPayloadJson)}</pre>
              </details>
            </article>
          ))}
        </div>
      </article>

      <article className="panel lead-detail-activity-panel">
        <div className="panel-heading">
          <div>
            <h2>Aktivite</h2>
            <p>Sistem ve CRM hareketleri.</p>
          </div>
        </div>
        {detail.activities.length > 0 ? (
          <div className="lead-activity-list">
            {detail.activities.map((activity) => {
              const detailText = activityDetail(activity);
              return (
                <div className="lead-activity-row" key={activity.id}>
                  <span className="lead-activity-dot" />
                  <div>
                    <strong>{activityLabels[activity.activityType] ?? activity.activityType}</strong>
                    {detailText ? <span className="lead-activity-detail">{detailText}</span> : null}
                    <span>{formatDate(activity.occurredAt)}</span>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="lead-detail-empty">Henüz aktivite yok.</div>
        )}
      </article>
    </section>
  );
}

function SourceValue({
  label,
  value,
  secondary,
  wide,
}: {
  label: string;
  value: string | null;
  secondary?: string | null;
  wide?: boolean;
}) {
  return (
    <div className={`lead-source-value ${wide ? "is-wide" : ""}`}>
      <span>{label}</span>
      <strong>{value || "—"}</strong>
      {secondary ? <small>{secondary}</small> : null}
    </div>
  );
}
