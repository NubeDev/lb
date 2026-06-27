# Observability — structured logs, traces, metrics

> **TODO (stub).** Not shipped yet. The ask lives in
> [`scope/observability/observability-scope.md`](../../scope/observability/observability-scope.md);
> this becomes the source-of-truth "as built" doc when the S10 slice ships.

Planned: every node **emits** OpenTelemetry-shaped signal via the Rust `tracing` ecosystem — spans
around each mediated tool call / store tx / bus route / job step / relay delivery, structured logs
within them, and metrics (tool latency, **capability-deny count**, sync lag, outbox retries) — with
a `trace_id` that **propagates across the routed Zenoh hop** and into jobs/outbox effects. Secret-safe
by construction (`Secret<T>` redaction + params-digest). Config-selected sink (file on edge, OTLP to a
collector on the hub); collection/dashboards are external, not in-core. One of three projections of
the host chokepoint (see the audit and undo docs).
</content>
