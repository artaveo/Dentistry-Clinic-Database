import { useEffect, useState } from "react";
import { Crown, IdCard, LockKeyhole, Pencil, ShieldCheck, UserPlus, UserRound, Users } from "lucide-react";
import type { RoleInfo, UserInfo } from "../../../shared/ts/contract";
import { isSessionError, rpc } from "../lib/api";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, Page, PageHeader } from "../ui/Card";
import { Switch } from "../ui/Controls";
import { Field, PasswordInput, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { Avatar } from "../ui/Brand";
import { useToast } from "../ui/Toast";

export function UsersPage({ currentUserId }: { currentUserId: string }) {
  const { t, err } = useI18n();
  const [users, setUsers] = useState<UserInfo[] | null>(null);
  const [roles, setRoles] = useState<RoleInfo[]>([]);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<UserInfo | null>(null);

  const load = () =>
    Promise.all([rpc("users.list", {}), rpc("roles.list", {})])
      .then(([u, r]) => {
        setUsers(u);
        setRoles(r);
        setError("");
      })
      .catch((e) => !isSessionError(e) && setError(err(e)));
  useEffect(() => {
    load();
  }, []);
  const assignable = roles.filter((r) => r.code !== "owner");

  return (
    <Page testId="page-users">
      <PageHeader
        title={t("users.title")}
        description={t("users.subtitle")}
        actions={<Button variant="primary" icon={UserPlus} onClick={() => setCreating(true)} data-testid="add-user-open">{t("users.add")}</Button>}
      />
      <Card flush>
        {error ? (
          <ErrorState message={error} onRetry={load} />
        ) : (
          <div className="table-wrap">
            <table className="table" data-testid="user-list">
              <thead>
                <tr><th>{t("users.person")}</th><th>{t("login.username")}</th><th>{t("users.role")}</th><th>{t("users.status")}</th><th className="cell-actions"><span className="visually-hidden">{t("users.edit")}</span></th></tr>
              </thead>
              <tbody>
                {!users && <SkeletonRows cols={5} />}
                {users?.map((u) => (
                  <tr key={u.id}>
                    <td>
                      <div className="person">
                        <Avatar name={u.display_name} />
                        <div className="person-text">
                          <span className="cell-strong">{u.display_name}</span>
                          {u.id === currentUserId && <span className="person-sub">{t("users.you")}</span>}
                        </div>
                      </div>
                    </td>
                    <td><bdi className="ltr">{u.username}</bdi></td>
                    <td>{u.role === "owner" ? <Badge tone="accent" icon={Crown}>{t("role.owner")}</Badge> : <Badge>{t(`role.${u.role}`)}</Badge>}</td>
                    <td>{u.is_active ? <Badge tone="success" dot>{t("users.active")}</Badge> : <Badge dot>{t("users.inactive")}</Badge>}</td>
                    <td className="cell-actions"><IconButton icon={Pencil} label={t("users.edit")} size="sm" onClick={() => setEditing(u)} data-testid={`edit-user-${u.username}`} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
            {users?.length === 0 && <EmptyState icon={Users} title={t("users.empty")} />}
          </div>
        )}
      </Card>
      {creating && <CreateUserDialog roles={assignable} onClose={() => setCreating(false)} onCreated={load} />}
      {editing && <EditUserDialog user={editing} roles={assignable} onClose={() => setEditing(null)} onSaved={load} />}
    </Page>
  );
}

function CreateUserDialog({ roles, onClose, onCreated }: { roles: RoleInfo[]; onClose: () => void; onCreated: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const form = useForm(
    { username: "", display_name: "", password: "", role: "receptionist" },
    { username: v.username, display_name: v.displayName, password: v.password },
  );
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const e = (k: "username" | "display_name" | "password" | "role") => form.error(k) && t(form.error(k)!);

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      await rpc("users.create", form.values);
      toast.success(t("users.created"));
      onCreated();
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("users.add")}
      description={t("users.addHint")}
      onClose={onClose}
      testId="create-user-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={UserPlus} loading={busy} type="submit" form="create-user-form" data-testid="create-user">{t("users.add")}</Button>
        </>
      }
    >
      <form id="create-user-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Field label={t("login.displayName")} hint={t("hint.displayName")} error={e("display_name")}>
          <TextInput icon={IdCard} value={form.values.display_name} onChange={(x) => form.set("display_name", x.target.value)} onBlur={() => form.blur("display_name")} data-testid="new-display" />
        </Field>
        <Field label={t("login.username")} hint={t("hint.username")} error={e("username")}>
          <TextInput icon={UserRound} dir="ltr" autoComplete="off" value={form.values.username} onChange={(x) => form.set("username", x.target.value)} onBlur={() => form.blur("username")} data-testid="new-username" />
        </Field>
        <Field label={t("login.password")} hint={t("hint.password")} error={e("password")}>
          <PasswordInput icon={LockKeyhole} autoComplete="new-password" value={form.values.password} onChange={(x) => form.set("password", x.target.value)} onBlur={() => form.blur("password")} data-testid="new-password" />
        </Field>
        <Field label={t("users.role")} hint={t("hint.role")} error={e("role")}>
          <Select value={form.values.role} onChange={(x) => form.set("role", x.target.value)} data-testid="new-role">
            {roles.map((r) => <option key={r.code} value={r.code}>{t(`role.${r.code}`)}</option>)}
          </Select>
        </Field>
      </form>
    </Dialog>
  );
}

function EditUserDialog({ user, roles, onClose, onSaved }: { user: UserInfo; roles: RoleInfo[]; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const form = useForm({ display_name: user.display_name, role: user.role }, { display_name: v.displayName });
  const [active, setActive] = useState(user.is_active);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const isOwner = user.role === "owner";

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      // `version` makes a concurrent edit by someone else fail instead of being overwritten.
      await rpc("users.update", { id: user.id, version: user.version, display_name: form.values.display_name, role: form.values.role, is_active: active });
      toast.success(t("common.saved"));
      onSaved();
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      title={t("users.editTitle")}
      description={<bdi className="ltr">{user.username}</bdi>}
      onClose={onClose}
      testId="edit-user-dialog"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" loading={busy} type="submit" form="edit-user-form" data-testid="save-user">{t("common.save")}</Button>
        </>
      }
    >
      <form id="edit-user-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {isOwner && <Notice tone="info">{t("users.ownerFixed")}</Notice>}
        <Field label={t("login.displayName")} error={form.error("display_name") && t(form.error("display_name")!)}>
          <TextInput icon={IdCard} value={form.values.display_name} onChange={(x) => form.set("display_name", x.target.value)} data-testid="edit-display" />
        </Field>
        <Field label={t("users.role")} error={form.error("role") && t(form.error("role")!)}>
          <Select value={form.values.role} disabled={isOwner} onChange={(x) => form.set("role", x.target.value)} data-testid="edit-role">
            {isOwner ? <option value="owner">{t("role.owner")}</option> : roles.map((r) => <option key={r.code} value={r.code}>{t(`role.${r.code}`)}</option>)}
          </Select>
        </Field>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className="row"><ShieldCheck size={18} className="subtle" aria-hidden /> {t("users.activeLabel")}</span>
          <Switch checked={active} disabled={isOwner} onChange={setActive} label={<span className="visually-hidden">{t("users.activeLabel")}</span>} testId="edit-active" />
        </div>
      </form>
    </Dialog>
  );
}
