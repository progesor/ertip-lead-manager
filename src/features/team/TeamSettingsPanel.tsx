import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState, type FormEvent } from "react";
import type { CommandError } from "../leads/types";
import type { StaffMember, StaffMemberInput, StaffRole } from "./types";

const roleLabels: Record<StaffRole, string> = {
  ADMIN: "Yönetici",
  MANAGER: "Ekip Yöneticisi",
  SALES: "Satış Personeli",
};

const roles = Object.entries(roleLabels) as Array<[StaffRole, string]>;

function commandErrorMessage(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    return String((error as CommandError).message);
  }
  if (typeof error === "string") return error;
  return "Personel işlemi tamamlanamadı.";
}

function emptyInput(): StaffMemberInput {
  return { displayName: "", email: null, role: "SALES" };
}

export function TeamSettingsPanel() {
  const [members, setMembers] = useState<StaffMember[]>([]);
  const [draft, setDraft] = useState<StaffMemberInput>(emptyInput());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editing, setEditing] = useState<StaffMemberInput>(emptyInput());
  const [loading, setLoading] = useState(true);
  const [mutating, setMutating] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const loadMembers = useCallback(async () => {
    setLoading(true);
    try {
      const response = await invoke<StaffMember[]>("list_staff_members", { includeInactive: true });
      setMembers(response);
    } catch (loadError) {
      setError(commandErrorMessage(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadMembers();
  }, [loadMembers]);

  async function createMember(event: FormEvent) {
    event.preventDefault();
    if (!draft.displayName.trim()) return;
    setMutating("create");
    setError(null);
    setNotice(null);
    try {
      await invoke<string>("create_staff_member", {
        input: {
          ...draft,
          email: draft.email?.trim() || null,
        },
      });
      setDraft(emptyInput());
      setNotice("Personel eklendi.");
      await loadMembers();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  function startEdit(member: StaffMember) {
    setEditingId(member.id);
    setEditing({
      displayName: member.displayName,
      email: member.email,
      role: member.role,
    });
    setError(null);
    setNotice(null);
  }

  async function saveEdit(member: StaffMember) {
    if (!editing.displayName.trim()) return;
    setMutating(`edit:${member.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke("update_staff_member", {
        userId: member.id,
        input: {
          ...editing,
          email: editing.email?.trim() || null,
        },
      });
      setEditingId(null);
      setNotice("Personel bilgileri güncellendi.");
      await loadMembers();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  async function toggleActive(member: StaffMember) {
    setMutating(`active:${member.id}`);
    setError(null);
    setNotice(null);
    try {
      await invoke("set_staff_member_active", {
        userId: member.id,
        isActive: !member.isActive,
      });
      setNotice(member.isActive ? "Personel pasife alındı. Geçmiş atamalar korunuyor." : "Personel yeniden aktifleştirildi.");
      await loadMembers();
    } catch (mutationError) {
      setError(commandErrorMessage(mutationError));
    } finally {
      setMutating(null);
    }
  }

  const activeCount = members.filter((member) => member.isActive).length;

  return (
    <article className="panel team-settings-panel">
      <div className="panel-heading team-settings-heading">
        <div>
          <h2>Ekip & Personel</h2>
          <p>Lead sorumlularını yönetin. Personel silinmez; pasife alınarak geçmiş atamalar korunur.</p>
        </div>
        <span className="placeholder-pill">{activeCount} aktif</span>
      </div>

      <div className="team-settings-note">
        <strong>Local çalışma modu</strong>
        <span>Bu kayıtlar şimdilik atama için kullanılıyor. Online sürümde login hesabı aynı personel kimliğine bağlanacak.</span>
      </div>

      {error ? <div className="import-error" role="alert">{error}</div> : null}
      {notice ? <div className="import-success" role="status">{notice}</div> : null}

      <form className="team-create-form" onSubmit={createMember}>
        <label>
          <span>Ad Soyad</span>
          <input
            type="text"
            value={draft.displayName}
            maxLength={100}
            required
            onChange={(event) => setDraft((value) => ({ ...value, displayName: event.target.value }))}
            placeholder="Örn. Ayşe Yılmaz"
            disabled={mutating !== null}
          />
        </label>
        <label>
          <span>E-posta</span>
          <input
            type="email"
            value={draft.email ?? ""}
            maxLength={254}
            onChange={(event) => setDraft((value) => ({ ...value, email: event.target.value }))}
            placeholder="Opsiyonel"
            disabled={mutating !== null}
          />
        </label>
        <label>
          <span>Rol</span>
          <select
            value={draft.role}
            onChange={(event) => setDraft((value) => ({ ...value, role: event.target.value as StaffRole }))}
            disabled={mutating !== null}
          >
            {roles.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
        </label>
        <button className="primary-button team-create-button" type="submit" disabled={mutating !== null || !draft.displayName.trim()}>
          {mutating === "create" ? "Ekleniyor…" : "Personel Ekle"}
        </button>
      </form>

      <div className="team-member-list" aria-busy={loading}>
        {members.map((member) => {
          const isEditing = editingId === member.id;
          return (
            <div className={`team-member-row ${member.isActive ? "" : "is-inactive"}`} key={member.id}>
              {isEditing ? (
                <div className="team-member-edit-grid">
                  <label>
                    <span>Ad Soyad</span>
                    <input
                      value={editing.displayName}
                      maxLength={100}
                      onChange={(event) => setEditing((value) => ({ ...value, displayName: event.target.value }))}
                      disabled={mutating !== null}
                    />
                  </label>
                  <label>
                    <span>E-posta</span>
                    <input
                      type="email"
                      value={editing.email ?? ""}
                      maxLength={254}
                      onChange={(event) => setEditing((value) => ({ ...value, email: event.target.value }))}
                      disabled={mutating !== null}
                    />
                  </label>
                  <label>
                    <span>Rol</span>
                    <select
                      value={editing.role}
                      onChange={(event) => setEditing((value) => ({ ...value, role: event.target.value as StaffRole }))}
                      disabled={mutating !== null}
                    >
                      {roles.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
                    </select>
                  </label>
                </div>
              ) : (
                <div className="team-member-copy">
                  <div>
                    <strong>{member.displayName}</strong>
                    <span className={`team-member-state ${member.isActive ? "is-active" : "is-inactive"}`}>
                      {member.isActive ? "Aktif" : "Pasif"}
                    </span>
                  </div>
                  <span>{member.email ?? "E-posta tanımlı değil"}</span>
                  <small>{roleLabels[member.role]} · {member.authSubject ? "Login bağlı" : "Login henüz bağlı değil"}</small>
                </div>
              )}

              <div className="team-member-actions">
                {isEditing ? (
                  <>
                    <button type="button" onClick={() => setEditingId(null)} disabled={mutating !== null}>Vazgeç</button>
                    <button type="button" className="is-primary" onClick={() => void saveEdit(member)} disabled={mutating !== null || !editing.displayName.trim()}>
                      {mutating === `edit:${member.id}` ? "Kaydediliyor…" : "Kaydet"}
                    </button>
                  </>
                ) : (
                  <>
                    <button type="button" onClick={() => startEdit(member)} disabled={mutating !== null}>Düzenle</button>
                    <button
                      type="button"
                      className={member.isActive ? "is-danger" : "is-primary"}
                      onClick={() => void toggleActive(member)}
                      disabled={mutating !== null}
                    >
                      {mutating === `active:${member.id}`
                        ? "Kaydediliyor…"
                        : member.isActive
                          ? "Pasife Al"
                          : "Aktifleştir"}
                    </button>
                  </>
                )}
              </div>
            </div>
          );
        })}

        {!loading && members.length === 0 ? (
          <div className="team-empty">Henüz personel eklenmedi.</div>
        ) : null}
      </div>
    </article>
  );
}
