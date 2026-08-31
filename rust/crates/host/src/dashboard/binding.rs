//! What a cell BINDS to — the four types that describe where a cell's data comes from and what a
//! control writes back: [`Source`] (the v2 tool binding), [`Action`] (a control's write tool),
//! [`Target`] (a v3 query target) and [`QueryOptions`] (the per-target knobs).
//!
//! Split out of `model.rs` (FILE-LAYOUT): the record shape and the binding vocabulary are edited for
//! different reasons — a new page setting touches `Dashboard`, a new query knob touches `Target`.
//! Re-exported through `model.rs`, so every `super::model::Target` path still resolves.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::null_default::null_default;

/// A cell's data source, v2: ANY MCP tool call (read or write) in the install grant — not the
/// frozen four series verbs (widget-builder scope, "The widget contract, v2"). The forwardable set
/// is `cell.tools ∩ install-grant`, re-checked at the host per call. A v1 cell carries no `source`
/// and falls back to `binding`; a v2 cell names `{ tool, args }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Source {
    /// The MCP tool the cell reads (or, for a control, the read it reflects). E.g. `series.read`,
    /// `series.watch`, `<ext>.<verb>`.
    pub tool: String,
    /// The arguments passed to `tool` (opaque to the host; re-checked per call).
    #[serde(default, deserialize_with = "null_default")]
    pub args: Value,
}

/// A control's write action, v2: the tool a `switch`/`slider`/`button` CALLS on interaction
/// (widget-builder scope, "Control views"). `args_template` is a typed template with one `{{value}}`
/// slot the interaction fills (the slider value, the switch state). The write tool is gated by its
/// own existing capability, re-checked at the host per call — the cell invents no new cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Action {
    /// The write tool invoked on interaction. E.g. `mqtt.publish`, `ingest.write`, `<ext>.<verb>`.
    pub tool: String,
    /// The argument template; a `{{value}}` token (any string leaf) is substituted with the control
    /// state on interaction. Opaque to the host. `rename = "argsTemplate"` because the entire platform
    /// speaks camelCase on the wire — the UI, `flowBindingOfAction`, every reminder descriptor, the
    /// `dashboard.pin` envelope — exactly like the sibling `Target::ref_id`'s `refId`. Without it a
    /// flow-bound switch/slider lost its `flows.inject` binding on every `dashboard.save`/`get`
    /// (stored `null`, read `undefined`), so the flow-fed-widgets feature read as entirely dead.
    #[serde(default, deserialize_with = "null_default", rename = "argsTemplate")]
    pub args_template: Value,
}

/// A v3 **target** — a Grafana "target": one query against one datasource (viz panel-model scope).
/// Generalizes the single [`Source`] to an ordered `sources[]`; each carries a `ref_id` (A, B, …)
/// referenced by transformations + overrides, and an optional `datasource` ref. A v2 single-`source`
/// cell reads as `sources[0]` through the UI adapter; the host stores whatever the client sends. The
/// datasource ref is opaque `Value` here — the host does not interpret it (datasource-binding scope
/// owns its resolution, leashed by the target tool's cap ∩ grant, re-checked per call).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Target {
    /// `"A"` | `"B"` | … — referenced by transformations + field overrides.
    #[serde(default, deserialize_with = "null_default", rename = "refId")]
    pub ref_id: String,
    /// Which datasource (native | series | federation | ext). Opaque to the host.
    #[serde(default, deserialize_with = "null_default")]
    pub datasource: Value,
    /// The resolved MCP tool (`store.query` | `series.read` | `federation.query` | ext tool).
    #[serde(default, deserialize_with = "null_default")]
    pub tool: String,
    /// The query args (opaque; re-checked per call, exactly like [`Source::args`]).
    #[serde(default, deserialize_with = "null_default")]
    pub args: Value,
    /// Skip this target's data (Grafana parity).
    #[serde(default, deserialize_with = "null_default")]
    pub hide: bool,
    /// CONDITIONAL target (conditional-targets scope) — a `${var} == value` expression saying when
    /// this target RUNS. Blank ⇒ always. The client evaluates it against the current variable scope
    /// and resolves it to a plain `hide` before dispatch, so nothing below has to understand it; the
    /// record has to carry the expression, which is why it is typed here rather than left to
    /// `args`. It is what makes N alternative baselines on one panel — previous period, a chosen
    /// site, the estate average, selected by one `comparison` variable — cost ONE query, not N.
    ///
    /// Untyped, this field was silently DROPPED on every `dashboard.save`: serde discards unknown
    /// keys, so an authored report round-tripped through the host with its comparison gating erased
    /// and every baseline drawing at once. Same failure as the query-options regression this module
    /// already carries a pin for (`docs/debugging/dashboard/query-options-silently-dropped-on-save.md`).
    #[serde(
        default,
        rename = "showWhen",
        deserialize_with = "null_default",
        skip_serializing_if = "String::is_empty"
    )]
    pub show_when: String,
}

/// Panel-level query options (viz grafana-parity-backend scope, P1) — the editor's "Query options"
/// block plus Grafana's per-panel time override. Typed (not opaque `Value`) because `viz.query`
/// interprets `timeFrom`/`timeShift` when dispatching targets; the rest ride to the client. All
/// fields additive/null-defaulted; the whole struct is skip-if-default so a pre-P1 cell round-trips
/// byte-stable. Regression pin: before P1 the UI sent this as a top-level cell field and the closed
/// `Cell` struct silently DROPPED it on `dashboard.save`
/// (`docs/debugging/dashboard/query-options-silently-dropped-on-save.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QueryOptions {
    /// Cap on returned points per target (0 = unset; the editor's "Max data points").
    #[serde(default, deserialize_with = "null_default", rename = "maxDataPoints")]
    pub max_data_points: u64,
    /// Minimum bucket interval, a duration string (`"10s"`; empty = unset).
    #[serde(default, deserialize_with = "null_default", rename = "minInterval")]
    pub min_interval: String,
    /// The shipped UI's relative-time field (pre-P1 vocabulary, kept verbatim; empty = unset).
    #[serde(default, deserialize_with = "null_default", rename = "relativeTime")]
    pub relative_time: String,
    /// Grafana panel time override: replaces the range with `[now - timeFrom, now]` (`"6h"`).
    #[serde(default, deserialize_with = "null_default", rename = "timeFrom")]
    pub time_from: String,
    /// Grafana panel time shift: moves BOTH range ends earlier by this duration (`"1d"`).
    #[serde(default, deserialize_with = "null_default", rename = "timeShift")]
    pub time_shift: String,
    /// Display-only (Grafana parity): hide the override badge in the panel header. Never affects
    /// the query.
    #[serde(
        default,
        deserialize_with = "null_default",
        rename = "hideTimeOverride"
    )]
    pub hide_time_override: bool,
}

impl QueryOptions {
    /// True when every field is unset — the skip-serializing predicate (a pre-P1 cell stays
    /// byte-stable on the wire and the record).
    pub fn is_empty(&self) -> bool {
        *self == QueryOptions::default()
    }
}
