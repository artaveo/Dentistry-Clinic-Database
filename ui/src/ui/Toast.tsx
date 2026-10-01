import { createContext, useCallback, useContext, useState } from "react";
import type { ReactNode } from "react";
import { CircleAlert, CircleCheck, Info } from "lucide-react";

type Tone = "success" | "danger" | "info";
type ToastItem = { id: number; tone: Tone; text: string };

const ToastContext = createContext<(tone: Tone, text: string) => void>(() => {});

/** Success/error feedback that never shifts the page layout (Raycast pattern). */
export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const show = useCallback((tone: Tone, text: string) => {
    const id = Date.now() + Math.random();
    setItems((xs) => [...xs.slice(-2), { id, tone, text }]);
    window.setTimeout(() => setItems((xs) => xs.filter((x) => x.id !== id)), tone === "danger" ? 6000 : 4000);
  }, []);
  const Icon = { success: CircleCheck, danger: CircleAlert, info: Info };
  return (
    <ToastContext.Provider value={show}>
      {children}
      <div className="toasts" aria-live="polite">
        {items.map((x) => {
          const I = Icon[x.tone];
          return (
            <div key={x.id} className={`glass-overlay toast ${x.tone}`} role={x.tone === "danger" ? "alert" : "status"} data-testid="toast">
              <I aria-hidden />
              <span className="toast-text">{x.text}</span>
            </div>
          );
        })}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const show = useContext(ToastContext);
  return {
    success: (text: string) => show("success", text),
    error: (text: string) => show("danger", text),
    info: (text: string) => show("info", text),
  };
}
