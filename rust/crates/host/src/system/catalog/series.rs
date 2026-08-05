//! The `series.*` / `ingest.*` families — the generic ingest + read surface (ingest scope).
//!
//! One family group per file (FILE-LAYOUT); assembled by the parent `mod.rs`.

use super::HostTool;

/// The catalog rows for this family group.
pub(super) const SERIES: &[HostTool] = &[
    // series.* / ingest.* — the generic ingest + read surface (ingest scope).
    HostTool {
        tool: "series.list",
        group: "series",
        description: "list the series (metrics) in the workspace",
    },
    HostTool {
        tool: "series.latest",
        group: "series",
        description: "the latest committed value of a series",
    },
    HostTool {
        tool: "series.latest_many",
        group: "series",
        description: "the latest committed value of each named series, in one round-trip",
    },
    HostTool {
        tool: "series.find",
        group: "series",
        description: "find series by tag/name match",
    },
    HostTool {
        tool: "series.read",
        group: "series",
        description: "read a committed range of a series (keyset-paged rows or decimated buckets)",
    },
    HostTool {
        tool: "series.retention.set",
        group: "series",
        description: "set the retention policy for a series prefix: raw time horizon (raw_for_ms), \
                      FIFO sample cap (max_samples, 0 = unbounded), and rollup tiers",
    },
    HostTool {
        tool: "series.retention.list",
        group: "series",
        description: "list the workspace's series retention policies",
    },
    HostTool {
        tool: "series.retention.status",
        group: "series",
        description: "the retention policy in force for ONE series or prefix after longest-prefix \
                      resolution (with the winning prefix named), plus the workspace's last GC pass",
    },
    HostTool {
        tool: "series.retention.patch",
        group: "series",
        description: "change SOME fields of a retention policy, keeping the rest — read-modify-write \
                      in one call. Absent keys keep their stored value, and a supplied tier is merged \
                      field-wise with the stored tier of the same width, so re-sending a tier without \
                      its method does not silently drop the method (which `set`, a whole-row replace, \
                      does by design)",
    },
    HostTool {
        tool: "series.retention.delete",
        group: "series",
        description: "delete a series retention policy (revert to keep-forever)",
    },
    HostTool {
        tool: "series.stats",
        group: "series",
        description: "what ONE series holds: raw vs rolled-up counts, first/last sample time, and \
                      the producers writing to it (single-subject — never fan this out)",
    },
    HostTool {
        tool: "series.rollup.read",
        group: "series",
        description: "read the STORED rollup rows of ONE series (`series_rollup`) verbatim, on the \
                      tier's own grid, with the full stat set (min/max/sum/num_count/count/last/ \
                      first). Distinct from `series.read {mode:\"buckets\"}`, which decimates live \
                      raw and merges the stored tail beneath it: this never merges and never falls \
                      back, so an empty result means \"nothing is folded here\" — a real answer, \
                      not an absence of one (single-subject — never fan this out)",
    },
    HostTool {
        tool: "series.producer.health",
        group: "series",
        description: "ask the producers of ONE series what they report about their own ingest \
                      (discovered by tool-name convention; a producer that is not an extension, or \
                      declares no health tool, says so rather than looking healthy)",
    },
    HostTool {
        tool: "series.retention.gc",
        group: "series",
        description: "run one retention pass now: roll up then evict raw samples past the time \
                      horizon or over the sample cap (a reactor also ticks this on a cadence)",
    },
    HostTool {
        tool: "ingest.write",
        group: "ingest",
        description: "write a sample into the exactly-once ingest buffer",
    },
    HostTool {
        tool: "series.delete",
        group: "series",
        description: "delete one series and every sample under it (irreversible)",
    },
    HostTool {
        tool: "series.rename",
        group: "series",
        description: "move a series to a new name, carrying its samples and policy",
    },
];
