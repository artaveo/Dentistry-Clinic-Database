import { useEffect, useRef, useState } from "react";
import { Phone, Search, UserPlus, X } from "lucide-react";
import type { PatientInfo } from "../../../../shared/ts/contract";
import { ApiError, isSessionError, rpc } from "../../lib/api";
import { useI18n } from "../../i18n";
import { Button } from "../../ui/Button";
import { Field, TextInput } from "../../ui/Field";
import { Notice } from "../../ui/Feedback";
import { v } from "../../lib/validation";

export type PickedPatient = { id: string; name: string; number: string; phone?: string | null };

export const pick = (p: PatientInfo): PickedPatient => ({ id: p.id, name: p.full_name, number: p.patient_number, phone: p.phone });

/**
 * Finds a patient by name, number or phone, or registers a new one without leaving the form
 * (4.3 "quick new patient"). A possible duplicate is shown first, so reception can use the
 * existing record instead of creating a second one.
 */
export function PatientPicker({ value, onChange, canCreate, error, testId = "patient-picker" }: {
  value: PickedPatient | null;
  onChange: (p: PickedPatient | null) => void;
  canCreate: boolean;
  error?: string | null;
  testId?: string;
}) {
  const { t, err } = useI18n();
  const [query, setQuery] = useState("");
  const [found, setFound] = useState<PatientInfo[]>([]);
  const [searching, setSearching] = useState(false);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [phone, setPhone] = useState("");
  const [touched, setTouched] = useState(false);
  const [duplicates, setDuplicates] = useState<PatientInfo[]>([]);
  const [failure, setFailure] = useState("");
  const [busy, setBusy] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    window.clearTimeout(timer.current);
    if (!query.trim()) {
      setFound([]);
      setSearching(false);
      return;
    }
    setSearching(true);
    timer.current = window.setTimeout(() => {
      rpc("patients.list", { query, status: "active", limit: 8, offset: 0 })
        .then((r) => setFound(r.items.filter((p) => !p.merged_into_id)))
        .catch(() => setFound([]))
        .finally(() => setSearching(false));
    }, 200);
    return () => window.clearTimeout(timer.current);
  }, [query]);

  const nameError = touched ? v.fullName(name) : null;
  const phoneError = touched ? v.phone(phone) : null;

  const register = async (allowDuplicate: boolean) => {
    setTouched(true);
    setFailure("");
    if (v.fullName(name) || v.phone(phone)) return;
    setBusy(true);
    try {
      if (!allowDuplicate) {
        const same = await rpc("patients.check_duplicate", { full_name: name, phone: phone || null });
        if (same.length) {
          setDuplicates(same);
          return;
        }
      }
      const created = await rpc("patients.create", {
        full_name: name.trim(), father_name: null, preferred_language: null, gender_id: null, date_of_birth: null, approximate_age: null,
        phone: phone.trim() || null, secondary_phone: null, province_id: null, district_id: null, address: null, emergency_contact_name: null,
        emergency_contact_phone: null, emergency_contact_relationship_id: null, referral_source_id: null, notes: null, registration_date: null,
        allow_duplicate: allowDuplicate,
      });
      onChange(pick(created));
      reset();
    } catch (x) {
      if (x instanceof ApiError && x.rule) setFailure(t(`rule.${x.rule}`));
      else if (!isSessionError(x)) setFailure(err(x));
    } finally {
      setBusy(false);
    }
  };

  const reset = () => {
    setCreating(false);
    setName("");
    setPhone("");
    setTouched(false);
    setDuplicates([]);
    setFailure("");
    setQuery("");
    setFound([]);
  };

  if (value) {
    return (
      <div className="patient-chip" data-testid={testId}>
        <div className="patient-chip-text">
          <span className="cell-strong" data-testid={`${testId}-name`}>{value.name}</span>
          <span className="subtle t-caption"><bdi className="ltr num">{value.number}</bdi>{value.phone ? <> · <bdi className="ltr">{value.phone}</bdi></> : null}</span>
        </div>
        <Button size="sm" variant="subtle" icon={X} onClick={() => onChange(null)} data-testid={`${testId}-change`}>{t("appt.patient.change")}</Button>
      </div>
    );
  }

  return (
    <div className="stack" data-testid={testId}>
      {!creating && (
        <>
          <Field label={t("appt.patient")} hint={t("appt.patient.hint")} error={error}>
            <TextInput icon={Search} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("patients.search.placeholder")} autoComplete="off" data-testid={`${testId}-search`} />
          </Field>
          {query.trim() && (
            <ul className="picker-list" data-testid={`${testId}-results`}>
              {found.map((p) => (
                <li key={p.id}>
                  <button type="button" onClick={() => onChange(pick(p))} data-testid={`${testId}-result-${p.patient_number}`}>
                    <span className="cell-strong">{p.full_name}</span>
                    <span className="subtle t-caption">{p.father_name ? `${p.father_name} · ` : ""}<bdi className="ltr num">{p.patient_number}</bdi>{p.phone ? <> · <bdi className="ltr">{p.phone}</bdi></> : null}</span>
                  </button>
                </li>
              ))}
              {!found.length && !searching && <li className="picker-empty subtle">{t("patients.emptySearch")}</li>}
            </ul>
          )}
          {canCreate && (
            <Button variant="link" icon={UserPlus} onClick={() => { setCreating(true); setName(query.trim()); }} data-testid={`${testId}-new`}>{t("appt.patient.new")}</Button>
          )}
        </>
      )}
      {creating && (
        <div className="stack picker-new" data-testid={`${testId}-form`}>
          <span className="t-subtitle">{t("appt.patient.newTitle")}</span>
          {failure && <Notice tone="danger">{failure}</Notice>}
          <Field label={t("patient.field.fullName")} hint={t("hint.fullName")} error={nameError && t(nameError)}>
            <TextInput value={name} onChange={(e) => { setName(e.target.value); setDuplicates([]); }} onBlur={() => setTouched(true)} data-testid={`${testId}-name-input`} />
          </Field>
          <Field label={t("patient.field.phone")} hint={t("hint.phone")} optional error={phoneError && t(phoneError)}>
            <TextInput icon={Phone} dir="ltr" value={phone} onChange={(e) => { setPhone(e.target.value); setDuplicates([]); }} onBlur={() => setTouched(true)} data-testid={`${testId}-phone-input`} />
          </Field>
          {duplicates.length > 0 && (
            <Notice tone="warning" title={t("appt.patient.duplicate")} testId={`${testId}-duplicates`}>
              <span className="stack" style={{ gap: 4 }}>
                {duplicates.map((d) => (
                  <Button key={d.id} size="sm" onClick={() => { onChange(pick(d)); reset(); }} data-testid={`${testId}-use-${d.patient_number}`}>
                    {d.full_name} · <bdi className="ltr num">{d.patient_number}</bdi>
                  </Button>
                ))}
              </span>
            </Notice>
          )}
          <div className="row">
            <Button onClick={reset}>{t("common.cancel")}</Button>
            {duplicates.length > 0 ? (
              <Button variant="primary" loading={busy} onClick={() => register(true)} data-testid={`${testId}-create-anyway`}>{t("appt.patient.createAnyway")}</Button>
            ) : (
              <Button variant="primary" icon={UserPlus} loading={busy} onClick={() => register(false)} data-testid={`${testId}-create`}>{t("appt.patient.create")}</Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
