import { useState } from "react";
import type {
  LeadDetailActivity,
  LeadDetailResponse,
  LeadStatus,
  ProductCode,
} from "./types";
import "./lead-history-panel.css";

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

const productLabels: Record<ProductCode, string> = {
  FUE_MICROMOTOR_SYSTEMS: "FUE Micromotor",
  LONG_HAIR_FUE_SOLUTIONS: "Long Hair FUE",
  FUE_PUNCHES: "FUE Punches",
  IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS: "Implanter / Forceps",
  MEDICAL_CHAIRS_CLINIC_FURNITURE: "Medikal Mobilya",
  OTHER_GENERAL_INFORMATION: "Diğer / Genel Bilgi",
  UNKNOWN: "Bilinmiyor / Legacy",
};

const activityLabels: Record<string, string> = {
  LEAD_CREATED: "Lead oluşturuldu",
  SUBMISSION_IMPORTED: "Submission içe aktarıldı",
  STATUS_CHANGED: "Durum değiştirildi",
  NOTE_CREATED: "Not eklendi",
  NOTE_UPDATED: "Not güncellendi",
  NOTE_DELETED: "Not silindi",
  PRODUCT_INTEREST_CHANGED: "Ürün ilgisi düzeltildi",
  FOLLOW_UP_CREATED: "Takip planlandı",
  FOLLOW_UP_RESCHEDULED: "Takip yeniden zamanlandı",
  FOLLOW_UP_COMPLETED: "Takip tamamlandı",
  FOLLOW_UP_CANCELLED: "Takip iptal edildi",
};

type HistoryTab = "activity" | "submissions" | "source";

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function formatPlatform(platform: string | null) {
  if (!platform) return "—";
  const value = platform.trim().toLowerCase();
  if (value === "facebook" || value === "fb") return "Facebook";
  if (value === "instagram" || value === "ig") return "Instagram";
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

function activityDetail(activity: LeadDetailActivity) {
  try {
    const payload = JSON.parse(activity.payloadJson) as {
      fromStatus?: LeadStatus;
      toStatus?: LeadStatus;
      productCode?: ProductCode;
      included?: boolean;
      dueAt?: string;
      previousDueAt?: string;
    };

    if (activity.activityType === "STATUS_CHANGED" && payload.fromStatus && payload.toStatus) {
      return `${statusLabels[payload.fromStatus] ?? payload.fromStatus} → ${statusLabels[payload.toStatus] ?? payload.toStatus}`;
    }

    if (activity.activityType === "PRODUCT_INTEREST_CHANGED" && payload.productCode) {
      const label = productLabels[payload.productCode] ?? payload.productCode;
      return `${label} · ${payload.included ? "eklendi" : "kaldırıldı"}`;
    }

    if (payload.dueAt && activity.activityType.startsWith("FOLLOW_UP_")) {
      return formatDate(payload.dueAt);
    }
  } catch {
    return null;
  }

  return null;
}

export function LeadHistoryPanel({ detail }: { detail: LeadDetailResponse }) {
  const [activeTab, setActiveTab] = useState<HistoryTab>("activity");
  const latestSubmission = detail.submissions[0] ?? null;

  return (
    <aside className="lead-history-panel" aria-label="Lead geçmişi ve kaynak bilgileri">
      <div className="lead-history-header">
        <div>
          <span>ARKA PLAN</span>
          <strong>Geçmiş & Kaynak</strong>
        </div>
        <span className="lead-history-count">{detail.submissions.length} submission</span>
      </div>

      <div className="lead-history-tabs" role="tablist" aria-label="Lead geçmişi sekmeleri">
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "activity"}
          className={activeTab === "activity" ? "is-active" : ""}
          onClick={() => setActiveTab("activity")}
        >
          Aktivite
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "submissions"}
          className={activeTab === "submissions" ? "is-active" : ""}
          onClick={() => setActiveTab("submissions")}
        >
          Submission
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={activeTab === "source"}
          className={activeTab === "source" ? "is-active" : ""}
          onClick={() => setActiveTab("source")}
        >
          Kaynak
        </button>
      </div>

      <div className="lead-history-body">
        {activeTab === "activity" ? (
          detail.activities.length > 0 ? (
            <div className="lead-history-activity-list">
              {detail.activities.map((activity) => {
                const detailText = activityDetail(activity);
                return (
                  <div className="lead-history-activity" key={activity.id}>
                    <span className="lead-history-dot" />
                    <div>
                      <strong>{activityLabels[activity.activityType] ?? activity.activityType}</strong>
                      {detailText ? <em>{detailText}</em> : null}
                      <span>{formatDate(activity.occurredAt)}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="lead-history-empty">Henüz aktivite yok.</div>
          )
        ) : null}

        {activeTab === "submissions" ? (
          detail.submissions.length > 0 ? (
            <div className="lead-history-submission-list">
              {detail.submissions.map((submission, index) => (
                <details className="lead-history-submission" key={submission.id} open={index === 0}>
                  <summary>
                    <div>
                      <strong>#{detail.submissions.length - index} · {formatDate(submission.sourceCreatedAtUtc)}</strong>
                      <span>{formatPlatform(submission.platform)} · {submission.externalLeadId}</span>
                    </div>
                    {submission.productInterests.length > 0 ? (
                      <span>{submission.productInterests.length} ürün</span>
                    ) : null}
                  </summary>

                  <div className="lead-history-submission-content">
                    {submission.productInterests.length > 0 ? (
                      <div className="lead-history-products">
                        {submission.productInterests.map((product) => (
                          <span key={product}>{productLabels[product] ?? product}</span>
                        ))}
                      </div>
                    ) : null}

                    <SourceLine label="Kampanya" value={submission.campaignName} secondary={submission.campaignId} />
                    <SourceLine label="Ad Set" value={submission.adsetName} secondary={submission.adsetId} />
                    <SourceLine label="Reklam" value={submission.adName} secondary={submission.adId} />
                    <SourceLine label="Form" value={submission.formName} secondary={submission.formId} />
                    <SourceLine label="Ham isim" value={submission.rawFullName} />
                    <SourceLine label="Ham e-posta" value={submission.rawEmail} />
                    <SourceLine label="Ham telefon" value={submission.rawPhone} />
                    <SourceLine label="Ham ülke" value={submission.rawCountry} />
                    <SourceLine label="Kaynak lead_status" value={submission.rawLeadStatus} />
                    <SourceLine label="Prosedür cevabı" value={submission.rawProcedureAnswer} />
                    <SourceLine label="Ürün cevabı" value={submission.rawProductAnswer} />
                    <SourceLine label="Kaynak tarih" value={submission.sourceCreatedAtRaw} />

                    <details className="lead-history-raw">
                      <summary>Ham Meta payload</summary>
                      <pre>{prettyJson(submission.rawPayloadJson)}</pre>
                    </details>
                  </div>
                </details>
              ))}
            </div>
          ) : (
            <div className="lead-history-empty">Submission kaydı yok.</div>
          )
        ) : null}

        {activeTab === "source" ? (
          <div className="lead-history-source">
            <SourceLine label="Contact ID" value={detail.contact.id} mono />
            <SourceLine label="Oluşturuldu" value={formatDate(detail.contact.createdAt)} />
            <SourceLine label="Son güncelleme" value={formatDate(detail.contact.updatedAt)} />
            <SourceLine label="Son lead" value={formatDate(detail.contact.latestSubmissionAt)} />
            <SourceLine label="Submission sayısı" value={String(detail.contact.submissionCount)} />
            <SourceLine label="Son platform" value={formatPlatform(latestSubmission?.platform ?? null)} />
            <SourceLine label="Son kampanya" value={latestSubmission?.campaignName ?? null} secondary={latestSubmission?.campaignId} />
            <SourceLine label="Son form" value={latestSubmission?.formName ?? null} secondary={latestSubmission?.formId} />

            <div className="lead-history-source-section">
              <span>Otomatik ürün ilgileri</span>
              <div className="lead-history-products">
                {detail.contact.automaticProductInterests.length > 0 ? (
                  detail.contact.automaticProductInterests.map((product) => (
                    <span key={product}>{productLabels[product] ?? product}</span>
                  ))
                ) : (
                  <small>Kaynak ürün ilgisi yok.</small>
                )}
              </div>
            </div>

            <div className="lead-history-source-section">
              <span>Manuel override</span>
              {detail.contact.productOverrides.length > 0 ? (
                <div className="lead-history-override-list">
                  {detail.contact.productOverrides.map((override, index) => (
                    <div key={`${override.productCode}-${override.createdAt}-${index}`}>
                      <strong>{productLabels[override.productCode] ?? override.productCode}</strong>
                      <span>{override.action === "ADD" ? "Eklendi" : "Kaldırıldı"} · {formatDate(override.createdAt)}</span>
                    </div>
                  ))}
                </div>
              ) : (
                <small>Manuel ürün düzeltmesi yok.</small>
              )}
            </div>
          </div>
        ) : null}
      </div>
    </aside>
  );
}

function SourceLine({
  label,
  value,
  secondary,
  mono,
}: {
  label: string;
  value: string | null;
  secondary?: string | null;
  mono?: boolean;
}) {
  return (
    <div className={`lead-history-source-line ${mono ? "is-mono" : ""}`}>
      <span>{label}</span>
      <div>
        <strong>{value || "—"}</strong>
        {secondary ? <small>{secondary}</small> : null}
      </div>
    </div>
  );
}
