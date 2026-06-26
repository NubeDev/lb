// View/DTO types for the coding-workflow surface — mirror the Rust workflow contract (the approval
// resolution + the gated job start + the outbox effect). One name across the Rust model, the DTO,
// and the client (FILE-LAYOUT).

/** A reviewer's decision on a `needs:approval` inbox item (mirrors `lb_inbox::Decision`). */
export type Decision = "approved" | "rejected" | "deferred";

/** The result of starting a coding job: the durable job id, and whether the approval gate let it
 *  through. `started: false` means the gate refused (awaiting approval) — the genuine S6 gate. */
export interface StartResult {
  jobId: string;
  started: boolean;
}

/** A must-deliver outbox effect, as the UI shows it (mirrors `lb_outbox::Effect`). */
export interface Effect {
  /** The delivery target (`github`, …). */
  target: string;
  /** The action the target performs (`create_pr`, …). */
  action: string;
  /** The stable dedup key the receiver honors. */
  idempotencyKey: string;
  /** Where the effect is in its lifecycle. */
  status: "pending" | "delivered" | "failed";
}
