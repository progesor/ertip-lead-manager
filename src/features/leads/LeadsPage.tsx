import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import "./leads.css";
import type {
  CommandError,
  DataQualityIssueType,
  LeadFilterOptions,
  LeadListResponse,
  LeadListSort,
  LeadStatus,
  ProductCode,
} from "./types";

const PAGE_SIZE = 25;

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

const platformLabels: Record<string, string> = {
  facebook: "Facebook",
  instagram: "Instagram",
  messenger: "Messenger",
  audience_network: "Audience Network",
};

const productOptions = Object.entries(productLabels) as Array<[ProductCode, string]>;
const statusOptions = Object.entries(statusLabels) as Array<[LeadStatus, string]>;
const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

const emptyResponse: LeadListResponse = {
  items: [],
  total: 0,
  page: 0,
  pageSize: PAGE_SIZE,
  totalPages: 0,
};

const emptyFilterOptions: LeadFilterOptions = {
  countries: [],
};

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", {
    dateStyle: "short",
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

function formatPlatform(platform: string) {
  const normalized = platform.trim().toLowerCase();
  return platformLabels[normalized] ?? normalized.replaceAll("_", " ");
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Lead listesi yüklenemedi.";
}

function clean(value: string) {
  const next = value.trim();
  return next.length > 0 ? next : null;
}

export function LeadsPage() {
  const [response, setResponse] = useState<LeadListResponse>(emptyResponse);
  const [filterOptions, setFilterOptions] = useState<LeadFilterOptions>(emptyFilterOptions);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<LeadStatus | "">("");
  const [country, setCountry] = useState("");
  const [countryQuery, setCountryQuery] = useState("");
  const [countryOpen, setCountryOpen] = useState(false);
  const [product, setProduct] = useState<ProductCode | "">("");
  const [repeatOnly, setRepeatOnly] = useState(false);
  const [warningOnly, setWarningOnly] = useState(false);
  const [sort, setSort] = useState<LeadListSort>("LATEST_DESC");
  const [page, setPage] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const requestSequence = useRef(0);

  const loadLeads = useCallback(async () => {
    const sequence = ++requestSequence.current;
    setLoading(true);
    setError(null);

    try {
      const next = await invoke<LeadListResponse>("list_leads", {
        request: {
          search: clean(search),
          status: status || null,
          countryCode: country || null,
          productCode: product || null,
          repeatOnly,
          warningOnly,
          sort,
          page,
          pageSize: PAGE_SIZE,
        },
      });

      if (sequence === requestSequence.current) {
        setResponse(next);
      }
    } catch (loadError) {
      if (sequence === requestSequence.current) {
        setError(commandErrorMessage(loadError));
      }
    } finally {
      if (sequence === requestSequence.current) {
        setLoading(false);
      }
    }
  }, [country, page, product, repeatOnly, search, sort, status, warningOnly]);

  useEffect(() => {
    void invoke<LeadFilterOptions>("get_lead_filter_options")
      .then(setFilterOptions)
      .catch((loadError) => setError((current) => current ?? commandErrorMessage(loadError)));
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadLeads();
    }, search.trim().length > 0 ? 220 : 0);

    return () => window.clearTimeout(timer);
  }, [loadLeads, search]);

  function resetPage() {
    setPage(0);
  }

  function clearFilters() {
    setSearch("");
    setStatus("");
    setCountry("");
    setCountryQuery("");
    setCountryOpen(false);
    setProduct("");
    setRepeatOnly(false);
    setWarningOnly(false);
    setSort("LATEST_DESC");
    setPage(0);
  }

  function selectCountry(code: string) {
    setCountry(code);
    setCountryQuery("");
    setCountryOpen(false);
    resetPage();
  }

  const start = response.total === 0 ? 0 : response.page * response.pageSize + 1;
  const end = Math.min(response.total, (response.page + 1) * response.pageSize);
  const hasFilters = Boolean(search || status || country || countryQuery || product || repeatOnly || warningOnly);
  const normalizedCountryQuery = countryQuery.trim().toLocaleLowerCase("tr-TR");
  const filteredCountries = filterOptions.countries.filter((code) => {
    if (!normalizedCountryQuery) return true;
    return (
      code.toLocaleLowerCase("tr-TR").includes(normalizedCountryQuery) ||
      formatCountry(code).toLocaleLowerCase("tr-TR").includes(normalizedCountryQuery)
    );
  });

  return (
    <section className="page-stack leads-page">
      <div className="page-heading leads-heading">
        <div>
          <div className="eyebrow">LEAD WORKSPACE</div>
          <h1>Leadler</h1>
          <p>İçe aktarılan benzersiz kişileri ara, filtrele ve günlük satış çalışmasına hazırla.</p>
        </div>
        <div className="leads-heading-summary">
          <strong>{response.total}</strong>
          <span>benzersiz kişi</span>
        </div>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}

      <article className="panel leads-filter-panel">
        <div className="leads-search-row">
          <label className="leads-search-field">
            <span>Arama</span>
            <input
              type="search"
              value={search}
              onChange={(event) => {
                setSearch(event.target.value);
                resetPage();
              }}
              placeholder="Ad, e-posta, telefon veya Meta Lead ID"
            />
          </label>

          <label>
            <span>Durum</span>
            <select
              value={status}
              onChange={(event) => {
                setStatus(event.target.value as LeadStatus | "");
                resetPage();
              }}
            >
              <option value="">Tüm durumlar</option>
              {statusOptions.map(([value, label]) => (
                <option value={value} key={value}>{label}</option>
              ))}
            </select>
          </label>

          <div className="leads-country-combobox">
            <span className="leads-field-label">Ülke</span>
            <div className="leads-country-input-wrap">
              <input
                type="search"
                role="combobox"
                value={country ? formatCountry(country) : countryQuery}
                onFocus={() => setCountryOpen(true)}
                onBlur={() => window.setTimeout(() => setCountryOpen(false), 120)}
                onChange={(event) => {
                  setCountry("");
                  setCountryQuery(event.target.value);
                  setCountryOpen(true);
                  resetPage();
                }}
                placeholder="Kod veya ülke ara"
                aria-label="Ülke filtresi"
                aria-expanded={countryOpen}
                aria-controls="lead-country-options"
                aria-autocomplete="list"
              />
              {(country || countryQuery) ? (
                <button
                  type="button"
                  className="leads-country-clear"
                  aria-label="Ülke filtresini temizle"
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    setCountry("");
                    setCountryQuery("");
                    setCountryOpen(false);
                    resetPage();
                  }}
                >
                  ×
                </button>
              ) : null}
            </div>
            {countryOpen ? (
              <div className="leads-country-menu" id="lead-country-options" role="listbox">
                {filteredCountries.length > 0 ? (
                  filteredCountries.map((code) => (
                    <button
                      type="button"
                      role="option"
                      aria-selected={country === code}
                      key={code}
                      onMouseDown={(event) => {
                        event.preventDefault();
                        selectCountry(code);
                      }}
                    >
                      <strong>{code}</strong>
                      <span>{formatCountry(code).replace(`${code} · `, "")}</span>
                    </button>
                  ))
                ) : (
                  <div className="leads-country-empty">Eşleşen ülke yok</div>
                )}
              </div>
            ) : null}
          </div>

          <label>
            <span>Ürün İlgisi</span>
            <select
              value={product}
              onChange={(event) => {
                setProduct(event.target.value as ProductCode | "");
                resetPage();
              }}
            >
              <option value="">Tüm ürünler</option>
              {productOptions.map(([value, label]) => (
                <option value={value} key={value}>{label}</option>
              ))}
            </select>
          </label>

          <label>
            <span>Sıralama</span>
            <select
              value={sort}
              onChange={(event) => {
                setSort(event.target.value as LeadListSort);
                resetPage();
              }}
            >
              <option value="LATEST_DESC">En yeni lead önce</option>
              <option value="LATEST_ASC">En eski lead önce</option>
              <option value="NAME_ASC">İsim A → Z</option>
              <option value="NAME_DESC">İsim Z → A</option>
            </select>
          </label>
        </div>

        <div className="leads-filter-actions">
          <button
            type="button"
            className={`leads-toggle ${repeatOnly ? "is-active" : ""}`}
            aria-pressed={repeatOnly}
            onClick={() => {
              setRepeatOnly((value) => !value);
              resetPage();
            }}
          >
            Repeat leadler
          </button>
          <button
            type="button"
            className={`leads-toggle ${warningOnly ? "is-active" : ""}`}
            aria-pressed={warningOnly}
            onClick={() => {
              setWarningOnly((value) => !value);
              resetPage();
            }}
          >
            Uyarılı leadler
          </button>
          {hasFilters ? (
            <button type="button" className="leads-clear-button" onClick={clearFilters}>
              Filtreleri Temizle
            </button>
          ) : null}
          <span className="leads-result-range">
            {loading ? "Yükleniyor…" : `${start}–${end} / ${response.total}`}
          </span>
        </div>
      </article>

      <article className="panel leads-list-panel">
        <div className="leads-table-wrap">
          <table className="leads-table">
            <thead>
              <tr>
                <th>Lead</th>
                <th>Durum</th>
                <th>Ülke</th>
                <th>Platform</th>
                <th>Ürün İlgisi</th>
                <th>Submission</th>
                <th>Veri Uyarısı</th>
                <th>Son Lead</th>
              </tr>
            </thead>
            <tbody>
              {response.items.map((lead) => (
                <tr key={lead.id}>
                  <td>
                    <div className="lead-primary-line">
                      <strong>{lead.displayName}</strong>
                      {lead.isRepeat ? <span className="lead-repeat-badge">Repeat</span> : null}
                    </div>
                    <div className="lead-contact-line">
                      {lead.primaryEmail ?? lead.primaryPhone ?? "İletişim bilgisi yok"}
                    </div>
                    {lead.primaryEmail && lead.primaryPhone ? (
                      <div className="lead-contact-secondary">{lead.primaryPhone}</div>
                    ) : null}
                  </td>
                  <td>
                    <span className={`lead-status lead-status-${lead.status.toLowerCase()}`}>
                      {statusLabels[lead.status]}
                    </span>
                  </td>
                  <td>
                    <span className="lead-country" title={lead.countryCode ?? undefined}>
                      {formatCountry(lead.countryCode)}
                    </span>
                  </td>
                  <td>
                    <div className="lead-platform-list">
                      {lead.platforms.length > 0 ? (
                        lead.platforms.map((platform) => (
                          <span className={`lead-platform-chip lead-platform-${platform}`} key={platform}>
                            {formatPlatform(platform)}
                          </span>
                        ))
                      ) : (
                        <span className="lead-muted">—</span>
                      )}
                    </div>
                  </td>
                  <td>
                    <div className="lead-product-list">
                      {lead.productInterests.length > 0 ? (
                        lead.productInterests.map((code) => (
                          <span className="lead-product-chip" key={code}>
                            {productLabels[code] ?? code}
                          </span>
                        ))
                      ) : (
                        <span className="lead-muted">—</span>
                      )}
                    </div>
                  </td>
                  <td>
                    <span className="lead-count-badge" title={`${lead.submissionCount} ayrı form kaydı`}>
                      {lead.submissionCount}
                    </span>
                  </td>
                  <td>
                    {lead.warningCount > 0 ? (
                      <div className="lead-warning-list">
                        {lead.warningSummaries.map((warning) => (
                          <span className="lead-warning-detail" key={warning.issueType}>
                            {warningLabels[warning.issueType] ?? warning.issueType}
                            {warning.count > 1 ? ` ×${warning.count}` : ""}
                          </span>
                        ))}
                      </div>
                    ) : (
                      <span className="lead-muted">—</span>
                    )}
                  </td>
                  <td className="lead-date-cell">{formatDate(lead.latestSubmissionAt)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {!loading && response.items.length === 0 ? (
          <div className="leads-empty-state">
            <strong>Eşleşen lead bulunamadı.</strong>
            <span>Arama veya filtreleri değiştirerek tekrar deneyin.</span>
          </div>
        ) : null}

        <div className="leads-pagination">
          <button
            type="button"
            disabled={loading || response.page <= 0}
            onClick={() => setPage((value) => Math.max(0, value - 1))}
          >
            ← Önceki
          </button>
          <span>
            Sayfa {response.totalPages === 0 ? 0 : response.page + 1} / {response.totalPages}
          </span>
          <button
            type="button"
            disabled={loading || response.totalPages === 0 || response.page + 1 >= response.totalPages}
            onClick={() => setPage((value) => value + 1)}
          >
            Sonraki →
          </button>
        </div>
      </article>
    </section>
  );
}
