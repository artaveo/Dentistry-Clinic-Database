// Builds the Word review form for a native Pashto reviewer (UI texts + reference data + provinces + districts).
// Usage: npm i docx@9 (in a scratch dir) && node build-pashto-review-docx.js <out.docx>
// Row IDs (UI-001, DS-299, ...) are mapped back to i18n keys / seed codes in docs/i18n/pashto-review-ids.json.
const fs = require("fs");
const path = require("path");
const {
  Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell, WidthType, ShadingType,
  AlignmentType, PageOrientation, HeadingLevel, CheckBox, BorderStyle, VerticalAlign, Footer, PageNumber,
} = require("docx");

const REPO = "E:/Dentistry Clinic Database";
const OUT = process.argv[2];
const read = (p) => fs.readFileSync(path.join(REPO, p), "utf8");
const json = (p) => JSON.parse(read(p));

function csv(p) {
  const [head, ...rows] = read(p).trim().split(/\r?\n/);
  const cols = head.split(",");
  return rows.map((r) => Object.fromEntries(r.split(",").map((v, i) => [cols[i], v])));
}

const en = json("ui/src/i18n/en.json"), fa = json("ui/src/i18n/fa.json"), ps = json("ui/src/i18n/ps.json");
const ui = Object.keys(ps).filter((k) => !k.startsWith("lang."))
  .map((k) => ({ key: k, en: en[k] ?? "", fa: fa[k] ?? "", ps: ps[k] ?? "" }));
const provinces = csv("core/seeds/provinces.csv");
const districts = csv("core/seeds/districts.csv");
const reference = csv("core/seeds/reference.csv");
const provName = Object.fromEntries(provinces.map((p) => [p.code, p]));

const FONT = "Tahoma";
const ACCENT = "0F6B74", HEAD_BG = "E2F0F1", ALT_BG = "F6F9F9", LINE = "C9D6D8";

const run = (text, o = {}) => new TextRun({ text: String(text), font: FONT, size: o.size ?? 20, sizeComplexScript: o.size ?? 20,
  bold: o.bold, boldComplexScript: o.bold, color: o.color, rightToLeft: o.ltr ? false : true });
const para = (children, o = {}) => new Paragraph({ children: Array.isArray(children) ? children : [children],
  bidirectional: !o.ltr, alignment: o.align ?? (o.ltr ? AlignmentType.LEFT : AlignmentType.RIGHT),
  spacing: { before: o.before ?? 0, after: o.after ?? 0, line: o.line ?? 276 }, heading: o.heading, keepNext: o.keepNext });

const border = { style: BorderStyle.SINGLE, size: 4, color: LINE };
const borders = { top: border, bottom: border, left: border, right: border };

// Landscape A4: content width = 16838 - 2*720 = 15398
const W = [700, 3500, 3300, 3300, 850, 850, 2898];
const TOTAL = W.reduce((a, b) => a + b, 0);

function cell(content, i, o = {}) {
  return new TableCell({
    width: { size: W[i], type: WidthType.DXA }, borders, verticalAlign: VerticalAlign.CENTER,
    shading: o.bg ? { type: ShadingType.CLEAR, color: "auto", fill: o.bg } : undefined,
    margins: { top: 60, bottom: 60, left: 90, right: 90 },
    children: [content],
  });
}
const box = () => para(new CheckBox({ checked: false, checkedState: { value: "2612", font: "MS Gothic" }, uncheckedState: { value: "2610", font: "MS Gothic" } }), { align: AlignmentType.CENTER });

function headerRow() {
  const h = ["شماره", "English", "دری", "پښتو", "✓ سم دی", "✗ غلط دی", "سمه بڼه (اصلاح)"];
  return new TableRow({ tableHeader: true, children: h.map((t, i) =>
    cell(para(run(t, { bold: true, color: ACCENT, ltr: i === 1 }), { align: i >= 4 ? AlignmentType.CENTER : undefined, ltr: i === 1 }), i, { bg: HEAD_BG })) });
}

function table(items, prefix, ids) {
  const rows = items.map((it, n) => {
    const id = `${prefix}-${String(n + 1).padStart(3, "0")}`;
    ids[id] = it.ref;
    const bg = n % 2 ? ALT_BG : undefined;
    return new TableRow({ cantSplit: true, children: [
      cell(para(run(id, { size: 16, color: "5D7276", ltr: true }), { ltr: true }), 0, { bg }),
      cell(para(run(it.en, { ltr: true }), { ltr: true }), 1, { bg }),
      cell(para(run(it.fa)), 2, { bg }),
      cell(para(run(it.ps, { bold: true, size: 22 })), 3, { bg }),
      cell(box(), 4, { bg }),
      cell(box(), 5, { bg }),
      cell(para(run("")), 6, { bg }),
    ] });
  });
  return new Table({ width: { size: TOTAL, type: WidthType.DXA }, columnWidths: W, visuallyRightToLeft: true, rows: [headerRow(), ...rows] });
}

const ids = {};
const h1 = (t) => para(run(t, { bold: true, size: 30, color: ACCENT }), { heading: HEADING_1, before: 240, after: 120, keepNext: true });
const HEADING_1 = HeadingLevel.HEADING_1;
const note = (t, o = {}) => para(run(t, { size: o.size ?? 20, color: o.color, bold: o.bold }), { after: o.after ?? 80 });

const intro = [
  para(run("د آرتاویو ډینټل پروګرام د پښتو متنونو کتنه", { bold: true, size: 40, color: ACCENT }), { after: 60 }),
  para(run("بازبینی متن‌های پشتوی برنامه Artaveo Dental", { size: 24, color: "5D7276" }), { after: 240 }),
  note("ګرانه کتونکیه، مننه چې دا کار کوئ.", { bold: true, size: 22 }),
  note("دا متنونه د غاښونو د کلینیک د مدیریت پروګرام کې ښودل کېږي (تڼۍ، سرلیکونه، پیغامونه، د ولایتونو او ولسوالیو نومونه)."),
  note("د هرې کرښې لپاره: د «پښتو» ستون ولولئ. انګلیسي او دري متن یوازې د مانا د پوهېدو لپاره دي."),
  note("• که پښتو متن سم او طبیعي وي، په «✓ سم دی» خانه کلیک وکړئ."),
  note("• که غلط، نا اشنا یا ناطبیعي وي، په «✗ غلط دی» کلیک وکړئ او سمه بڼه په وروستي ستون کې ولیکئ."),
  note("• په تڼیو (Button) کې لنډ متن غوره دی."),
  note("راهنمای دری: برای هر ردیف فقط ستون «پښتو» را بخوانید. اگر درست است تیک «✓ سم دی» و اگر غلط است «✗ غلط دی» را بزنید و شکل درست را در ستون آخر بنویسید.", { color: "5D7276", after: 200 }),
  para(run("د کتونکي نوم: ____________________        نېټه: ______________", { size: 22 }), { after: 120, before: 120 }),
];

const uiItems = ui.map((u) => ({ en: u.en, fa: u.fa, ps: u.ps, ref: { kind: "ui", key: u.key } }));
const provItems = provinces.map((p) => ({ en: p.en, fa: p.fa, ps: p.ps, ref: { kind: "province", code: p.code } }));
const distItems = districts.map((d) => ({ en: `${d.en}  (${provName[d.province_code]?.en ?? d.province_code})`, fa: d.fa, ps: d.ps, ref: { kind: "district", code: d.code } }));
const refItems = reference.map((r) => ({ en: r.en, fa: r.fa, ps: r.ps, ref: { kind: "reference", type: r.type, code: r.code } }));

const doc = new Document({
  creator: "Artaveo", title: "Artaveo Dental — Pashto review",
  styles: { default: { document: { run: { font: FONT, size: 20 } } } },
  sections: [{
    properties: { page: { size: { width: 11906, height: 16838, orientation: PageOrientation.LANDSCAPE }, margin: { top: 720, bottom: 720, left: 720, right: 720 } } },
    footers: { default: new Footer({ children: [para([run("Artaveo Dental — Pashto review · ", { size: 16, color: "5D7276", ltr: true }), new TextRun({ children: [PageNumber.CURRENT], size: 16, color: "5D7276" })], { align: AlignmentType.CENTER, ltr: true })] }) },
    children: [
      ...intro,
      h1(`۱. د پروګرام متنونه (${uiItems.length})`), table(uiItems, "UI", ids),
      h1(`۲. عمومي معلومات — جنس، د وینې ګروپ او نور (${refItems.length})`), table(refItems, "RF", ids),
      h1(`۳. ولایتونه (${provItems.length})`), table(provItems, "PR", ids),
      h1(`۴. ولسوالۍ (${distItems.length})`), table(distItems, "DS", ids),
      para(run("مننه! دا فایل بیرته ولېږئ.", { bold: true, size: 24, color: ACCENT }), { before: 300 }),
    ],
  }],
});

Packer.toBuffer(doc).then((buf) => {
  fs.writeFileSync(OUT, buf);
  fs.writeFileSync(path.join(REPO, "docs/i18n/pashto-review-ids.json"), JSON.stringify(ids, null, 1));
  console.log("written", OUT, "rows:", Object.keys(ids).length);
});
