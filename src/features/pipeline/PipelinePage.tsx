import { invoke } from "@tauri-apps/api/core";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { useLocation, useNavigate } from "react-router-dom";
import type {
  CommandError,
  LeadFilterOptions,
  LeadStatus,
  ProductCode,
} from "../leads/types";
import "./pipeline.css";
import "./pipeline-drag-preview.css";
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
const DRAG_THRESHOLD_PX = 7;

type FollowUpMode = "" | "OVERDUE" | "TODAY";

interface PipelineRestoreState {
  search: string;
  country: string;
  product: ProductCode | "";
  repeatOnly: boolean;
  warningOnly: boolean;
  includeTerminal: boolean;
  followUpMode: FollowUpMode;
}

interface PipelineRouteState {
  restorePipeline?: PipelineRestoreState;
}

interface CardPointerDrag {
  pointerId: number;
  contactId: string;
  sourceStatus: LeadStatus;
  startX: number;
  startY: number;
  offsetX: number;
  offsetY: number;
  width: number;
  card: PipelineCard;
  moved: boolean;
}

interface DragPreview {
  card: PipelineCard;
  x: number;
  y: number;
  width: number;
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

function localDayKey(date: Date) {
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function followUpKind(value: string) {
  const due = new Date(value);
  if (Number.isNaN(due.getTime())) return "upcoming";
  const now = new Date();
  if (due.getTime() < now.getTime()) return "overdue";
  if (localDayKey(due) === localDayKey(now)) return "today";
  return "upcoming";
}

function followUpLabel(value: string) {
  const kind = followUpKind(value);
  if (kind === "overdue") return `Gecikmiş · ${formatDate(value)}`;
  if (kind === "today") return `Bugün · ${formatDate(value)}`;
  return `Takip · ${formatDate(value)}`;
}

function pipelineTimeWindow() {
  const now = new Date();
  const tomorrowStart = new Date(
    now.getFullYear(),
    now.getMonth(),
    now.getDate() + 1,
    0,
    0,
    0,
    0,
  );
  return {
    nowUtc: now.toISOString(),
    tomorrowStartUtc: tomorrowStart.toISOString(),
  };
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

function pipelineStatusAtPoint(x: number, y: number): LeadStatus | null {
  const target = document.elementFromPoint(x, y);
  const column = target?.closest<HTMLElement>("[data-pipeline-status]");
  const status = column?.dataset.pipelineStatus as LeadStatus | undefined;
  return status && status in statusLabels ? status : null;
}

function moveCardLocally(
  board: PipelineBoardResponse,
  contactId: string,
  targetStatus: LeadStatus,
): PipelineBoardResponse {
  const sourceColumn = board.columns.find((column) =>
    column.cards.some((item) => item.id === contactId),
  );
  const movedCard = sourceColumn?.cards.find((item) => item.id === contactId);
  if (!sourceColumn || !movedCard) return board;

  const withoutSource = board.columns.map((column) => {
    if (column.status !== sourceColumn.status) return column;
    return {
      ...column,
      total: Math.max(0, column.total - 1),
      cards: column.cards.filter((item) => item.id !== contactId),
    };
  });

  let targetVisible = false;
  const nextColumns = withoutSource.map((column) => {
    if (column.status !== targetStatus) return column;
    targetVisible = true;

    const nextCards = [
      { ...movedCard, status: targetStatus },
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
    visibleTotal: board.visibleTotal + (targetVisible ? 1 : 0) - 1,
  };
}

function PipelineCardBody({ card }: { card: PipelineCard }) {
  return (
    <>
      <div className="pipeline-card-topline">
        <span className="pipeline-card-name">{card.displayName}</span>
        {card.isRepeat ? (
          <span className="pipeline-repeat">Repeat ×{card.submissionCount}</span>
        ) : null}
      </div>

      <div className="pipeline-card-contact">
        {card.primaryPhone ?? "Telefon bilgisi yok"}
      </div>

      <div className="pipeline-card-meta">
        <span>{formatCountry(card.countryCode)}</span>
        <span>{formatDate(card.latestSubmissionAt)}</span>
      </div>

      {card.primaryEmail ? (
        <div className="pipeline-card-email">{card.primaryEmail}</div>
      ) : null}

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
        {card.warningCount > 0 ? (
          <span className="pipeline-warning">⚠ {card.warningCount}</span>
        ) : null}
      </div>

      {card.nextFollowUpAt ? (
        <div className={`pipeline-follow-up pipeline-follow-up-${followUpKind(card.nextFollowUpAt)}`}>
          <span>{followUpLabel(card.nextFollowUpAt)}</span>
          {card.openFollowUpCount > 1 ? (
            <strong>+{card.openFollowUpCount - 1}</strong>
          ) : null}
        </div>
      ) : null}
    </>
  );
}

export function PipelinePage() {
  const navigate = useNavigate();
  const location = useLocation();
  const initialStateRef = useRef<PipelineRestoreState | undefined>(
    (location.state as PipelineRouteState | null)?.restorePipeline,
  );
  const pointerDragRef = useRef<CardPointerDrag | null>(null);
  const suppressClickRef = useRef(false);
  const [board, setBoard] = useState<PipelineBoardResponse | null>(null);
  const [filterOptions, setFilterOptions] = useState<LeadFilterOptions>(emptyFilters);
  const [search, setSearch] = useState(initialStateRef.current?.search ?? "");
  const [country, setCountry] = useState(initialStateRef.current?.country ?? "");
  const [product, setProduct] = useState<ProductCode | "">(
    initialStateRef.current?.product ?? "",
  );
  const [repeatOnly, setRepeatOnly] = useState(initialStateRef.current?.repeatOnly ?? false);
  const [warningOnly, setWarningOnly] = useState(initialStateRef.current?.warningOnly ?? false);
  const [includeTerminal, setIncludeTerminal] = useState(
    initialStateRef.current?.includeTerminal ?? false,
  );
  const [followUpMode, setFollowUpMode] = useState<FollowUpMode>(
    initialStateRef.current?.followUpMode ?? "",
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dragOverStatus, setDragOverStatus] = useState<LeadStatus | null>(null);
  const [dragPreview, setDragPreview] = useState<DragPreview | null>(null);
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
      const timeWindow = pipelineTimeWindow();
      const response = await invoke<PipelineBoardResponse>("get_pipeline_board", {
        request: {
          search: clean(search),
          countryCode: country || null,
          productCode: product || null,
          repeatOnly,
          warningOnly,
          includeTerminal,
          followUpMode: followUpMode || null,
          nowUtc: timeWindow.nowUtc,
          tomorrowStartUtc: timeWindow.tomorrowStartUtc,
          perColumnLimit: 100,
        },
      });
      setBoard(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, [country, followUpMode, includeTerminal, product, repeatOnly, search, warningOnly]);

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
    const sourceCard = board.columns
      .flatMap((column) => column.cards)
      .find((card) => card.id === contactId);
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
      setDraggingId(null);
      setDragOverStatus(null);
      setDragPreview(null);
    }
  }

  function beginCardPointerDrag(
    event: ReactPointerEvent<HTMLButtonElement>,
    card: PipelineCard,
  ) {
    if (event.button !== 0 || mutatingId !== null) return;

    const bounds = event.currentTarget.getBoundingClientRect();
    pointerDragRef.current = {
      pointerId: event.pointerId,
      contactId: card.id,
      sourceStatus: card.status,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: event.clientX - bounds.left,
      offsetY: event.clientY - bounds.top,
      width: bounds.width,
      card,
      moved: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveCardPointer(event: ReactPointerEvent<HTMLButtonElement>) {
    const drag = pointerDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    if (!drag.moved) {
      const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
      if (distance < DRAG_THRESHOLD_PX) return;
      drag.moved = true;
      suppressClickRef.current = true;
      setDraggingId(drag.contactId);
    }

    event.preventDefault();
    setDragPreview({
      card: drag.card,
      x: event.clientX - drag.offsetX,
      y: event.clientY - drag.offsetY,
      width: drag.width,
    });

    const targetStatus = pipelineStatusAtPoint(event.clientX, event.clientY);
    setDragOverStatus(
      targetStatus && targetStatus !== drag.sourceStatus ? targetStatus : null,
    );
  }

  function endCardPointer(event: ReactPointerEvent<HTMLButtonElement>) {
    const drag = pointerDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;

    const targetStatus = drag.moved
      ? pipelineStatusAtPoint(event.clientX, event.clientY)
      : null;

    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    pointerDragRef.current = null;
    setDraggingId(null);
    setDragOverStatus(null);
    setDragPreview(null);

    if (!drag.moved) return;

    event.preventDefault();
    if (targetStatus && targetStatus !== drag.sourceStatus) {
      void changeStatus(drag.contactId, targetStatus);
    }

    window.setTimeout(() => {
      suppressClickRef.current = false;
    }, 0);
  }

  function cancelCardPointer(event: ReactPointerEvent<HTMLButtonElement>) {
    const drag = pointerDragRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    pointerDragRef.current = null;
    suppressClickRef.current = true;
    setDraggingId(null);
    setDragOverStatus(null);
    setDragPreview(null);
    window.setTimeout(() => {
      suppressClickRef.current = false;
    }, 0);
  }

  function openLead(event: ReactMouseEvent<HTMLButtonElement>, contactId: string) {
    if (suppressClickRef.current) {
      event.preventDefault();
      suppressClickRef.current = false;
      return;
    }

    navigate(`/leads/${contactId}`, {
      state: {
        returnTo: "/pipeline",
        returnLabel: "Pipeline'a Dön",
        returnState: {
          restorePipeline: {
            search,
            country,
            product,
            repeatOnly,
            warningOnly,
            includeTerminal,
            followUpMode,
          } satisfies PipelineRestoreState,
        },
      },
    });
  }

  function clearFilters() {
    setSearch("");
    setCountry("");
    setProduct("");
    setRepeatOnly(false);
    setWarningOnly(false);
    setFollowUpMode("");
  }

  const hasFilters = Boolean(
    search || country || product || repeatOnly || warningOnly || followUpMode,
  );

  return (
    <section className="page-stack pipeline-page">
      <div className="page-heading pipeline-heading">
        <div>
          <div className="eyebrow">LIFECYCLE</div>
          <h1>Pipeline</h1>
          <p>Karta tıklayarak detayı aç; kartı tutup sürüklediğinde fareyle birlikte hareket eder.</p>
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
            placeholder="Ad, telefon, e-posta veya Meta Lead ID"
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
            className={followUpMode === "OVERDUE" ? "is-active" : ""}
            aria-pressed={followUpMode === "OVERDUE"}
            onClick={() =>
              setFollowUpMode((value) => (value === "OVERDUE" ? "" : "OVERDUE"))
            }
          >
            Gecikmiş
          </button>
          <button
            type="button"
            className={followUpMode === "TODAY" ? "is-active" : ""}
            aria-pressed={followUpMode === "TODAY"}
            onClick={() =>
              setFollowUpMode((value) => (value === "TODAY" ? "" : "TODAY"))
            }
          >
            Bugün Takip
          </button>
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
          {hasFilters ? (
            <button type="button" className="is-clear" onClick={clearFilters}>
              Filtreleri Temizle
            </button>
          ) : null}
        </div>
      </article>

      <div className="pipeline-board-wrap" aria-busy={loading}>
        <div className="pipeline-board">
          {(board?.columns ?? []).map((column) => (
            <fieldset
              className={`pipeline-column ${dragOverStatus === column.status ? "is-drag-over" : ""}`}
              key={column.status}
              aria-label={`${statusLabels[column.status]} pipeline kolonu`}
              data-pipeline-status={column.status}
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
                  <button
                    type="button"
                    className={`pipeline-card ${mutatingId === card.id ? "is-mutating" : ""} ${draggingId === card.id ? "is-dragging" : ""}`}
                    key={card.id}
                    disabled={mutatingId !== null}
                    title="Tıkla: lead detayı · Sürükle: aşamayı değiştir"
                    onPointerDown={(event) => beginCardPointerDrag(event, card)}
                    onPointerMove={moveCardPointer}
                    onPointerUp={endCardPointer}
                    onPointerCancel={cancelCardPointer}
                    onClick={(event) => openLead(event, card.id)}
                  >
                    <PipelineCardBody card={card} />
                  </button>
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

      {dragPreview ? (
        <div
          className="pipeline-drag-preview"
          aria-hidden="true"
          style={{
            width: dragPreview.width,
            transform: `translate3d(${dragPreview.x}px, ${dragPreview.y}px, 0)`,
          }}
        >
          <div className="pipeline-card pipeline-drag-preview-card">
            <PipelineCardBody card={dragPreview.card} />
          </div>
        </div>
      ) : null}
    </section>
  );
}
