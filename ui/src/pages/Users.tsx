import { useEffect, useState } from "react";
import { Crown, IdCard, KeyRound, LockKeyhole, LockOpen, Pencil, Plus, RotateCcw, ShieldCheck, Trash2, UserPlus, UserRound, Users } from "lucide-react";
import type { PermissionInfo, RoleInfo, UserInfo } from "../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../lib/api";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button, IconButton } from "../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../ui/Card";
import { Checkbox, Switch } from "../ui/Controls";
import { Field, PasswordInput, Select, TextInput } from "../ui/Field";
import { Badge, EmptyState, ErrorState, Notice, SkeletonRows } from "../ui/Feedback";
import { Dialog } from "../ui/Overlay";
import { Avatar } from "../ui/Brand";
import { useToast } from "../ui/Toast";
import { roleText, ruleText } from "./reception/labels";

export function UsersPage({ currentUserId }: { currentUserId: string }) {
  const { t, err } = useI18n();
  const [users, setUsers] = useState<UserInfo[] | null>(null);
  const [roles, setRoles] = useState<RoleInfo[]>([]);
  const [error, setError] = useState("");
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState<UserInfo | null>(null);
  const [permissions, setPermissions] = useState<PermissionInfo[]>([]);
  const [roleDialog, setRoleDialog] = useState<{ role?: RoleInfo } | null>(null);

  const load = () =>
    Promise.all([rpc("users.list", {}), rpc("roles.list", {}), rpc("permissions.list", {})])
      .then(([u, r, p]) => {
        setUsers(u);
        setRoles(r);
        setPermissions(p);
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
                    <td>{u.role === "owner" ? <Badge tone="accent" icon={Crown}>{t("role.owner")}</Badge> : <Badge>{roleText(t, u.role, u.role_label)}</Badge>}</td>
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
      <Card flush>
        <CardHeader icon={ShieldCheck} title={t("roles.title")} description={t("roles.subtitle")} actions={<Button icon={Plus} onClick={() => setRoleDialog({})} data-testid="role-add-open">{t("roles.add")}</Button>} />
        <div className="table-wrap">
          <table className="table" data-testid="role-list">
            <thead><tr><th>{t("users.role")}</th><th>{t("roles.permissions")}</th><th>{t("roles.users")}</th><th className="cell-actions"><span className="visually-hidden">{t("users.edit")}</span></th></tr></thead>
            <tbody>
              {roles.map((r) => (
                <tr key={r.code} data-testid={`role-row-${r.code}`}>
                  <td className="cell-strong">{r.code === "owner" ? t("role.owner") : roleText(t, r.code, r.label)}{r.is_system && <span className="subtle t-caption"> · {t("roles.builtIn")}</span>}</td>
                  <td>{r.permissions.length}</td>
                  <td>{r.user_count}</td>
                  <td className="cell-actions">
                    {r.code === "owner" ? <IconButton icon={ShieldCheck} label={t("roles.view")} size="sm" onClick={() => setRoleDialog({ role: r })} data-testid="role-view-owner" /> : <IconButton icon={Pencil} label={t("users.edit")} size="sm" onClick={() => setRoleDialog({ role: r })} data-testid={r.is_system ? `role-edit-${r.code}` : `role-edit-${r.label}`} />}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Card>
      {roleDialog && <RoleDialog role={roleDialog.role} permissions={permissions} onClose={() => setRoleDialog(null)} onSaved={load} />}
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
            {roles.map((r) => <option key={r.code} value={r.code}>{roleText(t, r.code, r.label)}</option>)}
          </Select>
        </Field>
      </form>
    </Dialog>
  );
}

function EditUserDialog({ user, roles, onClose, onSaved }: { user: UserInfo; roles: RoleInfo[]; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const [resetting, setResetting] = useState(false);
  // The version moves after an immediate action (unlock, new password); keep the newest one.
  const [current, setCurrent] = useState<UserInfo>(user);
  const unlock = async () => {
    try {
      setCurrent(await rpc("users.unlock", { id: current.id }));
      toast.success(t("users.unlocked"));
      onSaved();
    } catch (x) {
      if (!isSessionError(x)) toast.error(err(x));
    }
  };
  const form = useForm({ display_name: user.display_name, role: user.role }, { display_name: v.displayName });
  const [active, setActive] = useState(user.is_active);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const isOwner = current.role === "owner";

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      // `version` makes a concurrent edit by someone else fail instead of being overwritten.
      await rpc("users.update", { id: current.id, version: current.version, display_name: form.values.display_name, role: form.values.role, is_active: active });
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
        {current.locked && <Notice tone="warning" testId="user-locked">{t("users.lockedHint")}</Notice>}
        <Field label={t("login.displayName")} error={form.error("display_name") && t(form.error("display_name")!)}>
          <TextInput icon={IdCard} value={form.values.display_name} onChange={(x) => form.set("display_name", x.target.value)} data-testid="edit-display" />
        </Field>
        <Field label={t("users.role")} error={form.error("role") && t(form.error("role")!)}>
          <Select value={form.values.role} disabled={isOwner} onChange={(x) => form.set("role", x.target.value)} data-testid="edit-role">
            {isOwner ? <option value="owner">{t("role.owner")}</option> : roles.map((r) => <option key={r.code} value={r.code}>{roleText(t, r.code, r.label)}</option>)}
          </Select>
        </Field>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className="row"><ShieldCheck size={18} className="subtle" aria-hidden /> {t("users.activeLabel")}</span>
          <Switch checked={active} disabled={isOwner} onChange={setActive} label={<span className="visually-hidden">{t("users.activeLabel")}</span>} testId="edit-active" />
        </div>
        <div className="row">
          <Button icon={KeyRound} onClick={() => setResetting(true)} data-testid="user-reset-open">{t("users.resetPassword")}</Button>
          <Button icon={LockOpen} onClick={unlock} disabled={!current.locked} data-testid="user-unlock">{t("users.unlock")}</Button>
        </div>
      </form>
      {resetting && <ResetPasswordDialog user={current} onClose={() => setResetting(false)} onDone={(u) => { setCurrent(u); onSaved(); }} />}
    </Dialog>
  );
}

function ResetPasswordDialog({ user, onClose, onDone }: { user: UserInfo; onClose: () => void; onDone: (u: UserInfo) => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const form = useForm({ new_password: "" }, { new_password: v.password });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    if (!form.validate()) return;
    setBusy(true);
    try {
      onDone(await rpc("users.reset_password", { id: user.id, new_password: form.values.new_password }));
      toast.success(t("users.passwordReset"));
      onClose();
    } catch (x) {
      if (!form.serverError(x)) setError(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={t("users.resetPassword")}
      description={<span>{user.display_name} · <bdi className="ltr">{user.username}</bdi></span>}
      onClose={onClose}
      testId="reset-password-dialog"
      footer={<><Button onClick={onClose}>{t("common.cancel")}</Button><Button variant="primary" icon={KeyRound} loading={busy} type="submit" form="reset-password-form" data-testid="reset-password-confirm">{t("users.resetPassword")}</Button></>}
    >
      <form id="reset-password-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        <Notice tone="info">{t("users.resetHint")}</Notice>
        <Field label={t("users.newPassword")} hint={t("hint.password")} error={form.error("new_password") && t(form.error("new_password")!)}>
          <PasswordInput icon={LockKeyhole} autoComplete="new-password" value={form.values.new_password} onChange={(x) => form.set("new_password", x.target.value)} onBlur={() => form.blur("new_password")} data-testid="reset-password-input" />
        </Field>
      </form>
    </Dialog>
  );
}

const PERMISSION_GROUPS: { key: string; codes: string[] }[] = [
  { key: "patients", codes: ["patients.view", "patients.edit"] },
  { key: "appointments", codes: ["appointments.view", "appointments.edit", "appointments.treat", "doctors.manage"] },
  { key: "clinical", codes: ["clinical.view", "clinical.edit"] },
  { key: "billing", codes: ["billing.view", "billing.edit", "billing.void", "reports.view"] },
  { key: "inventory", codes: ["inventory.manage"] },
  { key: "admin", codes: ["users.manage", "settings.manage", "audit.view", "backup.view", "backup.create", "backup.restore"] },
];

/** A clinic's own role: a name and the permissions it grants (4.1). Built-in roles can only be viewed. */
function RoleDialog({ role, permissions, onClose, onSaved }: { role?: RoleInfo; permissions: PermissionInfo[]; onClose: () => void; onSaved: () => void }) {
  const { t, err } = useI18n();
  const toast = useToast();
  const readOnly = role?.code === "owner";
  // A built-in role keeps its name; its permissions can be changed (OF-028).
  const named = !role?.is_system;
  const form = useForm({ label: role?.label ?? "" }, { label: (s) => (!named ? null : !s.trim() ? "rule.required" : [...s.trim()].length > 60 ? "rule.role_label_length" : null) });
  const [granted, setGranted] = useState<string[]>(role?.permissions ?? []);
  const [permError, setPermError] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const assignable = new Set(permissions.filter((p) => p.assignable).map((p) => p.code));

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError("");
    setPermError("");
    if (!form.validate()) return;
    if (!granted.length) return setPermError(t("rule.permissions_empty"));
    setBusy(true);
    try {
      if (role) await rpc("roles.update", { code: role.code, version: role.version, label: form.values.label.trim(), permissions: granted });
      else await rpc("roles.create", { label: form.values.label.trim(), permissions: granted });
      toast.success(t("common.saved"));
      onSaved();
      onClose();
    } catch (x) {
      if (x instanceof ApiError && x.field === "permissions") setPermError(ruleText(t, x, err));
      else if (!form.serverError(x)) setError(ruleText(t, x, err));
    } finally {
      setBusy(false);
    }
  };

  const resetDefaults = async () => {
    if (!role) return;
    try {
      await rpc("roles.reset", { code: role.code, version: role.version });
      toast.success(t("roles.resetDone"));
      onSaved();
      onClose();
    } catch (x) {
      setError(ruleText(t, x, err));
    }
  };

  const remove = async () => {
    if (!role || !window.confirm(t("roles.deleteConfirm"))) return;
    try {
      await rpc("roles.delete", { code: role.code, version: role.version });
      toast.success(t("roles.deleted"));
      onSaved();
      onClose();
    } catch (x) {
      setError(ruleText(t, x, err));
    }
  };

  return (
    <Dialog
      title={role ? (readOnly ? (role.code === "owner" ? t("role.owner") : roleText(t, role.code, role.label)) : t("roles.edit")) : t("roles.add")}
      description={readOnly ? t("roles.builtInHint") : t("roles.hint")}
      onClose={onClose}
      wide
      testId="role-dialog"
      footer={
        <>
          {role && !readOnly && !role.is_system && <Button variant="danger" icon={Trash2} onClick={remove} data-testid="role-delete">{t("roles.delete")}</Button>}
          {role?.is_system && role.customized && <Button icon={RotateCcw} onClick={resetDefaults} data-testid="role-reset">{t("roles.resetDefaults")}</Button>}
          <span className="grow" />
          <Button onClick={onClose}>{readOnly ? t("common.close") : t("common.cancel")}</Button>
          {!readOnly && <Button variant="primary" loading={busy} type="submit" form="role-form" data-testid="role-save">{t("common.save")}</Button>}
        </>
      }
    >
      <form id="role-form" className="stack" onSubmit={submit} noValidate>
        {error && <Notice tone="danger">{error}</Notice>}
        {!readOnly && named && (
          <Field label={t("roles.name")} hint={t("hint.roleName")} error={form.error("label") && t(form.error("label")!)}>
            <TextInput value={form.values.label} onChange={(x) => form.set("label", x.target.value)} onBlur={() => form.blur("label")} data-testid="role-name" />
          </Field>
        )}
        {permError && <div className="field-error" role="alert" data-testid="field-error"><span>{permError}</span></div>}
        {PERMISSION_GROUPS.map((g) => (
          <fieldset className="perm-group" key={g.key}>
            <legend className="t-overline">{t(`perm.group.${g.key}`)}</legend>
            {g.codes.map((code) => (
              <Checkbox key={code} checked={granted.includes(code)} disabled={readOnly || !assignable.has(code)} onChange={(on) => setGranted((p) => (on ? [...p, code] : p.filter((c) => c !== code)))} testId={`perm-${code}`}>
                {t(`perm.${code}`)}
              </Checkbox>
            ))}
          </fieldset>
        ))}
      </form>
    </Dialog>
  );
}
