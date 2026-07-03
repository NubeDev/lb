// View/DTO type for the ui-layout surface — mirrors the Rust `lb_host::UiLayout` model 1:1
// (FILE-LAYOUT: same name across the Rust model, the DTO, and the client).

/** One member's saved layout for one dockable surface. `model` is the client's own layout JSON
 *  (for Data Studio: the FlexLayout model incl. per-tab draft configs) — opaque to the host. */
export interface UiLayout {
  surface: string;
  /** The layout JSON, or `null` when never saved / cleared (render the default layout). */
  model: unknown;
  updated_ts: number;
}
