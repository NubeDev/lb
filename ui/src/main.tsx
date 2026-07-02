// Publish the shell's React singletons onto `globalThis.__lb*` FIRST (before App renders and before any
// extension remote is dynamic-imported) so the importmap shims can re-export them. The side-effect
// import must precede everything else. See features/ext-host/singletons.ts.
import "./features/ext-host/singletons";

import React from "react";
import ReactDOM from "react-dom/client";

import { App } from "./App";
import "./styles/globals.css";
import "@nube/nav-rail/style.css"; // the reusable nav rail's self-contained tokens + utilities
import "@nube/panel/style.css"; // the reusable resizable panel's self-contained tokens + utilities

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
