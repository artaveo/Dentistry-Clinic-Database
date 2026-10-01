import { useEffect, useState } from "react";
import type { AppStatus, Language, SessionInfo } from "../../shared/ts/contract";
import { rpc, setToken } from "./lib/api";
import { LangContext, dirOf, errorText } from "./i18n";
import { ThemeContext, applyPerf, applyTheme, savePerf, saveTheme, storedPerf, storedTheme } from "./theme";
import type { PerfMode, ThemeChoice } from "./theme";
import { Setup } from "./setup/Wizard";
import { Login } from "./pages/Login";
import { Shell } from "./pages/Shell";

const LANG_KEY = "artaveo.lang";

function storedLang(): Language | null {
  try {
    const v = localStorage.getItem(LANG_KEY);
    return v === "fa" || v === "ps" || v === "en" ? v : null;
  } catch {
    return null;
  }
}

export function App() {
  const [lang, setLangState] = useState<Language>(storedLang() ?? "fa");
  const [theme, setThemeState] = useState<ThemeChoice>(storedTheme());
  const [perf, setPerfState] = useState<PerfMode>(storedPerf());
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [bootError, setBootError] = useState<unknown>(null);
  const [session, setSession] = useState<SessionInfo | null>(null);

  const setLang = (l: Language) => {
    setLangState(l);
    try {
      localStorage.setItem(LANG_KEY, l);
    } catch {
      /* per-device convenience only */
    }
  };
  const setTheme = (t: ThemeChoice) => {
    setThemeState(t);
    saveTheme(t);
  };
  const setPerf = (p: PerfMode) => {
    setPerfState(p);
    savePerf(p);
  };

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.dir = dirOf(lang);
  }, [lang]);
  useEffect(() => applyTheme(theme), [theme]);
  useEffect(() => applyPerf(perf), [perf]);

  const refresh = () =>
    rpc("app.status", {})
      .then((s) => {
        setStatus(s);
        if (!storedLang()) setLangState(s.default_language);
      })
      .catch(setBootError);

  useEffect(() => {
    refresh();
  }, []);

  const signOut = () => {
    setToken(null);
    setSession(null);
  };

  let page;
  if (bootError) page = <div className="center"><div className="card error" data-testid="boot-error">{errorText(lang, bootError)}</div></div>;
  else if (!status) page = <div className="center muted">…</div>;
  else if (status.state === "needs_setup") page = <Setup onDone={refresh} />;
  else if (!session) page = <Login clinicName={status.clinic_name} onLogin={(s) => { setToken(s.token); setSession(s); }} />;
  else page = <Shell session={session} clinicName={status.clinic_name ?? ""} onSignOut={signOut} />;

  return (
    <LangContext.Provider value={{ lang, setLang }}>
      <ThemeContext.Provider value={{ theme, setTheme, perf, setPerf }}>{page}</ThemeContext.Provider>
    </LangContext.Provider>
  );
}
