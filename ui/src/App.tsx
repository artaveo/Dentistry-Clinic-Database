import { useEffect, useState } from "react";
import type { AppStatus, Language, SessionInfo } from "../../shared/ts/contract";
import { ApiError, rpc, setToken } from "./lib/api";
import { appMode, type AppMode } from "./lib/lan";
import { applyBrand, storedBrand } from "./lib/color";
import { LangContext, dirOf, errorText } from "./i18n";
import { ThemeContext, applyPerf, applyTheme, savePerf, saveTheme, storedPerf, storedTheme } from "./theme";
import type { PerfMode, ThemeChoice } from "./theme";
import { Setup } from "./setup/Wizard";
import { Login } from "./pages/Login";
import { Shell } from "./pages/Shell";
import { ToastProvider } from "./ui/Toast";
import { ErrorState } from "./ui/Feedback";
import { ServerOffline } from "./shell/ServerOffline";
import { brandAssets } from "./ui/Brand";
import { ErrorBoundary } from "./ui/ErrorBoundary";
import { SafeMode } from "./setup/SafeMode";
import { startupState, type StartupState } from "./lib/safe";

const LANG_KEY = "artaveo.lang";
/** The splash (Artaveo, roadmap 2.1b) stays at least this long so it never flickers. */
const SPLASH_MS = 900;

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
  const [logo, setLogo] = useState<string | null>(null);
  const [bootError, setBootError] = useState<unknown>(null);
  // Safe Startup Mode (roadmap 11.3): the clinic did not open.
  const [safe, setSafe] = useState<Extract<StartupState, { safe: true }> | null>(null);
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [splash, setSplash] = useState<"on" | "fading" | "off">("on");
  // Whether this computer works on its own database or through the clinic server (roadmap 7.2).
  const [mode, setMode] = useState<AppMode>({ mode: "local" });

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
  useEffect(() => {
    applyBrand(...storedBrand());
    const t1 = window.setTimeout(() => setSplash("fading"), SPLASH_MS);
    const t2 = window.setTimeout(() => setSplash("off"), SPLASH_MS + 380);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, []);

  const refresh = async () => {
    appMode().then(setMode);
    const startup = await startupState();
    if (startup.safe) {
      setSafe(startup);
      return;
    }
    setSafe(null);
    return rpc("app.status", {})
      .then((s) => {
        setStatus(s);
        setBootError(null);
        if (!storedLang()) setLangState(s.default_language);
        if (s.state === "ready") rpc("app.clinic_logo", {}).then((l) => setLogo(l.data_url)).catch(() => setLogo(null));
      })
      .catch(setBootError);
  };

  useEffect(() => {
    refresh();
  }, []);

  const signOut = () => {
    setToken(null);
    setSession(null);
  };

  let page;
  if (safe) page = <SafeMode state={safe} onResolved={refresh} />;
  else if (bootError instanceof ApiError && bootError.code === "server_unreachable") page = <ServerOffline onRetry={refresh} />;
  else if (bootError)
    page = (
      <div className="auth">
        <div className="glass auth-card" data-testid="boot-error">
          <ErrorState message={errorText(lang, bootError)} onRetry={refresh} />
        </div>
      </div>
    );
  else if (!status) page = null;
  else if (status.state === "needs_setup") page = <Setup onDone={refresh} />;
  else if (!session)
    page = <Login clinicName={status.clinic_name} logo={logo} version={status.version} onLogin={(s) => { setToken(s.token); setSession(s); }} />;
  else
    page = (
      <Shell session={session} clientOf={mode.mode === "client" ? mode.server_name : null} clinicName={status.clinic_name ?? ""} logo={logo} version={status.version} onLogoChange={setLogo} onSignOut={signOut} />
    );

  return (
    <LangContext.Provider value={{ lang, setLang }}>
      <ThemeContext.Provider value={{ theme, setTheme, perf, setPerf }}>
        <ToastProvider>
          <ErrorBoundary>{page}</ErrorBoundary>
          {splash !== "off" && (
            <div className={`splash ${splash === "fading" && status ? "done" : ""}`} data-testid="splash" aria-hidden>
              <img src={brandAssets.splashDark} alt="" />
              <div className="splash-bar" />
            </div>
          )}
        </ToastProvider>
      </ThemeContext.Provider>
    </LangContext.Provider>
  );
}
