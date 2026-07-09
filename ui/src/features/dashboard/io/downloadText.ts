// Download a text string as a file (the export "save" step). One responsibility: turn a string + a
// filename into a browser download via an object URL + a synthetic anchor click. Guarded for the
// non-DOM (test/SSR) path so importing this module never assumes a `document`. In the Tauri shell the
// same anchor-download works (the webview honors `download`); a native save-dialog is a later polish,
// not a blocker — the bytes are identical.

/** Trigger a download of `text` as `filename` (MIME `application/json` by default). No-op (returns
 *  false) when there is no DOM. */
export function downloadText(
  filename: string,
  text: string,
  mime = "application/json",
): boolean {
  if (
    typeof document === "undefined" ||
    typeof URL === "undefined" ||
    !URL.createObjectURL
  )
    return false;
  const blob = new Blob([text], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoke on the next tick so the click has consumed the URL (a same-tick revoke can cancel it).
  setTimeout(() => URL.revokeObjectURL(url), 0);
  return true;
}
