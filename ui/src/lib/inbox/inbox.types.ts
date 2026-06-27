// View/DTO types for the inbox surface — the durable item mirrors `lb_inbox::Item` (same shape as
// a channel item), and the decision mirrors the Rust `Decision` enum (collaboration scope, slice 4).

export type { Item } from "@/lib/channel/channel.types";

/** A reviewer's decision on an inbox item — the kebab-case discriminants the node speaks. */
export type Decision = "approved" | "rejected" | "deferred";
