import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, KeyRound, Laptop, LoaderCircle, Plug, RefreshCw, Server, Wifi } from "lucide-react";
import { latinDigits } from "../lib/dates";
import { discoverServers, pairWithServer, type FoundServer, type PairResult } from "../lib/lan";
import { useForm, v } from "../lib/validation";
import { useI18n } from "../i18n";
import { Button } from "../ui/Button";
import { Field, TextInput } from "../ui/Field";
import { Notice } from "../ui/Feedback";

const failure = (k: string) => `connect.error.${k}`;

/**
 * Connects this computer to the clinic server (roadmap 7.2): the servers on the network are listed
 * by clinic name, so nobody types an address; the 6-digit code shown on the server proves the
 * right person is adding the computer. The address can still be typed when discovery finds nothing.
 */
export function ConnectServer({ onBack, onDone }: { onBack: () => void; onDone: () => void }) {
  const { t } = useI18n();
  const [searching, setSearching] = useState(true);
  const [found, setFound] = useState<FoundServer[]>([]);
  const [picked, setPicked] = useState<string | null>(null);
  const [manual, setManual] = useState(false);
  const [busy, setBusy] = useState(false);
  const [problem, setProblem] = useState("");
  const form = useForm(
    { address: "", code: "", name: "" },
    {
      address: (s) => (manual && !s.trim() ? "rule.required" : manual && !/^[\w.-]+(:\d{2,5})?$/.test(s.trim()) ? "rule.server_address" : null),
      code: (s) => (/^\d{6}$/.test(latinDigits(s).trim()) ? null : s.trim() ? "rule.pair_code" : "rule.required"),
      name: v.required,
    },
  );

  const search = useCallback(() => {
    setSearching(true);
    discoverServers()
      .then((list) => {
        setFound(list);
        setPicked((p) => p ?? (list.length === 1 ? list[0].addr : null));
        if (list.length === 0) setManual(true);
      })
      .catch(() => setManual(true))
      .finally(() => setSearching(false));
  }, []);
  useEffect(search, [search]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setProblem("");
    if (!form.validate()) return;
    const addr = manual ? form.values.address.trim() : picked;
    if (!addr) return setProblem(t("connect.pick"));
    setBusy(true);
    try {
      const r: PairResult = await pairWithServer(addr, latinDigits(form.values.code).trim(), form.values.name.trim());
      if (r.ok) return onDone();
      setProblem(t(failure(r.reason)).replace("{n}", String(r.attempts_left ?? 0)));
    } catch {
      setProblem(t(failure("unreachable")));
    } finally {
      setBusy(false);
    }
  };
  const e = (k: "address" | "code" | "name") => form.error(k) && t(form.error(k)!);

  return (
    <form className="stack" onSubmit={submit} noValidate data-testid="connect-server">
      <div className="setup-head">
        <h1 className="t-title-lg">{t("connect.title")}</h1>
        <p className="muted">{t("connect.hint")}</p>
      </div>

      <div className="stack" aria-busy={searching}>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className="t-overline">{t("connect.found")}</span>
          <Button variant="subtle" size="sm" icon={searching ? LoaderCircle : RefreshCw} disabled={searching} onClick={search} data-testid="connect-search">{searching ? t("connect.searching") : t("connect.again")}</Button>
        </div>
        {found.length > 0 && (
          <div className="server-list" role="radiogroup" aria-label={t("connect.found")}>
            {found.map((s) => (
              <button key={s.addr} type="button" role="radio" aria-checked={!manual && picked === s.addr} aria-pressed={!manual && picked === s.addr} className="server-pick" onClick={() => { setPicked(s.addr); setManual(false); }} data-testid="connect-server-item">
                <Server aria-hidden />
                <span className="stack" style={{ gap: 0 }}>
                  <span className="server-name">{s.clinic_name || t("connect.unnamed")}</span>
                  <span className="server-addr"><bdi className="ltr num">{s.addr}</bdi> · <bdi className="ltr">{s.fp_hint}</bdi></span>
                </span>
              </button>
            ))}
          </div>
        )}
        {!searching && found.length === 0 && <Notice tone="info">{t("connect.none")}</Notice>}
        {!manual && found.length > 0 && <Button variant="link" onClick={() => setManual(true)} data-testid="connect-manual">{t("connect.manual")}</Button>}
        {manual && (
          <Field label={t("connect.address")} hint={t("connect.addressHint")} error={e("address")}>
            <TextInput icon={Wifi} dir="ltr" inputMode="url" autoComplete="off" placeholder="192.168.1.10" value={form.values.address} onChange={(x) => form.set("address", x.target.value)} onBlur={() => form.blur("address")} data-testid="connect-address" />
          </Field>
        )}
      </div>

      <div className="grid-2">
        <Field label={t("connect.code")} hint={t("connect.codeHint")} error={e("code")}>
          <TextInput icon={KeyRound} large className="code-input" inputMode="numeric" autoComplete="one-time-code" maxLength={6} placeholder="000000" value={form.values.code} onChange={(x) => form.set("code", x.target.value)} onBlur={() => form.blur("code")} data-testid="connect-code" />
        </Field>
        <Field label={t("connect.deviceName")} hint={t("connect.deviceNameHint")} error={e("name")}>
          <TextInput icon={Laptop} large maxLength={60} value={form.values.name} onChange={(x) => form.set("name", x.target.value)} onBlur={() => form.blur("name")} data-testid="connect-name" />
        </Field>
      </div>

      {problem && <Notice tone="danger" testId="connect-problem">{problem}</Notice>}
      <div className="setup-actions">
        <Button variant="subtle" icon={ArrowLeft} flipIcon onClick={onBack} data-testid="connect-back">{t("setup.back")}</Button>
        <span className="spacer" />
        <Button type="submit" variant="primary" icon={Plug} loading={busy} data-testid="connect-submit">{t("connect.submit")}</Button>
      </div>
    </form>
  );
}
