// Minimal i18n for Phase 1 (full engine in Phase 2). Every UI string lives
// in fa/ps/en JSON (DoD rule 4); `scripts/check-i18n.mjs` keeps keys in sync.
import { createContext, useContext } from "react";
import type { Language } from "../../../shared/ts/contract";
import fa from "./fa.json";
import ps from "./ps.json";
import en from "./en.json";
import { ApiError } from "../lib/api";

export type Key = keyof typeof fa;
const dictionaries: Record<Language, Record<Key, string>> = { fa, ps, en };

export function translate(lang: Language, key: Key | string): string {
  return (dictionaries[lang] as Record<string, string>)[key] ?? (fa as Record<string, string>)[key] ?? key;
}

export function dirOf(lang: Language): "rtl" | "ltr" {
  return lang === "en" ? "ltr" : "rtl";
}

export function errorText(lang: Language, e: unknown): string {
  return e instanceof ApiError ? translate(lang, `error.${e.code}`) : translate(lang, "error.internal");
}

export const LangContext = createContext<{ lang: Language; setLang: (l: Language) => void }>({ lang: "fa", setLang: () => {} });

export function useI18n() {
  const { lang, setLang } = useContext(LangContext);
  return { lang, setLang, t: (key: Key | string) => translate(lang, key), err: (e: unknown) => errorText(lang, e) };
}
