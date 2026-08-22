import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type {
  CommandError,
  LeadFilterOptions,
  LeadStatus,
  ProductCode,
} from "../leads/types";
import "./pipeline.css";
import type { PipelineBoardResponse, PipelineCard } from "./types";

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
  UNKNOWN: "Bilinmiyor",
};

const productOptions = Object.entries(productLabels) as Array<[ProductCode, string]>;
const regionNames = new Intl.DisplayNames(["tr"], { type: "region" });

const emptyFilters: LeadFilterOptions = { countries: [] };

function formatDate(value: string | null) {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("tr-TR", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function formatCountry(code: string | null) {
  if (!code) return "—";
  const normalized = code.trim().toUpperCase();
  try {
    const name = regionNames.of(normalized);
    return name && name !== normalized ? `${normalized} · ${name}` : normalized;
  } catch {
    return normalized;
  }
}

function platformLabel(value: string) {
  switch (value.trim().toLowerCase()) {
    case "facebook":
    case "fb":
      return "Facebook";
    case "instagram":
    case "ig":
      return "Instagram";
    case "messenger":
      return "Messenger";
    default:
      return value.replaceAll("_", " ");
  }
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Pipeline işlemi tamamlanamadı.";
}

function clean(value: string) {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function moveCardLocally(
  board: PipelineBoardResponse,
  contactId: string,
  targetStatus: LeadStatus,
): PipelineBoardResponse {
  let movedCard: PipelineCard | null = null;
  let sourceVisible = false;

  const withoutSource = board.columns.map((column) => {
    const card = column.cards.find((item) => item.id === contactId);
    if (!card) return column;

    movedCard = card;
    sourceVisible = true;
    return {
      ...column,
      total: Math.max(0, column.total - 1),
      cards: column.cards.filter((item) => item.id !== contactId),
    };
  });

  const cardToMove = movedCard;
  if (!cardToMove) return board;

  let targetVisible = false;
  const nextColumns = withoutSource.map((column) => {
    if (column.status !== targetStatus) return column;
    targetVisible = true;

    const nextCards = [
      { ...cardToMove, status: targetStatus },
      ...column.cards.filter((item) => item.id !== contactId),
    ].slice(0, board.perColumnLimit);
    const nextTotal = column.total + 1;

    return {
      ...column,
      total: nextTotal,
      cards: nextCards,
      truncated: nextTotal > nextCards.length,
    };
  });

  return {
    ...board,
    columns: nextColumns,
    visibleTotal: board.visibleTotal + (targetVisible ? 1 : 0) - (sourceVisible ? 1 : 0),
  };
}

export function PipelinePage() {
  const [board, setBoard] = useState<PipelineBoardResponse | null>(null);
  const [filterOptions, setFilterOptions] = useState<LeadFilterOptions>(emptyFilters);
  const [search, setSearch] = useState("");
  const [country, setCountry] = useState("");
  const [product, setProduct] = useState<ProductCode | "">("");
  const [repeatOnly, setRepeatOnly] = useState(false);
  const [warningOnly, setWarningOnly] = useState(false);
  const [includeTerminal, setIncludeTerminal] = useState(false);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dragging, setDragging] = useState<{ id: string; status: LeadStatus } | null>(null);
  const [dragOverStatus, setDragOverStatus] = useState<LeadStatus | null>(null);
  const [mutatingId, setMutatingId] = useState<string | null>(null);

  const countryOptions = useMemo(
    () =>
      filterOptions.countries.map((code) => ({
        code,
        label: formatCountry(code),
      })),
    [filterOptions.countries],
  );

  const loadBoard = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<PipelineBoardResponse>("get_pipeline_board", {
        request: {
          search: clean(search),
          countryCode: country || null,
          productCode: product || null,
          repeatOnly,
          warningOnly,
          includeTerminal,
          perColumnLimit: 100,
        },
      });
      setBoard(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, [country, includeTerminal, product, repeatOnly, search, warningOnly]);

  useEffect(() => {
    void invoke<LeadFilterOptions>("get_lead_filter_options")
      .then(setFilterOptions)
      .catch(() => setFilterOptions(emptyFilters));
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void loadBoard();
    }, search.trim().length > 0 ? 220 : 0);
    return () => window.clearTimeout(timer);
  }, [loadBoard, search]);

  async function changeStatus(contactId: string, targetStatus: LeadStatus) {
    if (!board || mutatingId) return;
    const sourceCard = board.columns.flatMap((column) => column.cards).find((card) => card.id === contactId);
    if (!sourceCard || sourceCard.status === targetStatus) return;

    const snapshot = board;
    setMutatingId(contactId);
    setError(null);
    setNotice(null);
    setBoard(moveCardLocally(board, contactId, targetStatus));

    try {
      await invoke<boolean>("change_lead_status", {
        contactId,
        newStatus: targetStatus,
      });
      setNotice(`${sourceCard.displayName}: ${statusLabels[targetStatus]} olarak güncellendi.`);
      await loadBoard();
    } catch (mutationError) {
      setBoard(snapshot);
      setError(`${sourceCard.displayName} taşınamadı. ${commandErrorMessage(mutationError)}`);
    } finally {
      setMutatingId(null);
      setDragging(null);
      setDragOverStatus(null);
    }
  }

  function clearFilters() {
    setSearch("");
    setCountry("");
    setProduct("");
    setRepeatOnly(false);
    setWarningOnly(false);
  }

  const hasFilters = Boolean(search || country || product || repeatOnly || warningOnly);

  return (
    <section className="page-stack pipeline-page">
      <div className="page-heading pipeline-heading">
        <div>
          <div className="eyebrow">LIFECYCLE</div>
          <h1>Pipeline</h1>
          <p>Leadleri satış aşamalarında gör, sürükle veya kart üzerindeki durum seçicisiyle taşı.</p>
        </div>
        <div className="pipeline-total">
          <strong>{board?.visibleTotal ?? 0}</strong>
          <span>görünen lead</span>
        </div>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}
      {notice ? <div className="import-success" role="status">{notice}</div> : null}

      <article className="panel pipeline-toolbar">
        <label className="pipeline-search">
          <span>Arama</span>
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Ad, e-posta, telefon veya Meta Lead ID"
          />
        </label>

        <label>
          <span>Ülke</span>
          <select value={country} onChange={(event) => setCountry(event.target.value)}>
            <option value="">Tüm ülkeler</option>
            {countryOptions.map((option) => (
              <option key={option.code} value={option.code}>{option.label}</option>
            ))}
          </select>
        </label>

        <label>
          <span>Ürün İlgisi</span>
          <select value={product} onChange={(event) => setProduct(event.target.value as ProductCode | "")}>
            <option value="">Tüm ürünler</option>
            {productOptions.map(([value, label]) => (
              <option key={value} value={value}>{label}</option>
            ))}
          </select>
        </label>

        <div className="pipeline-toolbar-actions">
          <button
            type="button"
            className={repeatOnly ? "is-active" : ""}
            aria-pressed={repeatOnly}
            onClick={() => setRepeatOnly((value) => !value)}
          >
            Repeat
          </button>
          <button
            type="button"
            className={warningOnly ? "is-active" : ""}
            aria-pressed={warningOnly}
            onClick={() => setWarningOnly((value) => !value)}
          >
            Uyarılı
          </button>
          <button
            type="button"
            className={includeTerminal ? "is-active" : ""}
            aria-pressed={includeTerminal}
            onClick={() => setIncludeTerminal((value) => !value)}
          >
            Kazanıldı / Kaybedildi göster
          </button>
          {hasFilters ? <button type="button" className="is-clear" onClick={clearFilters}>Filtreleri Temizle</button> : null}
        </div>
      </article>

      <div className="pipeline-board-wrap" aria-busy={loading}>
        <div className="pipeline-board">
          {(board?.columns ?? []).map((column) => (
            <fieldset
              className={`pipeline-column ${dragOverStatus === column.status ? "is-drag-over" : ""}`}
              key={column.status}
              aria-label={`${statusLabels[column.status]} pipeline kolonu`}
              onDragOver={(event) => {
                event.preventDefault();
                if (dragging && dragging.status !== column.status) setDragOverStatus(column.status);
              }}
              onDragLeave={() => {
                if (dragOverStatus === column.status) setDragOverStatus(null);
              }}
              onDrop={(event) => {
                event.preventDefault();
                if (dragging) void changeStatus(dragging.id, column.status);
              }}
            >
              <header className="pipeline-column-heading">
                <div>
                  <span className={`pipeline-column-dot pipeline-column-dot-${column.status.toLowerCase()}`} />
                  <strong>{statusLabels[column.status]}</strong>
                </div>
                <span>{column.total}</span>
              </header>

              <div className="pipeline-card-list">
                {column.cards.map((card) => (
                  <article className={`pipeline-card ${mutatingId === card.id ? "is-mutating" : ""}`} key={card.id}>
                    <div className="pipeline-card-topline">
                      <button
                        type="button"
                        className="pipeline-drag-handle"
                        aria-label={`${card.displayName} leadini sürükle`}
                        draggable={mutatingId === null}
                        title="Sürükleyerek başka aşamaya taşı"
                        onDragStart={(event) => {
                          event.dataTransfer.effectAllowed = "move";
                          event.dataTransfer.setData("text/plain", card.id);
                          setDragging({ id: card.id, status: card.status });
                        }}
                        onDragEnd={() => {
                          setDragging(null);
                          setDragOverStatus(null);
                        }}
                      >
                        ⋮⋮
                      </button>
                      <Link to={`/leads/${card.id}`}>{card.displayName}</Link>
                      {card.isRepeat ? <span className="pipeline-repeat">Repeat ×{card.submissionCount}</span> : null}
                    </div>

                    <div className="pipeline-card-contact">
                      {card.primaryEmail ?? card.primaryPhone ?? "İletişim bilgisi yok"}
                    </div>

                    <div className="pipeline-card-meta">
                      <span>{formatCountry(card.countryCode)}</span>
                      <span>{formatDate(card.latestSubmissionAt)}</span>
                    </div>

                    <div className="pipeline-card-chips">
                      {card.productInterests.slice(0, 2).map((code) => (
                        <span className="lead-product-chip" key={code}>{productLabels[code]}</span>
                      ))}
                      {card.productInterests.length > 2 ? (
                        <span className="pipeline-more-chip">+{card.productInterests.length - 2}</span>
                      ) : null}
                      {card.platforms.map((platform) => (
                        <span className="lead-platform-chip" key={platform}>{platformLabel(platform)}</span>
                      ))}
                      {card.warningCount > 0 ? <span className="pipeline-warning">⚠ {card.warningCount}</span> : null}
                    </div>

                    <label className="pipeline-card-status">
                      <span>Durum</span>
                      <select
                        value={card.status}
                        disabled={mutatingId !== null}
                        onChange={(event) => void changeStatus(card.id, event.target.value as LeadStatus)}
                      >
                        {statusOptions.map(([value, label]) => (
                          <option value={value} key={value}>{label}</option>
                        ))}
                      </select>
                    </label>
                  </article>
                ))}

                {!loading && column.cards.length === 0 ? (
                  <div className="pipeline-column-empty">Bu aşamada lead yok.</div>
                ) : null}

                {column.truncated ? (
                  <div className="pipeline-column-truncated">
                    İlk {column.cards.length} / {column.total} kayıt gösteriliyor.
                  </div>
                ) : null}
              </div>
            </fieldset>
          ))}
        </div>

        {loading && !board ? <div className="pipeline-loading">Pipeline yükleniyor…</div> : null}
      </div>
    </section>
  );
}
