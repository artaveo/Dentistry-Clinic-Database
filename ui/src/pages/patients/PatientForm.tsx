import { useState } from "react";
import { Save, UserPlus } from "lucide-react";
import type { LabeledItem, PatientInfo } from "../../../../shared/ts/contract";
import { rpc } from "../../lib/api";
import { useForm, v } from "../../lib/validation";
import { useI18n } from "../../i18n";
import { useReferenceList } from "./useReferenceList";
import { GeoPicker } from "../../setup/GeoPicker";
import { Button } from "../../ui/Button";
import { Card, CardHeader, Page, PageHeader } from "../../ui/Card";
import { Field, Select, Textarea, TextInput } from "../../ui/Field";
import { Notice } from "../../ui/Feedback";
import { useToast } from "../../ui/Toast";
import { DuplicateWarningDialog } from "./DuplicateWarningDialog";

type FormValues = {
  full_name: string;
  father_name: string;
  phone: string;
  secondary_phone: string;
  gender_id: string;
  date_of_birth: string;
  approximate_age: string;
  preferred_language: string;
  province_id: string;
  district_id: string;
  address: string;
  emergency_contact_name: string;
  emergency_contact_phone: string;
  emergency_contact_relationship_id: string;
  referral_source_id: string;
  notes: string;
};

function toValues(p?: PatientInfo | null): FormValues {
  return {
    full_name: p?.full_name ?? "",
    father_name: p?.father_name ?? "",
    phone: p?.phone ?? "",
    secondary_phone: p?.secondary_phone ?? "",
    gender_id: p?.gender_id ?? "",
    date_of_birth: p?.date_of_birth ?? "",
    approximate_age: p?.approximate_age != null ? String(p.approximate_age) : "",
    preferred_language: p?.preferred_language ?? "",
    province_id: p?.province_id ?? "",
    district_id: p?.district_id ?? "",
    address: p?.address ?? "",
    emergency_contact_name: p?.emergency_contact_name ?? "",
    emergency_contact_phone: p?.emergency_contact_phone ?? "",
    emergency_contact_relationship_id: p?.emergency_contact_relationship_id ?? "",
    referral_source_id: p?.referral_source_id ?? "",
    notes: p?.notes ?? "",
  };
}

/** Full-field body shared by `patients.create`/`patients.update` — everything but `id`/`version`/`status`/`allow_duplicate`. */
function toParams(values: FormValues) {
  return {
    full_name: values.full_name.trim(),
    father_name: values.father_name.trim() || null,
    phone: values.phone.trim() || null,
    secondary_phone: values.secondary_phone.trim() || null,
    gender_id: values.gender_id || null,
    date_of_birth: values.date_of_birth || null,
    approximate_age: values.approximate_age.trim() ? Number(values.approximate_age) : null,
    preferred_language: (values.preferred_language || null) as "fa" | "ps" | "en" | null,
    province_id: values.province_id || null,
    district_id: values.district_id || null,
    address: values.address.trim() || null,
    emergency_contact_name: values.emergency_contact_name.trim() || null,
    emergency_contact_phone: values.emergency_contact_phone.trim() || null,
    emergency_contact_relationship_id: values.emergency_contact_relationship_id || null,
    referral_source_id: values.referral_source_id || null,
    notes: values.notes.trim() || null,
  };
}

function ReferenceSelect({ typeCode, value, onChange, testId }: { typeCode: string; value: string; onChange: (id: string) => void; testId?: string }) {
  const { t } = useI18n();
  const items = useReferenceList(typeCode);
  return (
    <Select value={value} onChange={(e) => onChange(e.target.value)} data-testid={testId}>
      <option value="">{t("common.choose")}</option>
      {items.map((i: LabeledItem) => <option key={i.id} value={i.id}>{i.label}</option>)}
    </Select>
  );
}

/** Create (no `patient`) or edit (`patient` given) — same sectioned form either way (3.1). */
export function PatientForm({ patient, onDone, onCancel }: { patient?: PatientInfo | null; onDone: (p: PatientInfo) => void; onCancel: () => void }) {
  const { t, err, lang } = useI18n();
  const toast = useToast();
  const form = useForm(toValues(patient), {
    full_name: v.required,
    phone: v.phone,
    secondary_phone: v.phone,
    emergency_contact_phone: v.phone,
    approximate_age: (s) => (s.trim() && !/^\d{1,3}$/.test(s.trim()) ? "rule.age_range" : null),
  });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [duplicates, setDuplicates] = useState<PatientInfo[] | null>(null);
  const e = (k: keyof FormValues) => form.error(k) && t(form.error(k)!);

  const doSave = async (allowDuplicate: boolean) => {
    setBusy(true);
    setError("");
    try {
      const body = toParams(form.values);
      const saved = patient
        ? await rpc("patients.update", { id: patient.id, version: patient.version, status: patient.status, ...body })
        : await rpc("patients.create", { ...body, registration_date: null, allow_duplicate: allowDuplicate });
      toast.success(t(patient ? "patients.saved" : "patients.created"));
      onDone(saved);
    } catch (x) {
      if (!form.serverError(x) && !(x as { rule?: string }).rule) setError(err(x));
      else if ((x as { rule?: string }).rule === "possible_duplicate" && !patient) {
        const dups = await rpc("patients.check_duplicate", { full_name: form.values.full_name.trim(), phone: form.values.phone.trim() || null });
        setDuplicates(dups);
      } else if (!form.serverError(x)) setError(err(x));
    } finally {
      setBusy(false);
    }
  };

  const submit = async (ev: React.FormEvent) => {
    ev.preventDefault();
    if (!form.validate()) return;
    await doSave(false);
  };

  return (
    <Page testId="page-patient-form">
      <PageHeader title={t(patient ? "patients.editTitle" : "patients.add")} description={t("patients.addHint")} />
      <form className="stack" onSubmit={submit} noValidate data-testid="patient-form">
        {error && <Notice tone="danger">{error}</Notice>}

        <Card>
          <CardHeader title={t("patient.section.identity")} />
          <div className="grid-2">
            <Field label={t("patient.field.fullName")} error={e("full_name")}>
              <TextInput autoFocus value={form.values.full_name} onChange={(x) => form.set("full_name", x.target.value)} onBlur={() => form.blur("full_name")} data-testid="patient-full-name" />
            </Field>
            <Field label={t("patient.field.fatherName")} optional>
              <TextInput value={form.values.father_name} onChange={(x) => form.set("father_name", x.target.value)} data-testid="patient-father-name" />
            </Field>
            <Field label={t("patient.field.gender")} optional>
              <ReferenceSelect typeCode="gender" value={form.values.gender_id} onChange={(id) => form.set("gender_id", id)} testId="patient-gender" />
            </Field>
            <Field label={t("patient.field.dateOfBirth")} optional>
              <TextInput type="date" dir="ltr" value={form.values.date_of_birth} onChange={(x) => form.set("date_of_birth", x.target.value)} data-testid="patient-dob" />
            </Field>
            <Field label={t("patient.field.approximateAge")} optional error={e("approximate_age")}>
              <TextInput type="number" dir="ltr" min={0} max={120} value={form.values.approximate_age} onChange={(x) => form.set("approximate_age", x.target.value)} data-testid="patient-age" />
            </Field>
            <Field label={t("patient.field.preferredLanguage")} optional>
              <Select value={form.values.preferred_language} onChange={(x) => form.set("preferred_language", x.target.value)} data-testid="patient-language">
                <option value="">{t("common.choose")}</option>
                <option value="fa">{t("lang.fa")}</option>
                <option value="ps">{t("lang.ps")}</option>
                <option value="en">{t("lang.en")}</option>
              </Select>
            </Field>
          </div>
        </Card>

        <Card>
          <CardHeader title={t("patient.section.contact")} />
          <div className="grid-2">
            <Field label={t("patient.field.phone")} optional error={e("phone")} hint={t("hint.phone")}>
              <TextInput dir="ltr" value={form.values.phone} onChange={(x) => form.set("phone", x.target.value)} onBlur={() => form.blur("phone")} data-testid="patient-phone" />
            </Field>
            <Field label={t("patient.field.secondaryPhone")} optional error={e("secondary_phone")}>
              <TextInput dir="ltr" value={form.values.secondary_phone} onChange={(x) => form.set("secondary_phone", x.target.value)} onBlur={() => form.blur("secondary_phone")} data-testid="patient-phone-2" />
            </Field>
            <GeoPicker provinceId={form.values.province_id || null} districtId={form.values.district_id || null} onChange={(province, district) => { form.set("province_id", province ?? ""); form.set("district_id", district ?? ""); }} />
            <Field label={t("patient.field.address")} optional className="span-2">
              <Textarea value={form.values.address} onChange={(x) => form.set("address", x.target.value)} data-testid="patient-address" />
            </Field>
          </div>
        </Card>

        <Card>
          <CardHeader title={t("patient.section.emergency")} />
          <div className="grid-2">
            <Field label={t("patient.field.emergencyName")} optional>
              <TextInput value={form.values.emergency_contact_name} onChange={(x) => form.set("emergency_contact_name", x.target.value)} data-testid="patient-emergency-name" />
            </Field>
            <Field label={t("patient.field.emergencyPhone")} optional error={e("emergency_contact_phone")}>
              <TextInput dir="ltr" value={form.values.emergency_contact_phone} onChange={(x) => form.set("emergency_contact_phone", x.target.value)} onBlur={() => form.blur("emergency_contact_phone")} data-testid="patient-emergency-phone" />
            </Field>
            <Field label={t("patient.field.emergencyRelationship")} optional>
              <ReferenceSelect typeCode="relationship" value={form.values.emergency_contact_relationship_id} onChange={(id) => form.set("emergency_contact_relationship_id", id)} testId="patient-emergency-relationship" />
            </Field>
          </div>
        </Card>

        <Card>
          <CardHeader title={t("patient.section.other")} />
          <div className="grid-2">
            <Field label={t("patient.field.referralSource")} optional>
              <ReferenceSelect typeCode="referral_source" value={form.values.referral_source_id} onChange={(id) => form.set("referral_source_id", id)} testId="patient-referral" />
            </Field>
            <Field label={t("patient.field.notes")} optional className="span-2">
              <Textarea value={form.values.notes} onChange={(x) => form.set("notes", x.target.value)} data-testid="patient-notes" />
            </Field>
          </div>
        </Card>

        <div className="row" style={{ justifyContent: "flex-end" }}>
          <Button onClick={onCancel}>{t("common.cancel")}</Button>
          <Button variant="primary" icon={patient ? Save : UserPlus} loading={busy} type="submit" data-testid="patient-save">
            {t(patient ? "common.save" : "patients.add")}
          </Button>
        </div>
      </form>
      {duplicates && duplicates.length > 0 && (
        <DuplicateWarningDialog
          patients={duplicates}
          lang={lang}
          onCancel={() => setDuplicates(null)}
          onContinue={() => {
            setDuplicates(null);
            void doSave(true);
          }}
        />
      )}
    </Page>
  );
}
