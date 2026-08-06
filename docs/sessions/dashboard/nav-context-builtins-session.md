# Session — nav-context built-ins: the typed carriers (lb half)

Scope: [`../../scope/frontend/dashboard/nav-context-builtins-scope.md`](../../scope/frontend/dashboard/nav-context-builtins-scope.md) ·
Issue [#144](https://github.com/NubeDev/lb/issues/144) · Branch `feat/nav-context-builtins`

## What was asked

The `${__nav.*}` / `${__page.*}` namespace is resolved **client-side**; lb's job is only the typed
carriers that let *any* client build the same context, plus the contract stating which strings are
templates. No templating engine in Rust, no server-side expansion, no `SCHEMA_VERSION` bump.

## What shipped

| Where | Change |
|---|---|
| `crates/ext-loader/src/template_refs.rs` (new) | Reference-name extraction for the one grammar — `$name` / `${name}` / `[[name]]`, `[A-Za-z_][\w.]*`, optional `:formathint`; `__`-prefix ⇒ built-in. Hand-rolled scanner, no `regex` dep. |
| `crates/ext-loader/src/manifest.rs` | `NavItem.title_template: Option<String>` (TOML `title_template`, alias `titleTemplate`), `NAV_MAX_TITLE_TEMPLATE = 256`, `validate_nav_templates`, `label` doc-comment. |
| `crates/assets/src/install/model.rs` + `crates/host/src/ui_decl.rs` | `ExtNavItem.title_template`, projected verbatim so it rides `ext.list`. |
| `crates/host/src/nav/model.rs` | `NavItem.title_template` + `ResolvedItem.title_template`, both `rename = "titleTemplate"`, `skip_serializing_if = "Option::is_none"`. |
| `crates/host/src/nav/resolve.rs`, `resolve_template_group.rs` | Relayed on every authored kind; the `template-group` fan-out gives **each generated child** the group's template. |
| `crates/host/src/nav/bounds.rs` | The same cap + the same unbindable-reference reject on the `nav.save` write path. |
| `crates/host/src/dashboard/model.rs` | Doc-comments declaring `Dashboard.heading` / `.description` and `Cell.title` / `.description` template strings the host stores raw. No type change. |

## Calls made

- **`label` warns, `title_template` rejects** (scope §G3 / open question 4), *not* "both reject" as
  the issue checklist reads in summary. `label` is retroactive and the grammar has no `$$` escape, so
  `"Cost $USD"` names `USD` under the extractor — hard-failing it would break shipped manifests in a
  change that claims no wire break. A `tracing::warn!` fires and the manifest loads. The one hard
  reject on `label` stays: `__nav.*`, which is self-referential and cannot exist in shipped data.
  `manifest.rs:a_label_with_an_unbindable_reference_warns_and_still_loads` is the guard — it fails
  loudly if anyone later promotes the warning without shipping `$$` first.
- **Built-ins are classified by the `__` prefix, not matched against a closed list** — the same rule
  `parse.ts:isBuiltinName` uses. A list here would reject a built-in the client already resolves the
  moment the namespace grows.
- **The validator lives in `template_refs`, called from both doors.** Validating only the manifest
  path would make the ext seam the privileged one (rule 10); `nav.save` gets the identical verdict.
- **A `tag-group`'s children do NOT inherit the group's template** — a tag-group expands to many
  *distinct* boards, each with its own stored heading, unlike a template-group's one-board-many-
  bindings fan-out.

## The projection trap

The scope flags this as the highest-probability bug. Reading `nav/store.rs` first: nav does **not**
carry a per-field `SELECT` — `lb_store::read` projects `SELECT data` (the whole envelope) and the
typed struct does the rest, so the trap here is serde-shaped rather than SQL-shaped. The regression
test still goes through the path that would bite either way — a **plain host** nav record written via
`nav.save` and read back through **both** `nav.get` and `nav.resolve`.

## Tests

`crates/host/tests/nav_context_builtins_test.rs` — 6, green on a real `mem://` node. Unit tests in
`template_refs` (7), `manifest.rs` (6 new), `ui_decl.rs` (1 new), `assets/install/model.rs` (extended).

`cargo fmt --all --check` clean for everything touched; `cargo build --workspace` green.
Pre-existing, unrelated failures on this base: 4 in `nav_test.rs` and 1 in `dashboard_test.rs` (all
`add_member` / team-share), and `role/gateway/tests/publish_zip_test.rs` needs a built
`hello_v2_ext.wasm` artifact to compile — all confirmed identical on a clean `origin/master` stash.

## Downstream

`NubeIO/rubix-ai → docs/scope/frontend/dashboard/nav-context-vars-scope.md` — the rendering half.
The wire key it must read is **`titleTemplate`** on both `ResolvedItem` (`nav.resolve`) and
`ExtNavItem` (`ext.list`), omitted entirely when unset.
