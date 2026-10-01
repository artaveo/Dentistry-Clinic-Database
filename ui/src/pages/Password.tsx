import { useState } from "react";
import { rpc } from "../lib/api";
import { useI18n } from "../i18n";

export function PasswordPage() {
  const { t, err } = useI18n();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (next !== repeat) return setMsg({ ok: false, text: t("error.mismatch") });
    try {
      await rpc("auth.change_password", { current_password: current, new_password: next });
      setCurrent(""); setNext(""); setRepeat("");
      setMsg({ ok: true, text: t("password.changed") });
    } catch (x) {
      setMsg({ ok: false, text: err(x) });
    }
  };
  return (
    <form className="card" onSubmit={submit}>
      <h2>{t("password.title")}</h2>
      <label>{t("password.current")}</label><input type="password" value={current} onChange={(e) => setCurrent(e.target.value)} />
      <label>{t("password.new")}</label><input type="password" value={next} onChange={(e) => setNext(e.target.value)} />
      <label>{t("login.passwordRepeat")}</label><input type="password" value={repeat} onChange={(e) => setRepeat(e.target.value)} />
      {msg && <div className={msg.ok ? "success" : "error"}>{msg.text}</div>}
      <div className="actions"><button className="primary" type="submit">{t("common.save")}</button></div>
    </form>
  );
}
