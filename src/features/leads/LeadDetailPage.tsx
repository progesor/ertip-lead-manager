import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import "./lead-detail.css";
import type {
  CommandError,
  DataQualityIssueType,
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

const productLabels: Record<ProductCode, string> = {
  FUE_MICROMOTOR_SYSTEMS: "FUE Micromotor",
  LONG_HAIR_FUE_SOLUTIONS: "Long Hair FUE",
  FUE_PUNCHES: "FUE Punches",
  IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS: "Implanter / Forceps",
  MEDICAL_CHAIRS_CLINIC_FURNITURE: "Medikal Mobilya",
  OTHER_GENERAL_INFORMATION: "Diğer / Genel Bilgi",
  UNKNOWN: "Bilinmiyor",
};

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
  return "Lead detayı yüklenemedi.";
}

export function LeadDetailPage() {
  const { leadId } = useParams();
  const navigate = useNavigate();
  const [detail, setDetail] = useState<LeadDetailResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!leadId) {
      setError("Lead kimliği bulunamadı.");
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    void invoke<LeadDetailResponse | null>("get_lead_detail", { contactId: leadId })
      .then((response) => {
        if (!response) {
          setError("Lead kaydı bulunamadı.");
          setDetail(null);
          return;
        }
        setDetail(response);
      })
      .catch((loadError) => setError(commandErrorMessage(loadError)))
      .finally(() => setLoading(false));
  }, [leadId]);

  if (loading) {
    return (
      <section className="page-stack lead-detail-page">
        <article className="panel lead-detail-loading">Lead detayı yükleniyor…</article>
      </section>
    );
  }

  if (error || !detail) {
    return (
      <section className="page-stack lead-detail-page">
        <button type="button" className="lead-back-button" onClick={() => navigate("/leads")}>← Leadlere Dön</button>
        <div className="import-error" role="alert">{error ?? "Lead detayı bulunamadı."}</div>
      </section>
    );
  }

  const { contact } = detail;
  const openIssues = detail.qualityIssues.filter((issue) => issue.status === "OPEN");

  return (
    <section className="page-stack lead-detail-page">
      <div className="lead-detail-topbar">
        <button type="button" className="lead-back-button" onClick={() => navigate("/leads")}>← Leadlere Dön</button>
        <span className="lead-detail-id" title={contact.id}>{contact.id}</span>
      </div>

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
        <div className="lead-detail-metrics">
          <div><strong>{contact.submissionCount}</strong><span>submission</span></div>
          <div><strong>{openIssues.length}</strong><span>açık uyarı</span></div>
          <div><strong>{formatDate(contact.latestSubmissionAt)}</strong><span>son lead</span></div>
        </div>
      </article>

      <div className="lead-detail-grid">
        <article className="panel lead-detail-products">
          <div className="panel-heading">
            <div>
              <h2>Ürün İlgileri</h2>
              <p>Tüm submission'lardan birleştirilmiş görünüm.</p>
            </div>
          </div>
          <div className="lead-product-list">
            {contact.productInterests.length > 0 ? (
              contact.productInterests.map((product) => (
                <span className="lead-product-chip" key={product}>{productLabels[product] ?? product}</span>
              ))
            ) : (
              <span className="lead-muted">Ürün ilgisi yok.</span>
            )}
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
            {detail.activities.map((activity) => (
              <div className="lead-activity-row" key={activity.id}>
                <span className="lead-activity-dot" />
                <div>
                  <strong>{activityLabels[activity.activityType] ?? activity.activityType}</strong>
                  <span>{formatDate(activity.occurredAt)}</span>
                </div>
              </div>
            ))}
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
