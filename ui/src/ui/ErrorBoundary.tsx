import { Component, type ReactNode } from "react";
import { TriangleAlert } from "lucide-react";
import { reportUiError } from "../lib/diagnostics";
import { useI18n } from "../i18n";
import { Button } from "./Button";

function Fallback({ onRetry }: { onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <div className="auth">
      <div className="glass auth-card stack" role="alert" data-testid="crash-screen">
        <h1 className="t-title-lg"><TriangleAlert aria-hidden /> {t("crash.title")}</h1>
        <p className="muted">{t("crash.body")}</p>
        <Button variant="primary" onClick={onRetry} data-testid="crash-retry">{t("crash.retry")}</Button>
      </div>
    </div>
  );
}

/** A screen that throws must not leave a white window: the error goes to the log, the user gets a way on. */
export class ErrorBoundary extends Component<{ children: ReactNode }, { failed: boolean }> {
  state = { failed: false };

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    reportUiError("react screen crashed", error);
  }

  render() {
    return this.state.failed ? <Fallback onRetry={() => this.setState({ failed: false })} /> : this.props.children;
  }
}
