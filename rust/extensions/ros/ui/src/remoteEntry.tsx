// remoteEntry.tsx — GENERATED boilerplate. The SDK owns the mount contract (docs/extensions/README.md
// §3a): no hand-written `mount`/`mountWidget`, no `createRoot`, no `document.head` injection.
import styles from "@/styles/tokens.css?inline";
import { defineRemote } from "@nube/ext-ui-sdk";
import { App } from "@/App";

// No bespoke `widgets` map anymore (panel-datasource-query scope supersedes the two former tiles —
// see `extension.toml`'s doc comment): reads go through the generic Datasource track, writes through
// rubix-ai's generic CONTROLS widgets, both against the SAME `ros.*` tools these widgets used to wrap.
export const { mount, mountWidget } = defineRemote({
  id: "ros",
  styles,
  page: (ctx, bridge) => <App ctx={ctx} bridge={bridge} />,
});
