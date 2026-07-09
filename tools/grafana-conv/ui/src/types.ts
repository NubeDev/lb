// The conversion-report wire types — a TypeScript mirror of the Rust
// `ConversionReport` / `ReportLine` / `Fate` (mapper/src/report.rs). The UI does
// not interpret these beyond rendering; the mapper is the source of truth.

export type Fate = "mapped" | "degraded" | "dropped";

export interface ReportLine {
  /** Stable slug (e.g. `"panel.repeat"`, `"var.adhoc"`). */
  code: string;
  fate: Fate;
  /** Where in the input it appeared (`"panels[2]"`, `""` for dashboard-level). */
  at: string;
  /** One-sentence reason. */
  reason: string;
}

export interface ConversionReport {
  mapped: ReportLine[];
  degraded: ReportLine[];
  dropped: ReportLine[];
}

export interface ConvertResponse {
  dashboard: unknown;
  report: ConversionReport;
}

export interface ConvertError {
  error: string;
}
