// Receipt document model → HTML. Used by receipt.html (single) and sheet.html (N-up).
import { formatShamsi, formatAfn, toDigits } from "./shamsi.js";

/** Output Profiles (roadmap 6.9): content width, font size, page size (0 = roll, height = content). */
export const PAPERS = {
  "58": { width: "48mm", fontSize: "8pt", pageMm: [58, 0] },
  "80": { width: "72mm", fontSize: "9.5pt", pageMm: [80, 0] },
  a6: { width: "97mm", fontSize: "10.5pt", pageMm: [105, 148] },
  a5: { width: "136mm", fontSize: "12pt", pageMm: [148, 210] },
  a4: { width: "72mm", fontSize: "9.5pt", pageMm: [210, 297] },
};

const LABELS = {
  fa: { clinic: "کلینیک دندان‌پزشکی آرتاویو", addr: "کابل، کارته سه، سرک دارالامان", receipt: "رسید پرداخت", no: "شماره", date: "تاریخ", patient: "مریض", doctor: "داکتر", total: "مجموع", paid: "پرداخت شده", balance: "باقی‌مانده", cash: "نقد", thanks: "از اعتماد شما سپاس‌گزاریم" },
  ps: { clinic: "د آرتاویو د غاښونو کلینیک", addr: "کابل، کارته سه، د دارالامان سړک", receipt: "د تادیې رسید", no: "شمېره", date: "نېټه", patient: "ناروغ", doctor: "ډاکټر", total: "ټول", paid: "ورکړل شوي", balance: "پاتې", cash: "نغدې", thanks: "ستاسو د باور مننه کوو؛ ښه روغتیا غواړو" },
  en: { clinic: "Artaveo Dental Clinic", addr: "Karte-3, Darulaman Rd, Kabul", receipt: "Payment Receipt", no: "No.", date: "Date", patient: "Patient", doctor: "Doctor", total: "Total", paid: "Paid", balance: "Balance", cash: "Cash", thanks: "Thank you for your trust" },
};

const SAMPLE_ITEMS = [
  { name: { fa: "عصب‌کشی (RCT) دندان ۳۶", ps: "د ۳۶ غاښ د عصب درملنه (RCT)", en: "Root canal (RCT) tooth 36" }, minor: 450000 },
  { name: { fa: "ترمیم کامپوزیت – Class II", ps: "کامپوزیټ ترمیم – Class II", en: "Composite filling – Class II" }, minor: 150000 },
  { name: { fa: "رادیوگرافی Periapical (PA)", ps: "رادیوګرافي Periapical (PA)", en: "Periapical X-ray (PA)" }, minor: 30000 },
];

const ltr = (s) => `<span class="ltr">${s}</span>`;

export function renderReceipt(el, { lang = "fa", digits = lang === "en" ? "latn" : "arabext", number = "RC-1405-000123", patientNo = "P-000123", items = SAMPLE_ITEMS, paid = 500000 } = {}) {
  const L = LABELS[lang];
  const total = items.reduce((s, i) => s + i.minor, 0);
  const issued = new Date(Date.UTC(2026, 9, 1, 8, 30));
  el.innerHTML = `
    <h1>${L.clinic}</h1>
    <div class="sub">${L.addr}</div>
    <div class="sub">${ltr(toDigits("+93 700 123 456", digits))}</div>
    <hr>
    <div class="row"><b>${L.receipt}</b><span>${L.no}: ${ltr(number)}</span></div>
    <div class="row"><span>${L.date}:</span><span>${formatShamsi(issued, lang, digits)}</span></div>
    <div class="row"><span>${L.patient}:</span><span>${lang === "en" ? "Ahmad Wali" : "احمد ولی"} (${ltr(patientNo)})</span></div>
    <div class="row"><span>${L.doctor}:</span><span>${lang === "en" ? "Dr. Zarghona" : "داکتر زرغونه"}</span></div>
    <hr>
    <table>${items.map((i) => `<tr><td>${i.name[lang]}</td><td class="amt">${formatAfn(i.minor, lang, digits)}</td></tr>`).join("")}</table>
    <hr>
    <div class="row total"><span>${L.total}</span><span>${formatAfn(total, lang, digits)}</span></div>
    <div class="row"><span>${L.paid} (${L.cash})</span><span>${formatAfn(paid, lang, digits)}</span></div>
    <div class="row"><span>${L.balance}</span><span>${formatAfn(total - paid, lang, digits)}</span></div>
    <hr>
    <div class="foot">${L.thanks}</div>`;
}
