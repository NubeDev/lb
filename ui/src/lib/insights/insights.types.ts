// The insights view/DTO types — now OWNED by the `@nube/insights` package (the reusable insights
// machinery, extracted so the page, dashboard widgets, and extensions share ONE implementation). The
// shell re-exports them so `@/lib/insights/insights.types` keeps working for the shell components while
// there is exactly one shape across the stack (the package's, which mirrors the Rust `lb_insights`
// records one-to-one). Add a shell-only view type here if one is ever needed; the wire vocabulary
// lives in the package.

export type {
  Severity,
  Status,
  OriginKind,
  Origin,
  Insight,
  Occurrence,
  InsightEvent,
  PageCursor,
  ListFilter,
  ListQuery,
  ListPage,
  OccCursor,
  OccurrencePage,
} from "@nube/insights";
