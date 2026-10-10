import type { ReactNode } from "react";
import type { AppointmentInfo, CalendarSystem, DocumentInfo, Language, Paper, RxItem } from "../../../shared/ts/contract";
import { formatDate, formatTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { DentalMark } from "../ui/Brand";
import { docT, num, rxInstructions } from "./docText";
import type { Letterhead } from "./letterhead";

/**
 * One printed document at its real size (ADR-13): the clinic's letterhead (2.1b), the document's title,
 * number and date, the patient, the body of its kind, signatures, and the footer. The same element is
 * shown scaled in the preview, printed, and saved as PDF, so what the screen shows is what the paper gets.
 */
export function DocumentSheet({ doc, letterhead, calendar }: { doc: DocumentInfo; letterhead: Letterhead; calendar: CalendarSystem }) {
  const lang = doc.language;
  const t = docT(lang);
  const c = doc.content;
  const title = c.kind === "consent" || c.kind === "post_op" ? c.title : t(`doc.kind.${doc.kind}`);
  const p = doc.patient;
  const facts = [
    `${t("doc.patientId")}: ${p.number}`,
    p.age != null ? `${t("doc.age")}: ${num(p.age, lang)}` : null,
    p.gender ? t(`doc.gender.${p.gender}`) : null,
    p.father_name ? `${t("doc.father")}: ${p.father_name}` : null,
  ].filter(Boolean);
  return (
    <Sheet paper={doc.paper} lang={lang} letterhead={letterhead} testId={`doc-sheet-${doc.number}`} void={doc.status === "void"}>
      <Letterhead letterhead={letterhead} lang={lang} doctor={doc.doctor ? { name: doc.doctor.name, lines: [doc.doctor.specialties.map((s) => tr(s, lang)).join(lang === "en" ? ", " : "، "), doc.doctor.license_number ? `${t("doc.license")}: ${doc.doctor.license_number}` : ""] } : null} />
      <h1 className="doc-title">{title}</h1>
      <div className="doc-meta">
        <span>{t("doc.number")}: <bdi className="ltr">{doc.number}</bdi></span>
        <span>{t("doc.date")}: {formatDate(doc.issued_at, lang, calendar)}</span>
      </div>
      <div className="doc-patient" data-testid="doc-patient">
        <span className="doc-patient-name">{t("doc.patient")}: <strong>{p.name}</strong></span>
        <span className="doc-patient-facts">{facts.map((f, i) => <span key={i}><bdi>{f}</bdi></span>)}</span>
      </div>
      <div className="doc-body">
        <Body doc={doc} calendar={calendar} />
      </div>
      <Signatures kind={doc.kind} lang={lang} />
    </Sheet>
  );
}

/** The paper, its direction and the footer shared by every document and the appointment card. */
function Sheet({ paper, lang, letterhead, testId, void: isVoid, children }: { paper: Paper; lang: Language; letterhead: Letterhead; testId?: string; void?: boolean; children: ReactNode }) {
  const t = docT(lang);
  const lh = letterhead.clinic;
  return (
    <article className={`doc-sheet paper-${paper}`} dir={lang === "en" ? "ltr" : "rtl"} lang={lang} data-testid={testId} data-paper={paper} style={{ ["--doc-accent" as string]: lh.color_primary }}>
      {isVoid && <div className="doc-void" aria-hidden>{t("doc.void")}</div>}
      {children}
      <footer className="doc-foot">
        <span>{[lh.name, lh.address, lh.phone && <bdi key="p" className="ltr">{lh.phone}</bdi>].filter(Boolean).map((x, i) => <span key={i}>{i > 0 && " · "}{x}</span>)}</span>
        {letterhead.brandFooter && <span className="doc-brand"><DentalMark size={9} /> Artaveo Dental</span>}
      </footer>
    </article>
  );
}

function Letterhead({ letterhead, lang, doctor }: { letterhead: Letterhead; lang: Language; doctor: { name: string; lines: string[] } | null }) {
  const c = letterhead.clinic;
  return (
    <header className="doc-head">
      <div className="doc-clinic">
        {letterhead.logo ? <img className="doc-logo" src={letterhead.logo} alt="" /> : <span className="doc-monogram">{[...c.name.trim()][0] ?? "؟"}</span>}
        <div className="doc-clinic-text">
          <div className="doc-clinic-name">{c.name}</div>
          {(c.address || c.phone) && <div className="doc-clinic-meta">{c.address}{c.address && c.phone ? " · " : ""}{c.phone && <bdi className="ltr">{c.phone}</bdi>}</div>}
        </div>
      </div>
      {doctor && (
        <div className="doc-doctor" lang={lang}>
          <div className="doc-doctor-name">{doctor.name}</div>
          {doctor.lines.filter(Boolean).map((l, i) => <div key={i} className="doc-doctor-line"><bdi>{l}</bdi></div>)}
        </div>
      )}
    </header>
  );
}

function Signatures({ kind, lang }: { kind: DocumentInfo["kind"]; lang: Language }) {
  const t = docT(lang);
  if (kind === "post_op") return null;
  return (
    <div className={`doc-signatures ${kind === "consent" ? "three" : ""}`}>
      {kind === "consent" && (
        <>
          <div className="doc-sign"><span className="doc-sign-line" />{t("doc.sign.patient")}</div>
          <div className="doc-sign"><span className="doc-thumb" />{t("doc.sign.thumb")}</div>
        </>
      )}
      <div className="doc-sign"><span className="doc-sign-line" />{t("doc.sign.doctor")}</div>
    </div>
  );
}

function Paragraphs({ text }: { text: string }) {
  const lines = text.split("\n").map((l) => l.trim()).filter(Boolean);
  const bullets = lines.length > 0 && lines.every((l) => l.startsWith("•"));
  return bullets ? (
    <ul className="doc-bullets">{lines.map((l, i) => <li key={i}>{l.replace(/^•\s*/, "")}</li>)}</ul>
  ) : (
    <>{lines.map((l, i) => <p key={i}>{l.startsWith("•") ? <span className="doc-bullet-line">{l}</span> : l}</p>)}</>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="doc-row">
      <span className="doc-row-label">{label}</span>
      <span className="doc-row-value">{children}</span>
    </div>
  );
}

function Body({ doc, calendar }: { doc: DocumentInfo; calendar: CalendarSystem }) {
  const lang = doc.language;
  const t = docT(lang);
  const c = doc.content;
  switch (c.kind) {
    case "prescription":
      return (
        <>
          <div className="rx-symbol" aria-hidden>℞</div>
          <ol className="rx-items" data-testid="doc-rx-items">
            {c.items.map((i, n) => <RxLine key={n} item={i} lang={lang} />)}
          </ol>
          {c.notes && <div className="doc-note">{c.notes}</div>}
        </>
      );
    case "consent":
    case "post_op":
      return (
        <>
          {(c.procedure || c.teeth) && (
            <div className="doc-fields">
              {c.procedure && <Row label={t("doc.procedure")}>{c.procedure}</Row>}
              {c.teeth && <Row label={t("doc.teeth")}><bdi>{c.teeth}</bdi></Row>}
            </div>
          )}
          <div className="doc-text"><Paragraphs text={c.body} /></div>
        </>
      );
    case "referral":
      return (
        <div className="doc-fields">
          {c.urgent && <div className="doc-urgent">{t("doc.referral.urgent")}</div>}
          <Row label={t("doc.referral.to")}>{c.to}{c.specialty ? ` — ${c.specialty}` : ""}</Row>
          <Row label={t("doc.referral.reason")}>{c.reason}</Row>
          {c.summary && <Row label={t("doc.referral.summary")}><Paragraphs text={c.summary} /></Row>}
          <p className="doc-closing">{t("doc.referral.closing")}</p>
        </div>
      );
    case "imaging_request":
      return (
        <div className="doc-fields">
          {c.center && <Row label={t("doc.imaging.center")}>{c.center}</Row>}
          <Row label={t("doc.imaging.tests")}>
            <ul className="doc-checks">{c.tests.map((x) => <li key={x}><span className="doc-check" aria-hidden>✓</span>{t(`doc.test.${x}`)}</li>)}</ul>
          </Row>
          {c.teeth && <Row label={t("doc.teeth")}><bdi>{c.teeth}</bdi></Row>}
          {c.notes && <Row label={t("doc.notes")}>{c.notes}</Row>}
        </div>
      );
    case "certificate": {
      const restTo = c.rest_days && c.rest_from ? addDaysIso(c.rest_from, c.rest_days - 1) : null;
      return (
        <div className="doc-text">
          {c.addressee && <p className="doc-addressee">{t("doc.certificate.to")}: {c.addressee}</p>}
          <p>
            {t("doc.certificate.visit")
              .replace("{patient}", doc.patient.name)
              .replace("{father}", doc.patient.father_name ? t("doc.certificate.childOf").replace("{father}", doc.patient.father_name) : "")
              .replace("{date}", formatDate(c.visit_date + "T06:00:00Z", lang, calendar))
              .replace(/\s+،/g, "،")
              .replace(/\s{2,}/g, " ")}
          </p>
          {c.rest_days && c.rest_from && restTo && (
            <p data-testid="doc-rest">
              {t("doc.certificate.rest")
                .replace("{days}", num(c.rest_days, lang))
                .replace("{from}", formatDate(c.rest_from + "T06:00:00Z", lang, calendar))
                .replace("{to}", formatDate(restTo + "T06:00:00Z", lang, calendar))}
            </p>
          )}
          {c.notes && <p>{c.notes}</p>}
          <p className="doc-closing">{t("doc.certificate.closing")}</p>
        </div>
      );
    }
    case "lab_order":
      return (
        <div className="doc-fields">
          {c.lab && <Row label={t("doc.lab.lab")}>{c.lab}</Row>}
          <Row label={t("doc.teeth")}><bdi>{c.teeth}</bdi></Row>
          <Row label={t("doc.lab.work")}>{c.work}</Row>
          {c.material && <Row label={t("doc.lab.material")}>{c.material}</Row>}
          {c.shade && <Row label={t("doc.lab.shade")}><bdi className="ltr">{c.shade}</bdi></Row>}
          {c.due_date && <Row label={t("doc.lab.due")}>{formatDate(c.due_date + "T06:00:00Z", lang, calendar)}</Row>}
          {c.notes && <Row label={t("doc.notes")}>{c.notes}</Row>}
        </div>
      );
    case "record_summary": {
      const s = c.snapshot;
      return (
        <div className="doc-fields">
          {c.purpose && <Row label={t("doc.summary.purpose")}>{c.purpose}</Row>}
          <h2 className="doc-h2">{t("doc.summary.medical")}</h2>
          {s?.medical.length ? <ul className="doc-bullets">{s.medical.map((m, i) => <li key={i}>{m}</li>)}</ul> : <p className="doc-muted">{t("doc.summary.noMedical")}</p>}
          {s?.medical_notes && <p className="doc-note">{s.medical_notes}</p>}
          <h2 className="doc-h2">{t("doc.summary.visits")}</h2>
          {s?.visits.length ? (
            <table className="doc-table">
              <thead><tr><th>{t("doc.date")}</th><th>{t("doc.summary.doctor")}</th><th>{t("doc.summary.reason")}</th></tr></thead>
              <tbody>{s.visits.map((v, i) => <tr key={i}><td>{formatDate(v.date + "T06:00:00Z", lang, calendar)}</td><td>{v.doctor}</td><td>{v.reason ?? "—"}</td></tr>)}</tbody>
            </table>
          ) : <p className="doc-muted">{t("doc.summary.none")}</p>}
          <h2 className="doc-h2">{t("doc.summary.prescriptions")}</h2>
          {s?.prescriptions.length ? (
            <ul className="doc-bullets">{s.prescriptions.map((r) => <li key={r.number}><bdi className="ltr">{r.number}</bdi> — {formatDate(r.issued_at, lang, calendar)}: <bdi className="ltr">{r.medicines.join(", ")}</bdi></li>)}</ul>
          ) : <p className="doc-muted">{t("doc.summary.none")}</p>}
        </div>
      );
    }
  }
}

function RxLine({ item, lang }: { item: RxItem; lang: Language }) {
  const t = docT(lang);
  const head = [item.name, item.strength].filter(Boolean).join(" ");
  const sentence = rxInstructions(item, lang);
  return (
    <li className="rx-item">
      <div className="rx-head">
        <bdi className="rx-name" dir="ltr">{head}</bdi>
        <span className="rx-form">{t(`rx.form.${item.form}`)}</span>
        {item.quantity && <span className="rx-qty">{t("rx.quantity")}: <bdi>{num(item.quantity, lang)}</bdi></span>}
      </div>
      {(sentence || item.note) && <div className="rx-sentence">{[sentence, item.note].filter(Boolean).join(lang === "en" ? ". " : ". ")}</div>}
    </li>
  );
}

function addDaysIso(iso: string, n: number): string {
  const d = new Date(`${iso}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}

/**
 * The appointment card the patient takes home (4.6), now on the print engine (A6): the clinic, the
 * visit's day and time, the doctor, and a short reminder — in the language chosen for it.
 */
export function AppointmentCardSheet({ appt, letterhead, calendar, lang }: { appt: AppointmentInfo; letterhead: Letterhead; calendar: CalendarSystem; lang: Language }) {
  const t = docT(lang);
  return (
    <Sheet paper="a6" lang={lang} letterhead={letterhead} testId="appointment-card">
      <Letterhead letterhead={letterhead} lang={lang} doctor={null} />
      <h1 className="doc-title">{t("doc.card.title")}</h1>
      <div className="card-when">
        <div className="card-day">{t(`wizard.day.${weekdayOf(appt.date)}`)}</div>
        <div className="card-date" data-testid="appointment-card-date">{formatDate(appt.start_at, lang, calendar)}</div>
        <div className="card-time"><bdi>{formatTime(appt.start_time, lang)}</bdi></div>
      </div>
      <div className="doc-fields">
        <Row label={t("doc.patient")}>{appt.patient_name} <bdi className="ltr">({appt.patient_number})</bdi></Row>
        <Row label={t("doc.summary.doctor")}>{appt.doctor_name}</Row>
        {appt.reason && <Row label={t("doc.summary.reason")}>{appt.reason}</Row>}
      </div>
      <p className="doc-muted card-note">{t("doc.card.note")}</p>
    </Sheet>
  );
}

function weekdayOf(iso: string): number {
  const [y, m, d] = iso.split("-").map(Number);
  return (new Date(Date.UTC(y, m - 1, d)).getUTCDay() + 1) % 7;
}
