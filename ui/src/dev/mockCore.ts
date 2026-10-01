// In-browser stand-in for the Rust Core, for UI work and design screenshots
// on a computer that cannot build the Core (`VITE_MOCK=1 npm run dev`).
// It is compiled out of every normal build (see `lib/api.ts`), is never used
// by E2E tests (they run the real Core) and mirrors only the behaviour the UI
// needs: the same methods, validation rules, field errors and lock rules.
// `?mock=seeded` starts with a configured clinic and some history.
import type {
  AuditEntry, BackupInfo, ClinicProfile, ErrorCode, LabeledItem, RpcRequest, RpcResponse, Settings, UserInfo, ValidationRule,
} from "../../../shared/ts/contract";

type Fail = { code: ErrorCode; detail: string; field?: string; rule?: ValidationRule };
class MockError extends Error {
  constructor(public f: Fail) {
    super(f.detail);
  }
}
const fail = (code: ErrorCode, detail: string, field?: string, rule?: ValidationRule): never => {
  throw new MockError({ code, detail, field, rule });
};
const invalid = (field: string, rule: ValidationRule, detail = rule) => fail("validation", detail, field, rule);

const VERSION = "0.2.1";
const PROVINCES: [string, string, string, string][] = [
  ["KBL", "کابل", "کابل", "Kabul"],
  ["HRT", "هرات", "هرات", "Herat"],
  ["BLK", "بلخ", "بلخ", "Balkh"],
  ["KDH", "کندهار", "کندهار", "Kandahar"],
  ["NGR", "ننگرهار", "ننګرهار", "Nangarhar"],
  ["BAM", "بامیان", "باميان", "Bamyan"],
];
const DISTRICTS: Record<string, [string, string, string, string][]> = {
  KBL: [["KBL-1", "ناحیه ۱", "ناحیه ۱", "District 1"], ["KBL-5", "ناحیه ۵", "ناحیه ۵", "District 5"], ["PGM", "پغمان", "پغمان", "Paghman"]],
  HRT: [["HRT-C", "شهر هرات", "د هرات ښار", "Herat City"], ["ENJ", "انجیل", "انجیل", "Injil"]],
};

type Session = { user: UserInfo; perms: string[]; last: number; locked: boolean };
type U = UserInfo & { password: string };

const ALL_PERMS = ["audit.view", "backup.create", "backup.restore", "backup.view", "settings.manage", "users.manage"];
const ROLE_PERMS: Record<string, string[]> = {
  owner: ALL_PERMS,
  administrator: ALL_PERMS.filter((p) => p !== "backup.restore"),
  receptionist: [],
  doctor: [],
  accountant: [],
  assistant: [],
};

const iso = (d = new Date()) => d.toISOString().replace(/\.\d+Z$/, (m) => m.slice(0, 4) + "Z");
const uuid = () => crypto.randomUUID();

const state = {
  setUp: false,
  clinicName: null as string | null,
  defaultLanguage: "fa" as "fa" | "ps" | "en",
  clinic: null as ClinicProfile | null,
  logo: null as string | null,
  users: [] as U[],
  sessions: new Map<string, Session>(),
  settings: { session_timeout_minutes: 10, daily_backup_hour: 19, backup_keep_daily: 14 } as Settings,
  backups: [] as BackupInfo[],
  audit: [] as AuditEntry[],
  recoveryKey: "",
};

function record(user: UserInfo | null, action: string, entity: string | null = null) {
  state.audit.unshift({
    id: state.audit.length + 1, at: iso(), user_id: user?.id ?? null, username: user?.username ?? null, action, entity,
    entity_id: null, old_value: null, new_value: null, computer: "RECEPTION-PC",
  });
}

function validateUsername(u: string, field = "username") {
  if (!/^[A-Za-z0-9._-]{3,32}$/.test(u)) invalid(field, "username_format");
}
function validatePassword(p: string, field = "password") {
  if ([...p].length < 8) invalid(field, "password_too_short");
}
function validateDisplay(d: string, field = "display_name") {
  if (!d.trim() || [...d].length > 100) invalid(field, "display_name_length");
}

function publicUser(u: U): UserInfo {
  const { password: _p, ...rest } = u;
  return rest;
}

function defaultClinic(name: string): ClinicProfile {
  return {
    name, default_language: "fa", logo_path: null, province_id: "KBL", district_id: "KBL-5", address: "کابل، کارته چهار، سرک دوم",
    phone: "0700 123 456", calendar_system: "shamsi", clinic_mode: "solo", install_mode: "single", theme: "system",
    color_primary: "#0e7490", color_secondary: "#64748b", color_accent: "#f59e0b",
    working_hours: [0, 1, 2, 3, 4, 5, 6].map((day) => ({ day, closed: day === 6, open: day === 6 ? null : "08:00", close: day === 6 ? null : "16:00" })),
    trial_started_at: iso(),
  };
}

function seed() {
  state.setUp = true;
  state.clinicName = "کلینیک دندان‌پزشکی لبخند";
  state.clinic = defaultClinic(state.clinicName);
  const owner: U = { id: uuid(), username: "owner", display_name: "داکتر احمد رحیمی", role: "owner", is_active: true, version: 1, password: "owner-pass-123" };
  state.users = [
    owner,
    { id: uuid(), username: "reception", display_name: "مریم کریمی", role: "receptionist", is_active: true, version: 1, password: "reception-123" },
    { id: uuid(), username: "dr.sultani", display_name: "داکتر فرید سلطانی", role: "doctor", is_active: true, version: 1, password: "doctor-pass-1" },
    { id: uuid(), username: "accounts", display_name: "نجیب الله", role: "accountant", is_active: false, version: 2, password: "accounts-pass" },
  ];
  const day = 86400_000;
  state.backups = [0, 1, 2].map((i) => ({
    id: uuid(), file_name: `artaveo-2026100${2 - i}-1900-${i ? "daily" : "manual"}.adbk`, created_at: iso(new Date(Date.now() - i * day - 3600_000 * (i ? 2 : 0))),
    size_bytes: 188_416 + i * 4096, kind: i ? "daily" : "manual", verified: true,
  }));
  const acts: [number, string, string | null][] = [
    [0, "auth.login", "app_user"], [0, "backup.create", "backup"], [1, "user.create", "app_user"], [0, "clinic.update", "clinic"],
    [null as unknown as number, "auth.login_failed", null], [0, "settings.update", "setting"], [0, "session.lock", "app_user"], [0, "session.unlock", "app_user"], [0, "app.setup", "db_meta"],
  ];
  acts.reverse().forEach(([ui, action, entity], i) => {
    const u = ui === null ? null : state.users[ui];
    state.audit.unshift({ id: i + 1, at: iso(new Date(Date.now() - (acts.length - i) * 2_700_000)), user_id: u?.id ?? null, username: u?.username ?? "unknown", action, entity, entity_id: null, old_value: null, new_value: null, computer: "RECEPTION-PC" });
  });
}

if (typeof location !== "undefined" && new URLSearchParams(location.search).get("mock") === "seeded") seed();

function session(token: string | null, method: string): Session {
  const s = token ? state.sessions.get(token) : undefined;
  if (!s) return fail(token ? "session_expired" : "unauthenticated", "no session");
  const timeout = state.settings.session_timeout_minutes * 60_000;
  if (!s.locked && Date.now() - s.last >= timeout) s.locked = true;
  if (s.locked && !["session.state", "session.unlock", "auth.logout"].includes(method)) fail("session_locked", "screen is locked");
  if (!s.locked && method !== "session.state") s.last = Date.now();
  return s;
}
const stateOf = (s: Session) => ({
  locked: s.locked,
  idle_seconds_left: s.locked ? 0 : Math.max(0, Math.floor((state.settings.session_timeout_minutes * 60_000 - (Date.now() - s.last)) / 1000)),
});
const need = (s: Session, perm: string) => (s.perms.includes(perm) ? undefined : fail("forbidden", perm));

function geo(list: [string, string, string, string][], lang: string): LabeledItem[] {
  const i = lang === "ps" ? 2 : lang === "en" ? 3 : 1;
  return list.map((r) => ({ id: r[0], code: r[0], label: r[i] }));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function call(method: string, p: any, token: string | null): unknown {
  switch (method) {
    case "app.status":
      return { state: state.setUp ? "ready" : "needs_setup", version: VERSION, environment: "development", clinic_name: state.clinicName, default_language: state.defaultLanguage };
    case "app.clinic_logo":
      return { data_url: state.logo };
    case "app.setup": {
      if (state.setUp) fail("already_set_up", "set up");
      if (!p.clinic_name?.trim()) invalid("clinic_name", "clinic_name_length");
      validateUsername(p.owner_username, "owner_username");
      validateDisplay(p.owner_display_name, "owner_display_name");
      validatePassword(p.owner_password, "owner_password");
      state.setUp = true;
      state.clinicName = p.clinic_name.trim();
      state.defaultLanguage = p.language;
      state.clinic = { ...defaultClinic(state.clinicName!), ...p, name: state.clinicName!, default_language: p.language, logo_path: null, trial_started_at: p.trial_acknowledged ? iso() : null };
      if (p.logo_base64) state.logo = `data:image/png;base64,${p.logo_base64}`;
      const owner: U = { id: uuid(), username: p.owner_username, display_name: p.owner_display_name, role: "owner", is_active: true, version: 1, password: p.owner_password };
      state.users = [owner];
      record(owner, "app.setup", "db_meta");
      state.recoveryKey = Array.from({ length: 6 }, () => Array.from({ length: 6 }, () => "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"[Math.floor(Math.random() * 32)]).join("")).join("-");
      return { recovery_key: state.recoveryKey };
    }
    case "auth.login": {
      if (!state.setUp) fail("not_set_up", "");
      const u = state.users.find((x) => x.username.toLowerCase() === String(p.username).toLowerCase());
      if (!u || !u.is_active || u.password !== p.password) {
        record(u ? publicUser(u) : null, "auth.login_failed");
        fail("invalid_credentials", "bad");
      }
      const token = uuid();
      const s: Session = { user: publicUser(u!), perms: ROLE_PERMS[u!.role] ?? [], last: Date.now(), locked: false };
      state.sessions.set(token, s);
      record(s.user, "auth.login", "app_user");
      return { token, user: s.user, permissions: s.perms, timeout_minutes: state.settings.session_timeout_minutes, locked: false };
    }
    case "auth.recover_owner": {
      if (p.recovery_key.trim().toUpperCase() !== state.recoveryKey) fail("recovery_key_invalid", "bad key", "recovery_key", "recovery_key");
      validatePassword(p.new_password, "new_password");
      state.users[0].password = p.new_password;
      return {};
    }
  }
  if (!state.setUp) {
    if (method === "geo.provinces") return geo(PROVINCES, p.language);
    if (method === "geo.districts") return geo(DISTRICTS[p.province_id] ?? [], p.language);
    fail("not_set_up", "");
  }
  const s = session(token, method);
  switch (method) {
    case "auth.logout":
      state.sessions.delete(token!);
      return {};
    case "auth.change_password": {
      const u = state.users.find((x) => x.id === s.user.id)!;
      if (u.password !== p.current_password) fail("invalid_credentials", "wrong", "current_password", "wrong_password");
      validatePassword(p.new_password, "new_password");
      u.password = p.new_password;
      return {};
    }
    case "session.state":
    case "session.touch":
      return stateOf(s);
    case "session.lock":
      s.locked = true;
      record(s.user, "session.lock", "app_user");
      return stateOf(s);
    case "session.unlock": {
      const u = state.users.find((x) => x.id === s.user.id)!;
      if (u.password !== p.password) fail("invalid_credentials", "wrong", "password", "wrong_password");
      s.locked = false;
      s.last = Date.now();
      record(s.user, "session.unlock", "app_user");
      return stateOf(s);
    }
    case "users.list":
      need(s, "users.manage");
      return state.users.map(publicUser).sort((a, b) => a.username.localeCompare(b.username));
    case "roles.list":
      need(s, "users.manage");
      return Object.keys(ROLE_PERMS).map((code) => ({ code, permissions: ROLE_PERMS[code] }));
    case "users.create": {
      need(s, "users.manage");
      validateUsername(p.username);
      validateDisplay(p.display_name);
      validatePassword(p.password);
      if (state.users.some((u) => u.username.toLowerCase() === p.username.toLowerCase())) invalid("username", "username_taken");
      const u: U = { id: uuid(), username: p.username, display_name: p.display_name.trim(), role: p.role, is_active: true, version: 1, password: p.password };
      state.users.push(u);
      record(s.user, "user.create", "app_user");
      return publicUser(u);
    }
    case "users.update": {
      need(s, "users.manage");
      validateDisplay(p.display_name);
      const u = state.users.find((x) => x.id === p.id) ?? fail("not_found", "user");
      if (u.version !== p.version) fail("conflict", "changed");
      Object.assign(u, { display_name: p.display_name.trim(), role: u.role === "owner" ? "owner" : p.role, is_active: u.role === "owner" ? true : p.is_active, version: u.version + 1 });
      record(s.user, "user.update", "app_user");
      return publicUser(u);
    }
    case "reference.list":
      return [];
    case "geo.provinces":
      return geo(PROVINCES, p.language);
    case "geo.districts":
      return geo(DISTRICTS[p.province_id] ?? [], p.language);
    case "clinic.get":
      return state.clinic;
    case "clinic.update": {
      need(s, "settings.manage");
      for (const d of p.working_hours) if (!d.closed && (!d.open || !d.close || d.open >= d.close)) invalid("working_hours", "working_hours");
      state.clinic = { ...state.clinic!, ...p, name: state.clinic!.name, install_mode: state.clinic!.install_mode };
      record(s.user, "clinic.update", "clinic");
      return state.clinic;
    }
    case "clinic.set_logo":
      need(s, "settings.manage");
      if (p.logo_base64 && !/\.(png|jpe?g|webp)$/i.test(p.logo_file_name ?? "")) invalid("logo", "logo_type");
      state.logo = p.logo_base64 ? `data:image/${/\.png$/i.test(p.logo_file_name) ? "png" : "jpeg"};base64,${p.logo_base64}` : null;
      record(s.user, "clinic.set_logo", "clinic");
      return { data_url: state.logo };
    case "settings.get":
      return state.settings;
    case "settings.update":
      need(s, "settings.manage");
      if (p.session_timeout_minutes < 1 || p.session_timeout_minutes > 240) invalid("session_timeout_minutes", "session_timeout_range");
      if (p.daily_backup_hour < 0 || p.daily_backup_hour > 23) invalid("daily_backup_hour", "backup_hour_range");
      if (p.backup_keep_daily < 1 || p.backup_keep_daily > 365) invalid("backup_keep_daily", "backup_keep_range");
      state.settings = { ...p };
      record(s.user, "settings.update", "setting");
      return state.settings;
    case "backup.create": {
      need(s, "backup.create");
      const b: BackupInfo = { id: uuid(), file_name: `artaveo-${iso().slice(0, 16).replace(/[-:T]/g, "")}-manual.adbk`, created_at: iso(), size_bytes: 192_512, kind: "manual", verified: true };
      state.backups.unshift(b);
      record(s.user, "backup.create", "backup");
      return b;
    }
    case "backup.list":
      need(s, "backup.view");
      return state.backups;
    case "audit.list":
      need(s, "audit.view");
      return state.audit.slice(p.offset, p.offset + p.limit);
    case "system.info":
      return {
        app_version: VERSION, git_commit: "dev-mock", build_arch: "arm64", machine_arch: "arm64", emulated: false,
        os: "Windows 11 Pro 24H2 (build 26200)", environment: "development", computer_name: "RECEPTION-PC",
        data_dir: "C:\\ProgramData\\ArtaveoDental", log_dir: "C:\\ProgramData\\ArtaveoDental\\logs",
        database: { encrypted: true, cipher_version: "4.6.1 community", sqlite_version: "3.46.0", key_protection: "windows_dpapi_machine", schema_version: 4, size_bytes: 188_416 },
        integrity: { status: "ok", checked_at: iso(new Date(Date.now() - 60_000)), detail: null },
        last_backup: state.backups[0] ?? null,
      };
  }
  return fail("unknown_method", method);
}

export async function mockTransport(req: RpcRequest): Promise<RpcResponse> {
  await new Promise((r) => setTimeout(r, 60));
  try {
    return { status: "ok", result: call(req.method, req.params, req.token) };
  } catch (e) {
    if (e instanceof MockError) {
      return { status: "error", error: { code: e.f.code, detail: e.f.detail, field: e.f.field ?? null, rule: e.f.rule ?? null } };
    }
    return { status: "error", error: { code: "internal", detail: String(e), field: null, rule: null } };
  }
}
