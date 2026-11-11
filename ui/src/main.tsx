import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
// Per-area stylesheets (`styles/<area>.css`) load after the base file, so they can refine it.
import.meta.glob("./styles/*.css", { eager: true });
import { installGlobalErrorLogging } from "./lib/diagnostics";

installGlobalErrorLogging();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
