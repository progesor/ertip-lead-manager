import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CommandError, LeadStatus } from "../leads/types";
import "./analytics.css";
import { MarketingDimensionsPanel } from "./MarketingDimensionsPanel";
import type {
  AnalyticsBreakdownPoint,
  AnalyticsResponse,
  AnalyticsStatusPoint,
  AnalyticsTrendPoint,
} from "./types";

type RangeMode = "7d" | "30d" | "90d" | "all" | "custom";

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

const productLabels: Record<string, string> = {
  FUE_MICROMOTOR_SYSTEMS: "FUE Micromotor Systems",
  LONG_HAIR_FUE_SOLUTIONS: "Long Hair FUE Solutions",
  FUE_PUNCHES: "FUE Punches",
  IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS: "Implanters / Forceps / Instruments",
  MEDICAL_CHAIRS_CLINIC_FURNITURE: "Medical Chairs & Clinic Furniture",
  OTHER_GENERAL_INFORMATION: "Diğer / Genel Bilgi",
  UNKNOWN: "Bilinmeyen / Legacy",
  NO_PRODUCT: "Ürün ilgisi yok",
};

const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Analiz verileri yüklenemedi.";
}

function localDateInput(date: Date) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function presetDates(days: number) {
  const end = new Date();
  const start = new Date(end.getFullYear(), end.getMonth(), end.getDate() - (days - 1));
  return { from: localDateInput(start), to: localDateInput(end) };
}

function localDateBoundary(value: string, endExclusive: boolean) {
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(year, month - 1, day + (endExclusive ? 1 : 0), 0, 0, 0, 0);
  return date.toISOString();
}

function formatNumber(value: number) {
  return new Intl.NumberFormat("tr-TR").format(value);
}

function formatPercent(value: number, denominator: number) {
  if (denominator <= 0) return "0%";
  return new Intl.NumberFormat("tr-TR", {
    style: "percent",
    maximumFractionDigits: 1,
  }).format(value / denominator);
}

function formatDay(value: string) {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", { day: "2-digit", month: "short" }).format(date);
}

function formatTimestamp(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", { dateStyle: "medium" }).format(date);
}

function countryLabel(key: string) {
  if (key === "UNKNOWN") return "Ülke bilinmiyor";
  try {
    const name = regionNames.of(key);
    return name && name !== key ? `${key} · ${name}` : key;
  } catch {
    return key;
  }
}

function platformLabel(key: string) {
  switch (key) {
    case "facebook":
      return "Facebook";
    case "instagram":
      return "Instagram";
    case "messenger":
      return "Messenger";
    case "audience_network":
      return "Audience Network";
    case "unknown":
      return "Platform bilinmiyor";
    default:
      return key.replaceAll("_", " ");
  }
}

export function AnalyticsPage() {
  const initial = useMemo(() => presetDates(30), []);
  const [mode, setMode] = useState<RangeMode>("30d");
  const [fromDate, setFromDate] = useState(initial.from);
  const [toDate, setToDate] = useState(initial.to);
  const [report, setReport] = useState<AnalyticsResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadReport = useCallback(async () => {
    if (mode !== "all" && (!fromDate || !toDate || fromDate > toDate)) return;
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<AnalyticsResponse>("get_analytics_report", {
        request: {
          fromUtc: mode === "all" ? null : localDateBoundary(fromDate, false),
          toUtc: mode === "all" ? null : localDateBoundary(toDate, true),
        },
      });
      setReport(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, [fromDate, mode, toDate]);

  useEffect(() => {
    void loadReport();
  }, [loadReport]);

  function applyPreset(nextMode: Exclude<RangeMode, "custom">) {
    setMode(nextMode);
    if (nextMode === "all") return;
    const days = nextMode === "7d" ? 7 : nextMode === "30d" ? 30 : 90;
    const dates = presetDates(days);
    setFromDate(dates.from);
    setToDate(dates.to);
  }

  const won = report?.currentStatusFunnel.find((item) => item.status === "WON")?.contacts ?? 0;
  const summary = report?.summary;
  const repeatRate = summary ? formatPercent(summary.repeatSubmissions, summary.submissions) : "—";
  const wonRate = summary ? formatPercent(won, summary.uniqueContacts) : "—";
  const submissionsPerContact =
    summary && summary.uniqueContacts > 0
      ? new Intl.NumberFormat("tr-TR", { maximumFractionDigits: 2 }).format(
          summary.submissions / summary.uniqueContacts,
        )
      : "—";

  return (
    <section className="page-stack analytics-page">
      <div className="page-heading analytics-heading">
        <div>
          <div className="eyebrow">M5 · ANALYTICS</div>
          <h1>Analiz</h1>
          <p>Submission akışını, benzersiz kişileri ve mevcut CRM durumunu aynı tarih penceresinde karşılaştır.</p>
        </div>
        <button type="button" className="analytics-refresh" onClick={() => void loadReport()} disabled={loading}>
          {loading ? "Yenileniyor…" : "Yenile"}
        </button>
      </div>

      <article className="panel analytics-filter-panel">
        <fieldset className="analytics-presets">
          <legend>Tarih aralığı hızlı seçimleri</legend>
          {(["7d", "30d", "90d", "all"] as const).map((value) => (
            <button
              type="button"
              key={value}
              className={mode === value ? "is-active" : ""}
              aria-pressed={mode === value}
              onClick={() => applyPreset(value)}
            >
              {value === "7d" ? "7 Gün" : value === "30d" ? "30 Gün" : value === "90d" ? "90 Gün" : "Tümü"}
            </button>
          ))}
        </fieldset>
        <div className="analytics-custom-range">
          <label>
            <span>Başlangıç</span>
            <input
              type="date"
              value={fromDate}
              disabled={mode === "all"}
              onChange={(event) => {
                setMode("custom");
                setFromDate(event.target.value);
              }}
            />
          </label>
          <label>
            <span>Bitiş</span>
            <input
              type="date"
              value={toDate}
              disabled={mode === "all"}
              onChange={(event) => {
                setMode("custom");
                setToDate(event.target.value);
              }}
            />
          </label>
          <div className="analytics-data-range">
            <span>DB veri aralığı</span>
            <strong>
              {formatTimestamp(report?.range.earliestSubmissionAt ?? null)} — {formatTimestamp(report?.range.latestSubmissionAt ?? null)}
            </strong>
          </div>
        </div>
      </article>

      {error ? <div className="import-error" role="alert">{error}</div> : null}

      <div className="analytics-kpi-grid" aria-busy={loading}>
        <MetricCard label="Submission" value={summary ? formatNumber(summary.submissions) : "—"} hint="Benzersiz form gönderimi" />
        <MetricCard label="Benzersiz Kişi" value={summary ? formatNumber(summary.uniqueContacts) : "—"} hint="Dönemde ≥1 submission" />
        <MetricCard label="Repeat Submission" value={summary ? formatNumber(summary.repeatSubmissions) : "—"} hint={`Submissionların ${repeatRate}`} />
        <MetricCard label="Submission / Kişi" value={submissionsPerContact} hint="Dönem ortalaması" />
        <MetricCard label="Mevcut WON" value={formatNumber(won)} hint={`Dönem kişilerine göre ${wonRate}`} />
      </div>

      <article className="panel analytics-trend-panel">
        <div className="panel-heading">
          <div>
            <h2>Submission Trendi</h2>
            <p>Gün bazında gerçek submission ve o gün submission gönderen benzersiz kişi sayısı.</p>
          </div>
          <div className="analytics-legend">
            <span><i className="is-submission" />Submission</span>
            <span><i className="is-contact" />Benzersiz kişi</span>
          </div>
        </div>
        <TrendBars points={report?.trend ?? []} />
      </article>

      <div className="analytics-main-grid">
        <article className="panel analytics-funnel-panel">
          <div className="panel-heading">
            <div>
              <h2>Mevcut Durum Funnel'ı</h2>
              <p>Payda: seçilen dönemde submission gönderen {formatNumber(summary?.uniqueContacts ?? 0)} benzersiz kişi. Bu, tarihsel status snapshot değildir.</p>
            </div>
          </div>
          <StatusFunnel points={report?.currentStatusFunnel ?? []} denominator={summary?.uniqueContacts ?? 0} />
        </article>

        <article className="panel analytics-definition-panel">
          <div className="panel-heading"><div><h2>Metrik Tanımları</h2><p>Raporların aynı şeyi ifade etmesi için sabit kurallar.</p></div></div>
          <dl>
            <div><dt>Submission</dt><dd>DB'ye eklenmiş benzersiz Meta form gönderimi. Exact duplicate importlar dahil değildir.</dd></div>
            <div><dt>Repeat Submission</dt><dd>Aynı contact'ın ilk gerçek submission'ından sonra gelen ikinci ve sonraki gönderimler.</dd></div>
            <div><dt>Ürün kırılımı</dt><dd>Kaynak submission ürün ilgisidir. Multi-select bir submission her seçtiği ürün kategorisine ayrı katkı verir.</dd></div>
            <div><dt>Funnel</dt><dd>Dönemde görülen kişilerin bugünkü CRM status dağılımıdır; geçmişteki status değişimlerini yeniden kurmaz.</dd></div>
          </dl>
        </article>
      </div>

      <div className="analytics-breakdown-grid">
        <BreakdownCard title="Ülke" hint="Normalize contact ülkesine göre" rows={report?.countryBreakdown ?? []} labelFor={countryLabel} />
        <BreakdownCard title="Platform" hint="Submission kaynak platformuna göre" rows={report?.platformBreakdown ?? []} labelFor={platformLabel} />
        <BreakdownCard title="Ürün İlgisi" hint="Kaynak normalize submission ilgileri · multi-select üyelik" rows={report?.productBreakdown ?? []} labelFor={(key) => productLabels[key] ?? key} />
      </div>

      <MarketingDimensionsPanel
        campaigns={report?.campaignBreakdown ?? []}
        forms={report?.formBreakdown ?? []}
        adsets={report?.adsetBreakdown ?? []}
        ads={report?.adBreakdown ?? []}
      />
    </section>
  );
}

function MetricCard({ label, value, hint }: { label: string; value: string; hint: string }) {
  return <article className="kpi-card analytics-kpi-card"><div className="kpi-label">{label}</div><div className="kpi-value">{value}</div><div className="kpi-hint">{hint}</div></article>;
}

function TrendBars({ points }: { points: AnalyticsTrendPoint[] }) {
  if (points.length === 0) return <div className="analytics-empty">Bu tarih aralığında submission yok.</div>;
  const max = Math.max(...points.map((point) => point.submissions), 1);
  return (
    <div className="analytics-trend-scroll">
      <div className="analytics-trend-bars" style={{ minWidth: Math.max(640, points.length * 28) }}>
        {points.map((point, index) => {
          const showLabel = points.length <= 18 || index === 0 || index === points.length - 1 || index % Math.ceil(points.length / 10) === 0;
          return (
            <div className="analytics-trend-day" key={point.day} title={`${formatDay(point.day)} · ${point.submissions} submission · ${point.uniqueContacts} kişi · ${point.repeatSubmissions} repeat`}>
              <div className="analytics-trend-value">{point.submissions}</div>
              <div className="analytics-trend-columns">
                <span className="is-submission" style={{ height: `${Math.max(4, (point.submissions / max) * 100)}%` }} />
                <span className="is-contact" style={{ height: `${Math.max(4, (point.uniqueContacts / max) * 100)}%` }} />
              </div>
              <div className="analytics-trend-label">{showLabel ? formatDay(point.day) : ""}</div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function StatusFunnel({ points, denominator }: { points: AnalyticsStatusPoint[]; denominator: number }) {
  return (
    <div className="analytics-funnel-list">
      {points.map((point) => (
        <div className="analytics-funnel-row" key={point.status}>
          <div className="analytics-funnel-label"><strong>{statusLabels[point.status]}</strong><span>{formatNumber(point.contacts)} · {formatPercent(point.contacts, denominator)}</span></div>
          <div className="analytics-funnel-track"><span className={`analytics-funnel-fill status-${point.status.toLowerCase()}`} style={{ width: `${denominator > 0 ? Math.max(0, (point.contacts / denominator) * 100) : 0}%` }} /></div>
        </div>
      ))}
    </div>
  );
}

function BreakdownCard({ title, hint, rows, labelFor }: { title: string; hint: string; rows: AnalyticsBreakdownPoint[]; labelFor: (key: string) => string }) {
  const visible = rows.slice(0, 10);
  const max = Math.max(...visible.map((row) => row.submissions), 1);
  return (
    <article className="panel analytics-breakdown-card">
      <div className="panel-heading"><div><h2>{title}</h2><p>{hint}</p></div><span className="placeholder-pill">{rows.length} kategori</span></div>
      {visible.length > 0 ? (
        <div className="analytics-breakdown-list">
          {visible.map((row) => (
            <div className="analytics-breakdown-row" key={row.key}>
              <div className="analytics-breakdown-copy"><strong title={labelFor(row.key)}>{labelFor(row.key)}</strong><span>{formatNumber(row.submissions)} submission · {formatNumber(row.uniqueContacts)} kişi</span></div>
              <div className="analytics-breakdown-track"><span style={{ width: `${(row.submissions / max) * 100}%` }} /></div>
            </div>
          ))}
          {rows.length > visible.length ? <div className="analytics-more">İlk {visible.length} kategori gösteriliyor.</div> : null}
        </div>
      ) : <div className="analytics-empty">Bu kırılım için veri yok.</div>}
    </article>
  );
}
