import type { ReactNode } from "react";
import type { AppointmentInfo, CalendarSystem, DocumentInfo, Language, Paper, RecordSnapshot, RxItem } from "../../../shared/ts/contract";
import { formatDate, formatTime } from "../lib/dates";
import { tr } from "../lib/medical";
import { formatAmount } from "../lib/money";
import { surfacesText, teethLabel, toothLabel } from "../chart/numbering";
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
      {kind === "treatment_plan" && <div className="doc-sign"><span className="doc-sign-line" />{t("doc.sign.patient")}</div>}
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
    case "treatment_plan": {
      const several = c.plans.length > 1;
      const until = c.valid_days ? addDaysIso(doc.issued_at.slice(0, 10), c.valid_days) : null;
      return (
        <div className="doc-fields" data-testid="doc-plan">
          {several && <p className="doc-muted">{t("doc.plan.choose")}</p>}
          {c.plans.map((plan, n) => (
            <section key={plan.plan_id} className="doc-plan">
              <h2 className="doc-h2">{several ? `${t("doc.plan.option").replace("{n}", num(n + 1, lang))}: ${plan.title}` : plan.title}</h2>
              <table className="doc-table doc-plan-table">
                <thead>
                  <tr>
                    <th>{t("doc.plan.phase")}</th>
                    <th>{t("doc.plan.service")}</th>
                    <th>{t("doc.teeth")}</th>
                    <th className="num">{t("doc.plan.sessions")}</th>
                    <th className="num">{t("doc.plan.price")}</th>
                  </tr>
                </thead>
                <tbody>
                  {plan.items.map((i, k) => (
                    <tr key={k}>
                      <td>{num(i.phase, lang)}</td>
                      <td>{i.service}{i.status === "completed" ? ` (${t("doc.plan.done")})` : ""}</td>
                      <td><bdi>{[teethLabel(i.teeth, c.numbering, lang), surfacesText(i.surfaces, i.teeth[0])].filter(Boolean).join(" ")}</bdi></td>
                      <td className="num">{num(i.sessions, lang)}</td>
                      <td className="num">{i.units > 1 ? `${num(i.units, lang)} × ${formatAmount(i.unit_price, lang)} = ` : ""}{i.total ? formatAmount(i.total, lang) : "—"}</td>
                    </tr>
                  ))}
                </tbody>
                <tfoot>
                  <tr><td colSpan={4}>{t("doc.plan.total")}</td><td className="num"><strong>{formatAmount(plan.total, lang)}</strong></td></tr>
                </tfoot>
              </table>
            </section>
          ))}
          {c.notes && <p className="doc-note">{c.notes}</p>}
          <p className="doc-muted">{t("doc.plan.estimateNote")}</p>
          {until && <p className="doc-muted" data-testid="doc-plan-valid">{t("doc.plan.validUntil").replace("{date}", formatDate(until + "T06:00:00Z", lang, calendar))}</p>}
        </div>
      );
    }
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
          {s && <ClinicalRecord s={s} draft={doc.id === "draft"} lang={lang} calendar={calendar} />}
        </div>
      );
    }
  }
}

/** The clinical part of a record summary (Phase 5B): the chart's conditions, the treatments and the signed notes. */
function ClinicalRecord({ s, draft, lang, calendar }: { s: RecordSnapshot; draft: boolean; lang: Language; calendar: CalendarSystem }) {
  const t = docT(lang);
  // The editor's live preview has no record yet: it says where the lines will come from.
  const none = <p className="doc-muted">{t(draft ? "doc.summary.fromRecord" : "doc.summary.none")}</p>;
  const sep = lang === "en" ? ", " : "، ";
  const teeth = [...new Set(s.findings.map((f) => f.tooth))];
  const where = (teethList: number[], surfaces: string) => [teethLabel(teethList, s.numbering, lang), surfacesText(surfaces, teethList[0])].filter(Boolean).join(" ");
  return (
    <>
      <h2 className="doc-h2">{t("doc.summary.findings")}</h2>
      {teeth.length ? (
        <table className="doc-table" data-testid="doc-summary-findings">
          <thead><tr><th>{t("doc.teeth")}</th><th>{t("doc.summary.condition")}</th></tr></thead>
          <tbody>
            {teeth.map((tooth) => (
              <tr key={tooth}>
                <td><bdi>{toothLabel(tooth, s.numbering, lang)}</bdi></td>
                <td>{s.findings.filter((f) => f.tooth === tooth).map((f) => (f.surfaces ? `${f.condition} (${surfacesText(f.surfaces, tooth)})` : f.condition)).join(sep)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : none}
      <h2 className="doc-h2">{t("doc.summary.treatments")}</h2>
      {s.treatments.length ? (
        <table className="doc-table" data-testid="doc-summary-treatments">
          <thead><tr><th>{t("doc.date")}</th><th>{t("doc.plan.service")}</th><th>{t("doc.teeth")}</th><th>{t("doc.summary.doctor")}</th></tr></thead>
          <tbody>
            {s.treatments.map((x, i) => (
              <tr key={i}>
                <td>{x.date ? formatDate(x.date + "T06:00:00Z", lang, calendar) : "—"}</td>
                <td>
                  {x.service}
                  {x.status !== "completed" && x.sessions_planned > 1 && <span className="doc-muted"> · {t("chart.sessionOf").replace("{n}", num(x.sessions_done, lang)).replace("{of}", num(x.sessions_planned, lang))}</span>}
                </td>
                <td><bdi>{where(x.teeth, x.surfaces) || "—"}</bdi></td>
                <td>{x.doctor ?? "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : none}
      <h2 className="doc-h2">{t("doc.summary.notes")}</h2>
      {s.notes.length ? (
        <ul className="doc-bullets" data-testid="doc-summary-notes">
          {s.notes.map((n, i) => (
            <li key={i}>
              <strong>{formatDate(n.date + "T06:00:00Z", lang, calendar)}</strong>{n.doctor && ` — ${n.doctor}`}
              {n.diagnosis && <div>{t("note.field.diagnosis")}: {n.diagnosis}</div>}
              {n.procedure && <div>{t("note.field.procedure")}: {n.procedure}</div>}
            </li>
          ))}
        </ul>
      ) : none}
    </>
  );
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
