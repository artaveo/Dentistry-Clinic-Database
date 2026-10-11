// Fills a fresh dev-server clinic with a realistic Phase 5B patient (chart findings, two alternative plans,
// a root canal under way, a perio exam, a signed note), for design screenshots and the manual-test guide.
//
//   artaveo-dev-server --data-dir <fresh folder> --port 8787
//   node scripts/seed-5b.mjs http://127.0.0.1:8787/rpc
const [url = "http://127.0.0.1:8787/rpc"] = process.argv.slice(2);
let token = null;
async function rpc(method, params = {}) {
  const res = await fetch(url, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ method, params, token }) });
  const body = await res.json();
  if (body.status !== "ok") throw new Error(`${method}: ${JSON.stringify(body.error)}`);
  return body.result;
}

const status = await rpc("app.status");
if (status.state === "needs_setup") {
  await rpc("app.setup", { clinic_name: "کلینیک دندان آریانا", owner_username: "owner", owner_display_name: "داکتر احمد رحیمی", owner_password: "owner-pass-123", language: "fa", clinic_mode: "solo", address: "کابل، کارته سه", phone: "0700123456" });
}
token = (await rpc("auth.login", { username: "owner", password: "owner-pass-123" })).token;
const doctors = await rpc("doctors.list", {});
const doctor = doctors[0] ?? (await rpc("doctors.create", { full_name: "داکتر احمد رحیمی", color: "#0E7490", license_number: "MoPH-4471" }));
const catalog = await rpc("catalog.get", {});
const svc = (code) => catalog.services.find((s) => s.code === code);
for (const [code, afn] of [["END-01M", 4500], ["END-01D", 3500], ["PRS-01Z", 9000], ["PRS-01P", 6000], ["RST-01C", 1200], ["RST-02C", 1800], ["SRG-01", 800], ["IMP-01O", 28000], ["IMP-02", 6000], ["IMP-03", 9000], ["PER-01", 1000], ["DX-01", 200], ["RAD-02", 700], ["PRS-02Z", 9000], ["ORT-01M", 60000]]) {
  const s = svc(code);
  if (s && !s.price) await rpc("catalog.save_service", { ...s, price: afn * 100 });
}
const conditions = await rpc("tooth_conditions.list", {});
const cond = (code) => conditions.find((c) => c.code === code).id;

const patient = await rpc("patients.create", { full_name: "فریده احمدی", father_name: "محمد نعیم", phone: "0799555123", approximate_age: 38, allow_duplicate: true });
const P = patient.id;
const finding = (code, teeth, surfaces = "", note = null) => rpc("chart.add_findings", { patient_id: P, condition_id: cond(code), teeth, surfaces, note });
await finding("filling", [16], "MO");
await finding("filling", [26], "O");
await finding("caries", [36], "OD");
await finding("caries", [47], "B");
await finding("crown", [11]);
await finding("root_canal", [11]);
await finding("missing", [18, 28, 38]);
await finding("extracted", [46]);
await finding("periapical_lesion", [36]);

const item = async (plan, code, teeth, extra = {}) => (await rpc("treatment_items.add", { patient_id: P, plan_id: plan, service_id: svc(code).id, teeth, ...extra }))[0];
// Plan A: save 36 (root canal + crown), fill 47, implant at 46, scaling.
const a = await rpc("plans.create", { patient_id: P, title: "گزینه الف — حفظ دندان ۳۶", notes: null });
const rct = await item(a.id, "END-01M", [36], { phase: 1, details: "سه کانال" });
await item(a.id, "PRS-01Z", [36], { phase: 2 });
await item(a.id, "RST-02C", [47], { surfaces: "B", phase: 1 });
const imp = await item(a.id, "IMP-01O", [46], { phase: 2, details: "Osstem Ø4.0×10" });
await item(a.id, "IMP-03", [46], { phase: 3 });
await item(a.id, "PER-01", [], { phase: 1 });
// Plan B: extract 36 and a bridge 35–37.
const b = await rpc("plans.create", { patient_id: P, title: "گزینه ب — کشیدن و بریج", notes: null });
await item(b.id, "SRG-01", [36], { phase: 1 });
await item(b.id, "PRS-02Z", [35, 36, 37], { phase: 2 });
await rpc("plans.set_status", { id: a.id, version: 1, status: "accepted", cancel_alternatives: false });
// Work done: scaling today, first session of the root canal, the implant placed.
const plans = await rpc("plans.list", { patient_id: P });
const pa = plans.find((p) => p.id === a.id);
const scaling = pa.items.find((i) => i.service_id === svc("PER-01").id);
await rpc("treatment_items.record_session", { item_id: scaling.id, version: scaling.version, doctor_id: doctor.id, note: "جرم‌گیری کامل و پالیش" });
const r = await rpc("treatment_items.record_session", { item_id: rct.id, version: rct.version, doctor_id: doctor.id, note: "باز کردن حفره و پاکسازی سه کانال" });
const impNow = pa.items.find((i) => i.id === imp.id);
await rpc("treatment_items.record_session", { item_id: impNow.id, version: impNow.version, doctor_id: doctor.id, note: "کاشت ایمپلنت، ثبات اولیه خوب" });
// The next root-canal session booked.
if (r.next) {
  const [y, m, d] = r.next.date.split("-");
  await rpc("appointments.create", { patient_id: P, doctor_id: doctor.id, date: `${y}-${m}-${d}`, start_time: "10:00", end_time: "11:00", treatment_item_id: rct.id, override_schedule: true }).catch((e) => console.warn(String(e)));
}
// A signed note and a perio exam.
await rpc("notes.save", { patient_id: P, doctor_id: doctor.id, fields: { chief_complaint: "درد دندان ۳۶ هنگام جویدن", examination: "دق مثبت روی ۳۶، تورم ندارد. عکس پری‌اپیکال: ضایعه ریشه.", diagnosis: "نکروز پالپ با ضایعه پری‌اپیکال", procedure: "بی‌حسی، باز کردن حفره، پاکسازی سه کانال، پانسمان", follow_up: "جلسه دوم یک هفته بعد" }, sign: true });
const teeth = [];
for (const q of [1, 2, 3, 4]) for (let n = 1; n <= 8; n++) {
  const t = q * 10 + n;
  const missing = [18, 28, 38, 46].includes(t);
  const d = (base) => Array.from({ length: 6 }, (_, i) => (missing ? null : base + ((t + i) % 3 === 0 ? 1 : 0) + (n >= 6 && i % 3 === 1 ? 1 : 0)));
  teeth.push({ tooth: t, missing, implant: false, depth: d([16, 26, 36, 47].includes(t) ? 4 : 2), recession: Array(6).fill(missing ? null : (n === 1 ? 1 : 0)), bleeding: Array.from({ length: 6 }, (_, i) => !missing && (t + i) % 5 === 0), plaque: Array.from({ length: 6 }, (_, i) => !missing && (t + i) % 7 === 0), mobility: missing ? null : 0, furcation: [null, null] });
}
await rpc("perio.save", { patient_id: P, teeth, notes: "لثه در ناحیه آسیاها ملتهب" });
console.log(`patient ${patient.patient_number} ${P}`);
