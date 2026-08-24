import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import type { CommandError, LeadStatus } from "../leads/types";
import "./dashboard.css";

interface DashboardKpis {
  totalContacts: number;
  newContacts: number;
  qualifiedContacts: number;
  quoteSentContacts: number;
  wonContacts: number;
}

interface DashboardAnalyticsSummary {
  submissions: number;
  uniqueContacts: number;
  repeatSubmissions: number;
  wonContacts: number;
}

interface DashboardAttentionLead {
  id: string;
  displayName: string;
  status: LeadStatus;
  primaryPhone: string | null;
  countryCode: string | null;
  latestSubmissionAt: string | null;
  dueAt: string | null;
  count: number;
}

interface DashboardAttentionGroup {
  total: number;
  items: DashboardAttentionLead[];
}

interface DashboardAttentionResponse {
  kpis: DashboardKpis;
  analytics30d: DashboardAnalyticsSummary;
  newUncontacted: DashboardAttentionGroup;
  dueToday: DashboardAttentionGroup;
  overdue: DashboardAttentionGroup;
  recentRepeats: DashboardAttentionGroup;
  openQualityIssues: DashboardAttentionGroup;
}

interface AttentionDefinition {
  key: keyof Pick<
    DashboardAttentionResponse,
    "overdue" | "dueToday" | "newUncontacted" | "recentRepeats" | "openQualityIssues"
  >;
  title: string;
  hint: string;
  tone: string;
}

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

const attentionDefinitions: AttentionDefinition[] = [
  { key: "overdue", title: "Gecikmiş Takipler", hint: "Zamanı geçmiş açık follow-up'lar", tone: "danger" },
  { key: "dueToday", title: "Bugünkü Takipler", hint: "Bugün aksiyon bekleyen leadler", tone: "warning" },
  { key: "newUncontacted", title: "Yeni Leadler", hint: "Henüz iletişime geçilmemiş leadler", tone: "primary" },
  { key: "recentRepeats", title: "Recent Repeat", hint: "Son 7 günde yeniden form gönderenler", tone: "info" },
  { key: "openQualityIssues", title: "Veri Uyarıları", hint: "Açık veri-kalite sorunları", tone: "neutral" },
];

const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", { dateStyle: "short", timeStyle: "short" }).format(date);
}

function formatCountry(code: string | null) {
  if (!code) return "Ülke yok";
  const normalized = code.trim().toUpperCase();
  try {
    const name = regionNames.of(normalized);
    return name && name !== normalized ? `${normalized} · ${name}` : normalized;
  } catch {
    return normalized;
  }
}

function formatPercent(value: number, denominator: number) {
  if (denominator <= 0) return "0%";
  return new Intl.NumberFormat("tr-TR", { style: "percent", maximumFractionDigits: 1 }).format(
    value / denominator,
  );
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Dashboard verileri yüklenemedi.";
}

function dashboardRequest() {
  const now = new Date();
  const tomorrowStart = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 0, 0, 0, 0);
  const recentRepeatSince = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
  const analyticsSince = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 29, 0, 0, 0, 0);

  return {
    nowUtc: now.toISOString(),
    todayStartUtc: now.toISOString(),
    tomorrowStartUtc: tomorrowStart.toISOString(),
    recentRepeatSinceUtc: recentRepeatSince.toISOString(),
    analyticsSinceUtc: analyticsSince.toISOString(),
    groupLimit: 6,
  };
}

function itemTimestamp(groupKey: AttentionDefinition["key"], item: DashboardAttentionLead) {
  if (groupKey === "overdue" || groupKey === "dueToday") return item.dueAt;
  return item.latestSubmissionAt;
}

function itemCountLabel(groupKey: AttentionDefinition["key"], count: number) {
  if (count <= 1) return null;
  if (groupKey === "recentRepeats") return `${count} submission`;
  if (groupKey === "openQualityIssues") return `${count} uyarı`;
  if (groupKey === "overdue" || groupKey === "dueToday") return `${count} takip`;
  return null;
}

export function DashboardPage() {
  const navigate = useNavigate();
  const [dashboard, setDashboard] = useState<DashboardAttentionResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadDashboard = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<DashboardAttentionResponse>("get_dashboard_attention", {
        request: dashboardRequest(),
      });
      setDashboard(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadDashboard();
  }, [loadDashboard]);

  function openLead(contactId: string) {
    navigate(`/leads/${contactId}`, {
      state: { returnTo: "/", returnLabel: "Genel Bakış'a Dön" },
    });
  }

  const kpis = dashboard?.kpis;
  const cards = [
    ["Toplam Lead", kpis?.totalContacts ?? 0, "Benzersiz kişi"],
    ["Yeni", kpis?.newContacts ?? 0, "İletişim bekleyen"],
    ["Nitelikli", kpis?.qualifiedContacts ?? 0, "QUALIFIED"],
    ["Teklif", kpis?.quoteSentContacts ?? 0, "QUOTE_SENT"],
    ["Kazanılan", kpis?.wonContacts ?? 0, "WON"],
  ] as const;
  const analytics = dashboard?.analytics30d;

  return (
    <section className="page-stack dashboard-page">
      <div className="page-heading dashboard-heading">
        <div>
          <div className="eyebrow">GÜNLÜK ÇALIŞMA ALANI</div>
          <h1>Genel Bakış</h1>
          <p>Önce aksiyon gerektiren leadleri gör; detay gerektiğinde aç, operasyonu Kanban'dan yönet.</p>
        </div>
        <div className="dashboard-heading-actions">
          <button type="button" className="dashboard-refresh" onClick={() => void loadDashboard()} disabled={loading}>
            {loading ? "Yenileniyor…" : "Yenile"}
          </button>
          <button type="button" className="primary-button" onClick={() => navigate("/pipeline")}>
            Pipeline'ı Aç
          </button>
        </div>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}

      <div className="dashboard-kpi-grid">
        {cards.map(([title, value, hint]) => (
          <article className="kpi-card" key={title}>
            <div className="kpi-label">{title}</div>
            <div className="kpi-value">{loading && !dashboard ? "—" : value}</div>
            <div className="kpi-hint">{hint}</div>
          </article>
        ))}
      </div>

      <div className="dashboard-attention-heading">
        <div>
          <h2>İlgilenilmesi Gerekenler</h2>
          <p>Satış günü içinde önce bakılması gereken kayıtlar.</p>
        </div>
      </div>

      <div className="dashboard-attention-grid" aria-busy={loading}>
        {attentionDefinitions.map((definition) => {
          const group = dashboard?.[definition.key];
          return (
            <article className={`panel dashboard-attention-card is-${definition.tone}`} key={definition.key}>
              <div className="dashboard-attention-card-heading">
                <div>
                  <h3>{definition.title}</h3>
                  <p>{definition.hint}</p>
                </div>
                <strong>{group?.total ?? 0}</strong>
              </div>

              <div className="dashboard-attention-list">
                {(group?.items ?? []).map((item) => {
                  const countLabel = itemCountLabel(definition.key, item.count);
                  return (
                    <button type="button" className="dashboard-attention-item" key={item.id} onClick={() => openLead(item.id)}>
                      <div className="dashboard-attention-item-main">
                        <strong>{item.displayName}</strong>
                        <span className="dashboard-attention-phone">{item.primaryPhone ?? "Telefon bilgisi yok"}</span>
                        <span>{formatCountry(item.countryCode)}</span>
                      </div>
                      <div className="dashboard-attention-item-meta">
                        <span className={`lead-status lead-status-${item.status.toLowerCase()}`}>{statusLabels[item.status]}</span>
                        <span>{formatDate(itemTimestamp(definition.key, item))}</span>
                        {countLabel ? <em>{countLabel}</em> : null}
                      </div>
                    </button>
                  );
                })}

                {!loading && (group?.items.length ?? 0) === 0 ? (
                  <div className="dashboard-attention-empty">Şu anda kayıt yok.</div>
                ) : null}
              </div>

              {group && group.total > group.items.length ? (
                <div className="dashboard-attention-more">İlk {group.items.length} kayıt gösteriliyor · toplam {group.total}</div>
              ) : null}
            </article>
          );
        })}
      </div>

      <article className="panel dashboard-analytics-summary">
        <div className="dashboard-analytics-summary-copy">
          <div className="eyebrow">SON 30 GÜN</div>
          <strong>Lead Akışı</strong>
          <span>Detaylı trend ve kaynak kırılımları Analiz ekranında.</span>
        </div>
        <div className="dashboard-analytics-metrics">
          <div><strong>{analytics?.submissions ?? 0}</strong><span>submission</span></div>
          <div><strong>{analytics?.uniqueContacts ?? 0}</strong><span>benzersiz kişi</span></div>
          <div><strong>{formatPercent(analytics?.repeatSubmissions ?? 0, analytics?.submissions ?? 0)}</strong><span>repeat oranı</span></div>
          <div><strong>{formatPercent(analytics?.wonContacts ?? 0, analytics?.uniqueContacts ?? 0)}</strong><span>mevcut WON</span></div>
        </div>
        <button type="button" className="dashboard-analytics-link" onClick={() => navigate("/analytics")}>
          Analize Git →
        </button>
      </article>
    </section>
  );
}
