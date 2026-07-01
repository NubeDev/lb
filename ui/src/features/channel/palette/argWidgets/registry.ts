// Re-export shim (widget-kit scope, Phase 1) — the x-lb widget registry MOVED to the Widget Kit library
// at `@/lib/widgets/registry` (the library's public resolver, importable by any surface). This shim keeps
// the palette's existing `./argWidgets/registry` import path working during the extraction (behavior-
// preserving move — no assertion changes). New code imports from `@/lib/widgets/registry` directly.
export * from "@/lib/widgets/registry";
