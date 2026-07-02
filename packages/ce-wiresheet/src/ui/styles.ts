// Themed scrollbars for the declarative UI panels. Inline styles can't target
// ::-webkit-scrollbar, so inject a scoped stylesheet once. Everything inside a
// `.ce-ui-root` (the tab host) gets thin dark scrollbars matching the wiresheet.

let injected = false;

export function injectUiStyles(): void {
  if (injected || typeof document === "undefined") return;
  injected = true;
  const css = `
.ce-ui-root { scrollbar-width: thin; scrollbar-color: hsl(var(--input)) transparent; }
.ce-ui-root ::-webkit-scrollbar { width: 10px; height: 10px; }
.ce-ui-root ::-webkit-scrollbar-track { background: transparent; }
.ce-ui-root ::-webkit-scrollbar-thumb {
  background: hsl(var(--border));
  border-radius: 6px;
  border: 2px solid transparent;
  background-clip: padding-box;
}
.ce-ui-root ::-webkit-scrollbar-thumb:hover { background: hsl(var(--input)); background-clip: padding-box; }
.ce-ui-root ::-webkit-scrollbar-corner { background: transparent; }
`;
  const el = document.createElement("style");
  el.setAttribute("data-ce-ui", "");
  el.textContent = css;
  document.head.appendChild(el);
}
