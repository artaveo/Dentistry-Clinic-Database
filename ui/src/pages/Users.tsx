import { useEffect, useState } from "react";
import type { RoleInfo, UserInfo } from "../../../shared/ts/contract";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";

export function UsersPage() {
  const { t, err } = useI18n();
  const [users, setUsers] = useState<UserInfo[]>([]);
  const [roles, setRoles] = useState<RoleInfo[]>([]);
  const [form, setForm] = useState({ username: "", display_name: "", password: "", role: "receptionist" });
  const [edit, setEdit] = useState<UserInfo | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  const load = () => Promise.all([rpc("users.list", {}), rpc("roles.list", {})]).then(([u, r]) => { setUsers(u); setRoles(r); });
  useEffect(() => {
    load().catch((e) => setMsg({ ok: false, text: err(e) }));
  }, []);
  const assignable = roles.filter((r) => r.code !== "owner");

  const create = async (e: React.FormEvent) => {
    e.preventDefault();
    setMsg(null);
    try {
      await rpc("users.create", form);
      setForm({ username: "", display_name: "", password: "", role: form.role });
      setMsg({ ok: true, text: t("users.created") });
      await load();
    } catch (x) {
      setMsg({ ok: false, text: err(x) });
    }
  };

  const save = async () => {
    if (!edit) return;
    setMsg(null);
    try {
      // `version` makes a concurrent edit by someone else fail instead of being overwritten.
      await rpc("users.update", { id: edit.id, version: edit.version, display_name: edit.display_name, role: edit.role, is_active: edit.is_active });
      setEdit(null);
      setMsg({ ok: true, text: t("common.saved") });
      await load();
    } catch (x) {
      setMsg({ ok: false, text: err(x) });
    }
  };

  return (
    <div className="card wide">
      <h2>{t("users.title")}</h2>
      <table data-testid="user-list">
        <thead><tr><th>{t("login.username")}</th><th>{t("login.displayName")}</th><th>{t("users.role")}</th><th>{t("users.active")}</th><th /></tr></thead>
        <tbody>
          {users.map((u) =>
            edit?.id === u.id ? (
              <tr key={u.id}>
                <td className="ltr">{u.username}</td>
                <td><input value={edit.display_name} onChange={(e) => setEdit({ ...edit, display_name: e.target.value })} /></td>
                <td>
                  {u.role === "owner" ? t("role.owner") : (
                    <select value={edit.role} onChange={(e) => setEdit({ ...edit, role: e.target.value })}>
                      {assignable.map((r) => <option key={r.code} value={r.code}>{t(`role.${r.code}`)}</option>)}
                    </select>
                  )}
                </td>
                <td><input type="checkbox" disabled={u.role === "owner"} checked={edit.is_active} onChange={(e) => setEdit({ ...edit, is_active: e.target.checked })} /></td>
                <td className="row"><button className="primary" onClick={save}>{t("common.save")}</button><button onClick={() => setEdit(null)}>{t("common.cancel")}</button></td>
              </tr>
            ) : (
              <tr key={u.id}>
                <td className="ltr">{u.username}</td>
                <td>{u.display_name}</td>
                <td>{t(`role.${u.role}`)}</td>
                <td>{u.is_active ? t("common.yes") : t("common.no")}</td>
                <td><button onClick={() => setEdit(u)}>{t("users.edit")}</button></td>
              </tr>
            ),
          )}
        </tbody>
      </table>
      {msg && <div className={msg.ok ? "success" : "error"} data-testid="users-message">{msg.text}</div>}
      <form onSubmit={create} style={{ marginTop: 16 }}>
        <h2>{t("users.add")}</h2>
        <label htmlFor="nu">{t("login.username")}</label>
        <input id="nu" className="ltr" value={form.username} onChange={(e) => setForm({ ...form, username: e.target.value })} data-testid="new-username" />
        <label htmlFor="nd">{t("login.displayName")}</label>
        <input id="nd" value={form.display_name} onChange={(e) => setForm({ ...form, display_name: e.target.value })} data-testid="new-display" />
        <label htmlFor="np">{t("login.password")}</label>
        <input id="np" type="password" value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} data-testid="new-password" />
        <label htmlFor="nr">{t("users.role")}</label>
        <select id="nr" value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })} data-testid="new-role">
          {assignable.map((r) => <option key={r.code} value={r.code}>{t(`role.${r.code}`)}</option>)}
        </select>
        <div className="actions"><button className="primary" type="submit" data-testid="create-user">{t("users.add")}</button></div>
      </form>
    </div>
  );
}
