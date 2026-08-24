import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useMemo, useState } from "react";
import type { CommandError } from "./types";
import "./follow-ups.css";

export type FollowUpStatus = "OPEN" | "COMPLETED" | "CANCELLED";

export interface FollowUpItem {
  id: string;
  leadContactId: string;
  dueAt: string;
  status: FollowUpStatus;
  note: string | null;
  createdAt: string;
  completedAt: string | null;
}

interface FollowUpPanelProps {
  contactId: string;
  onChanged?: () => void;
}

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Takip işlemi tamamlanamadı.";
}

function pad(value: number) {
  return value.toString().padStart(2, "0");
}

function toLocalInputValue(date: Date) {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function initialDueValue() {
  const date = new Date();
  date.setDate(date.getDate() + 1);
  date.setHours(10, 0, 0, 0);
  return toLocalInputValue(date);
}

function localInputToUtc(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new Error("Geçerli bir takip tarihi seçin.");
  }
  return date.toISOString();
}

function utcToLocalInput(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "" : toLocalInputValue(date);
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

function localDayKey(date: Date) {
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function dueClass(item: FollowUpItem) {
  if (item.status !== "OPEN") return item.status.toLowerCase();
  const due = new Date(item.dueAt);
  if (Number.isNaN(due.getTime())) return "upcoming";
  const now = new Date();
  if (due.getTime() < now.getTime()) return "overdue";
  if (localDayKey(due) === localDayKey(now)) return "today";
  return "upcoming";
}

function dueLabel(item: FollowUpItem) {
  if (item.status === "COMPLETED") return "Tamamlandı";
  if (item.status === "CANCELLED") return "İptal edildi";
  const kind = dueClass(item);
  if (kind === "overdue") return "Gecikmiş";
  if (kind === "today") return "Bugün";
  return "Planlandı";
}

export function FollowUpPanel({ contactId, onChanged }: FollowUpPanelProps) {
  const [items, setItems] = useState<FollowUpItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [dueDraft, setDueDraft] = useState(initialDueValue);
  const [noteDraft, setNoteDraft] = useState("");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingDue, setEditingDue] = useState("");
  const [editingNote, setEditingNote] = useState("");
  const [mutating, setMutating] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await invoke<FollowUpItem[]>("list_lead_follow_ups", { contactId });
      setItems(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, [contactId]);

  useEffect(() => {
    void load();
  }, [load]);

  const openItems = useMemo(() => items.filter((item) => item.status === "OPEN"), [items]);
  const historyItems = useMemo(() => items.filter((item) => item.status !== "OPEN"), [items]);

  async function afterMutation(message: string) {
    setNotice(message);
    await load();
    onChanged?.();
  }

  async function createFollowUp() {
    if (!dueDraft) return;
    setMutating("create");
    setError(null);
    setNotice(null);
    try {
      await invoke<string>("create_lead_follow_up", {
        contactId,
        dueAt: localInputToUtc(dueDraft),
        note: noteDraft.trim() || null,
      });
      setDueDraft(initialDueValue());
      setNoteDraft("");
      await afterMutation("Takip planlandı.");
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  function beginEdit(item: FollowUpItem) {
    setEditingId(item.id);
    setEditingDue(utcToLocalInput(item.dueAt));
    setEditingNote(item.note ?? "");
    setError(null);
    setNotice(null);
  }

  async function saveReschedule(item: FollowUpItem) {
    if (!editingDue) return;
    setMutating(`reschedule:${item.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke<boolean>("reschedule_lead_follow_up", {
        contactId,
        followUpId: item.id,
        dueAt: localInputToUtc(editingDue),
        note: editingNote.trim() || null,
      });
      setEditingId(null);
      setEditingDue("");
      setEditingNote("");
      await afterMutation("Takip zamanı güncellendi.");
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function complete(item: FollowUpItem) {
    setMutating(`complete:${item.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke<boolean>("complete_lead_follow_up", {
        contactId,
        followUpId: item.id,
      });
      if (editingId === item.id) setEditingId(null);
      await afterMutation("Takip tamamlandı.");
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function cancel(item: FollowUpItem) {
    const accepted = await confirm("Bu takip iptal edilecek. Aktivite geçmişi korunacak. Devam edilsin mi?", {
      title: "Takibi İptal Et",
      kind: "warning",
      okLabel: "İptal Et",
      cancelLabel: "Vazgeç",
    });
    if (!accepted) return;

    setMutating(`cancel:${item.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke<boolean>("cancel_lead_follow_up", {
        contactId,
        followUpId: item.id,
      });
      if (editingId === item.id) setEditingId(null);
      await afterMutation("Takip iptal edildi.");
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  return (
    <section className="follow-up-page-slot">
      <article className="panel follow-up-panel">
        <div className="panel-heading">
          <div>
            <h2>Takip Planı</h2>
            <p>Arama, teklif veya yeniden iletişim için tarih/saat planlayın.</p>
          </div>
          <span className="placeholder-pill">{openItems.length} açık</span>
        </div>

        {error ? <div className="import-error" role="alert">{error}</div> : null}
        {notice ? <div className="import-success" role="status">{notice}</div> : null}

        <div className="follow-up-compose">
          <label>
            <span>Tarih / Saat</span>
            <input
              type="datetime-local"
              value={dueDraft}
              onChange={(event) => setDueDraft(event.target.value)}
              disabled={mutating !== null}
            />
          </label>
          <label className="follow-up-compose-note">
            <span>Kısa Not</span>
            <input
              value={noteDraft}
              maxLength={1000}
              onChange={(event) => setNoteDraft(event.target.value)}
              placeholder="Örn. fiyat teklifini tekrar sor"
              disabled={mutating !== null}
            />
          </label>
          <button type="button" onClick={createFollowUp} disabled={mutating !== null || !dueDraft}>
            {mutating === "create" ? "Planlanıyor…" : "Takip Planla"}
          </button>
        </div>

        <div className="follow-up-section-heading">
          <strong>Açık Takipler</strong>
          {loading ? <span>Yükleniyor…</span> : <span>{openItems.length} kayıt</span>}
        </div>

        {openItems.length > 0 ? (
          <div className="follow-up-list">
            {openItems.map((item) => {
              const editing = editingId === item.id;
              return (
                <article className={`follow-up-card follow-up-${dueClass(item)}`} key={item.id}>
                  <div className="follow-up-card-head">
                    <div>
                      <span className={`follow-up-state follow-up-state-${dueClass(item)}`}>{dueLabel(item)}</span>
                      <strong>{formatDate(item.dueAt)}</strong>
                    </div>
                    <div className="follow-up-card-actions">
                      <button type="button" onClick={() => beginEdit(item)} disabled={mutating !== null}>Düzenle</button>
                      <button type="button" className="is-primary" onClick={() => void complete(item)} disabled={mutating !== null}>Tamamla</button>
                      <button type="button" className="is-danger" onClick={() => void cancel(item)} disabled={mutating !== null}>İptal</button>
                    </div>
                  </div>

                  {editing ? (
                    <div className="follow-up-edit-grid">
                      <label>
                        <span>Yeni Tarih / Saat</span>
                        <input type="datetime-local" value={editingDue} onChange={(event) => setEditingDue(event.target.value)} />
                      </label>
                      <label>
                        <span>Not</span>
                        <input value={editingNote} maxLength={1000} onChange={(event) => setEditingNote(event.target.value)} />
                      </label>
                      <div>
                        <button type="button" className="is-primary" onClick={() => void saveReschedule(item)} disabled={!editingDue || mutating !== null}>Kaydet</button>
                        <button type="button" onClick={() => setEditingId(null)} disabled={mutating !== null}>Vazgeç</button>
                      </div>
                    </div>
                  ) : item.note ? (
                    <p>{item.note}</p>
                  ) : (
                    <p className="follow-up-no-note">Not eklenmemiş.</p>
                  )}
                </article>
              );
            })}
          </div>
        ) : !loading ? (
          <div className="follow-up-empty">Açık takip yok.</div>
        ) : null}

        {historyItems.length > 0 ? (
          <details className="follow-up-history">
            <summary>Geçmiş takipler · {historyItems.length}</summary>
            <div className="follow-up-history-list">
              {historyItems.map((item) => (
                <div className="follow-up-history-row" key={item.id}>
                  <span className={`follow-up-state follow-up-state-${dueClass(item)}`}>{dueLabel(item)}</span>
                  <strong>{formatDate(item.dueAt)}</strong>
                  <span>{item.note ?? "—"}</span>
                </div>
              ))}
            </div>
          </details>
        ) : null}
      </article>
    </section>
  );
}
