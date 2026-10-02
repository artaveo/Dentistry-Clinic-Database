// In-browser stand-in for the Rust Core, for UI work and design screenshots
// on a computer that cannot build the Core (`VITE_MOCK=1 npm run dev`).
// It is compiled out of every normal build (see `lib/api.ts`), is never used
// by E2E tests (they run the real Core) and mirrors only the behaviour the UI
// needs: the same methods, validation rules, field errors and lock rules.
// `?mock=seeded` starts with a configured clinic and some history.
import type {
  AttachmentInfo, AuditEntry, BackupInfo, ClinicProfile, ErrorCode, LabeledItem, MedicalHistoryInfo, PatientInfo, RpcRequest, RpcResponse, Settings, UserInfo, ValidationRule,
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

const VERSION = "0.3.0";
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

const ALL_PERMS = ["audit.view", "backup.create", "backup.restore", "backup.view", "settings.manage", "users.manage", "patients.view", "patients.edit", "clinical.view", "clinical.edit"];
const ROLE_PERMS: Record<string, string[]> = {
  owner: ALL_PERMS,
  administrator: ALL_PERMS.filter((p) => p !== "backup.restore"),
  receptionist: ["patients.view", "patients.edit"],
  doctor: ["patients.view", "patients.edit", "clinical.view", "clinical.edit"],
  accountant: ["patients.view"],
  assistant: ["patients.view", "clinical.view"],
};

const REFERENCE: Record<string, [string, string, string, string][]> = {
  gender: [["male", "مرد", "نارینه", "Male"], ["female", "زن", "ښځینه", "Female"]],
  referral_source: [
    ["family_friend", "آشنا یا خانواده", "پیژندګلو یا کورنۍ", "Family or friend"],
    ["social_media", "شبکه‌های اجتماعی", "ټولنیزې رسنۍ", "Social media"],
    ["signboard", "تابلوی کلینیک", "د کلینیک بورډ", "Clinic signboard"],
    ["doctor_referral", "معرفی داکتر", "د ډاکټر معرفي", "Doctor referral"],
    ["walk_in", "عبوری", "ناڅاپي راتګ", "Walk-in"],
    ["other", "سایر", "نور", "Other"],
  ],
  relationship: [
    ["spouse", "همسر", "میرمن یا مېړه", "Spouse"],
    ["parent", "پدر یا مادر", "پلار یا مور", "Parent"],
    ["child", "فرزند", "اولاد", "Child"],
    ["sibling", "خواهر یا برادر", "خور یا ورور", "Sibling"],
    ["relative", "خویشاوند", "خپلوان", "Relative"],
    ["friend", "دوست", "ملګری", "Friend"],
    ["other", "سایر", "نور", "Other"],
  ],
};

type P = PatientInfo;
type MH = MedicalHistoryInfo;
type A = AttachmentInfo & { sha256: string; data_url: string };

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
  patients: [] as P[],
  patientSeq: 0,
  medicalHistory: new Map<string, MH>(),
  attachments: [] as A[],
};

function record(user: UserInfo | null, action: string, entity: string | null = null, entityId: string | null = null) {
  state.audit.unshift({
    id: state.audit.length + 1, at: iso(), user_id: user?.id ?? null, username: user?.username ?? null, action, entity,
    entity_id: entityId, old_value: null, new_value: null, computer: "RECEPTION-PC",
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
function checkPhone(ph: string, field: string) {
  const digits = ph.replace(/\D/g, "").length;
  if (digits < 7 || digits > 15) invalid(field, "phone_format");
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

  const day2 = 86400_000;
  const samples: [string, string, string, string | null][] = [
    ["احمد خان", "کریم داد", "0700123456", "gender/male"],
    ["زرغونه احمدی", "", "0744567890", "gender/female"],
    ["نجیب الله رحیمی", "عبدالله", "0788112233", "gender/male"],
  ];
  samples.forEach(([full_name, father_name, phone, gender], i) => {
    state.patientSeq++;
    const patient: P = {
      id: uuid(), patient_number: `P-${String(state.patientSeq).padStart(6, "0")}`, full_name, father_name: father_name || null,
      preferred_language: "fa", gender_id: gender, date_of_birth: null, approximate_age: 25 + i * 8, phone, secondary_phone: null,
      province_id: "KBL", district_id: "KBL-5", address: "کابل، کارته چهار", emergency_contact_name: null, emergency_contact_phone: null,
      emergency_contact_relationship_id: null, referral_source_id: "referral_source/family_friend", notes: null,
      registration_date: iso(new Date(Date.now() - i * day2)).slice(0, 10), status: "active", merged_into_id: null, version: 1,
    };
    state.patients.push(patient);
  });
  state.medicalHistory.set(state.patients[0].id, { patient_id: state.patients[0].id, allergies: "پنی‌سیلین", current_medications: null, chronic_conditions: "دیابت", dental_history: null, previous_surgeries: null, notes: null, version: 1 });
}

if (typeof location !== "undefined" && new URLSearchParams(location.search).get("mock") === "seeded") seed();

function session(token: string | null, method: string): Session {
  const s = token ? state.sessions.get(token) : undefined;
  if (!s) return fail(token ? "session_expired" : "unauthenticated", "no session");
  const timeout = state.settings.session_timeout_minutes * 60_000;
  if (!s.locked && Date.now() - s.last >= timeout) s.locked = true;
  if (s.locked && !["session.state", "session.unlock", "auth.logout"].includes(method)) fail("session_locked", "screen is locked");
  return s; // only session.touch / login / unlock are activity (OF-012)
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

function findPatient(id: string): P {
  return state.patients.find((x) => x.id === id) ?? fail("not_found", "patient");
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
      return stateOf(s);
    case "session.touch": {
      const at = Date.now() - Math.min(p.idle_ms ?? 0, state.settings.session_timeout_minutes * 60_000);
      if (!s.locked && at > s.last) s.last = at;
      return stateOf(s);
    }
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
    case "reference.list": {
      const i = p.language === "ps" ? 2 : p.language === "en" ? 3 : 1;
      return (REFERENCE[p.type_code] ?? []).map((r) => ({ id: `${p.type_code}/${r[0]}`, code: r[0], label: r[i] }));
    }
    case "patients.list": {
      const q = (p.query ?? "").trim().toLowerCase();
      let items = state.patients.filter((x) => !p.status || x.status === p.status);
      if (q) items = items.filter((x) => [x.full_name, x.father_name, x.patient_number, x.phone, x.secondary_phone].some((f) => f?.toLowerCase().includes(q)));
      items = [...items].sort((a, b) => b.registration_date.localeCompare(a.registration_date) || b.patient_number.localeCompare(a.patient_number));
      return { items: items.slice(p.offset, p.offset + p.limit), total: items.length };
    }
    case "patients.get":
      return findPatient(p.patient_id);
    case "patients.check_duplicate": {
      const name = p.full_name.trim().toLowerCase();
      const phone = (p.phone ?? "").replace(/\D/g, "");
      return state.patients.filter((x) => x.full_name.trim().toLowerCase() === name || (phone && x.phone?.replace(/\D/g, "") === phone));
    }
    case "patients.create": {
      need(s, "patients.edit");
      if (!p.full_name?.trim()) invalid("full_name", "full_name_length");
      if (p.phone) checkPhone(p.phone, "phone");
      if (p.secondary_phone) checkPhone(p.secondary_phone, "secondary_phone");
      if (!p.allow_duplicate) {
        const name = p.full_name.trim().toLowerCase();
        const phone = (p.phone ?? "").replace(/\D/g, "");
        if (state.patients.some((x) => x.full_name.trim().toLowerCase() === name || (phone && x.phone?.replace(/\D/g, "") === phone))) {
          invalid("full_name", "possible_duplicate");
        }
      }
      state.patientSeq++;
      const patient: P = {
        id: uuid(), patient_number: `P-${String(state.patientSeq).padStart(6, "0")}`, full_name: p.full_name.trim(),
        father_name: p.father_name || null, preferred_language: p.preferred_language || null, gender_id: p.gender_id || null,
        date_of_birth: p.date_of_birth || null, approximate_age: p.approximate_age ?? null, phone: p.phone || null,
        secondary_phone: p.secondary_phone || null, province_id: p.province_id || null, district_id: p.district_id || null,
        address: p.address || null, emergency_contact_name: p.emergency_contact_name || null, emergency_contact_phone: p.emergency_contact_phone || null,
        emergency_contact_relationship_id: p.emergency_contact_relationship_id || null, referral_source_id: p.referral_source_id || null,
        notes: p.notes || null, registration_date: p.registration_date || iso().slice(0, 10), status: "active", merged_into_id: null, version: 1,
      };
      state.patients.push(patient);
      record(s.user, "patient.create", "patient", patient.id);
      return patient;
    }
    case "patients.update": {
      need(s, "patients.edit");
      const patient = findPatient(p.id);
      if (patient.version !== p.version) fail("conflict", "changed");
      if (!p.full_name?.trim()) invalid("full_name", "full_name_length");
      if (p.phone) checkPhone(p.phone, "phone");
      Object.assign(patient, {
        full_name: p.full_name.trim(), father_name: p.father_name || null, preferred_language: p.preferred_language || null,
        gender_id: p.gender_id || null, date_of_birth: p.date_of_birth || null, approximate_age: p.approximate_age ?? null,
        phone: p.phone || null, secondary_phone: p.secondary_phone || null, province_id: p.province_id || null, district_id: p.district_id || null,
        address: p.address || null, emergency_contact_name: p.emergency_contact_name || null, emergency_contact_phone: p.emergency_contact_phone || null,
        emergency_contact_relationship_id: p.emergency_contact_relationship_id || null, referral_source_id: p.referral_source_id || null,
        notes: p.notes || null, status: p.status, version: patient.version + 1,
      });
      record(s.user, "patient.update", "patient", patient.id);
      return patient;
    }
    case "patients.delete": {
      need(s, "patients.edit");
      state.patients = state.patients.filter((x) => x.id !== p.id);
      record(s.user, "patient.delete", "patient", p.id);
      return {};
    }
    case "patients.merge": {
      need(s, "patients.edit");
      const keep = findPatient(p.keep_id);
      const merged = findPatient(p.merge_id);
      if (keep.id === merged.id) invalid("merge_id", "cannot_merge_self");
      Object.assign(merged, { status: "inactive", merged_into_id: keep.id, version: merged.version + 1 });
      record(s.user, "patient.merge", "patient", p.merge_id);
      return keep;
    }
    case "patients.export":
      return { csv_base64: btoa("patient_number,full_name\n"), file_name: "patients.csv" };
    case "patients.import":
      return { total: 0, imported: 0, skipped: 0, preview: [], errors: [] };
    case "medical_history.get": {
      const h = state.medicalHistory.get(p.patient_id);
      return h ?? { patient_id: p.patient_id, allergies: null, current_medications: null, chronic_conditions: null, dental_history: null, previous_surgeries: null, notes: null, version: 0 };
    }
    case "medical_history.update": {
      need(s, "clinical.edit");
      const before = state.medicalHistory.get(p.patient_id);
      if ((before?.version ?? 0) !== p.version) fail("conflict", "changed");
      const after: MH = {
        patient_id: p.patient_id, allergies: p.allergies ?? null, current_medications: p.current_medications ?? null,
        chronic_conditions: p.chronic_conditions ?? null, dental_history: p.dental_history ?? null,
        previous_surgeries: p.previous_surgeries ?? null, notes: p.notes ?? null, version: (before?.version ?? 0) + 1,
      };
      state.medicalHistory.set(p.patient_id, after);
      record(s.user, "patient.medical_history_update", "patient", p.patient_id);
      return after;
    }
    case "attachments.list":
      return state.attachments.filter((a) => a.patient_id === p.patient_id);
    case "attachments.upload": {
      need(s, "patients.edit");
      const mime = /\.(png)$/i.test(p.file_name) ? "image/png" : /\.(jpe?g)$/i.test(p.file_name) ? "image/jpeg" : "application/octet-stream";
      const a: A = {
        id: uuid(), patient_id: p.patient_id, kind: p.kind, file_name: p.file_name, mime_type: mime, size_bytes: p.data_base64.length,
        tooth: p.tooth || null, description: p.description || null, has_thumbnail: false, captured_at: p.captured_at || iso().slice(0, 10),
        created_at: iso(), version: 1, sha256: uuid(), data_url: `data:${mime};base64,${p.data_base64}`,
      };
      state.attachments.push(a);
      record(s.user, "patient.attachment_add", "patient", p.patient_id);
      return a;
    }
    case "attachments.delete": {
      need(s, "patients.edit");
      const target = state.attachments.find((a) => a.id === p.id) ?? fail("not_found", "attachment");
      state.attachments = state.attachments.filter((a) => a.id !== p.id);
      record(s.user, "patient.attachment_delete", "patient", target.patient_id);
      return {};
    }
    case "attachments.file": {
      const a = state.attachments.find((x) => x.id === p.id) ?? fail("not_found", "attachment");
      return { data_url: a.data_url };
    }
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
    case "audit.list": {
      need(s, p.entity_id ? "patients.view" : "audit.view");
      const rows = p.entity_id ? state.audit.filter((a) => a.entity_id === p.entity_id) : state.audit;
      return rows.slice(p.offset, p.offset + p.limit);
    }
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
