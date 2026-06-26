// View/DTO types for the native Tier-2 sidecar surface — mirror the Rust `native.*` contract (the
// durable status + the install/restart result). One name across the Rust model, the DTO, and the
// client (FILE-LAYOUT): `NativeStatus`/`Lifecycle` match `lb_host::native`.

/** The lifecycle a sidecar should be in (mirrors `lb_host::Lifecycle`). Durable intent. */
export type Lifecycle = "started" | "stopped";

/** The durable status of one native extension in a workspace (mirrors `lb_host::NativeStatus`),
 *  merged on the wire with the live `running` flag from the runtime sidecar map. */
export interface NativeStatus {
  extId: string;
  version: string;
  lifecycle: Lifecycle;
  /** How many times the supervisor has restarted the child (the supervision proof, surfaced). */
  restartCount: number;
  /** Whether a live child is currently running on the node (runtime map, not the durable record). */
  running: boolean;
}

/** The result of installing/restarting a native sidecar: the version now supervised. */
export interface NativeResult {
  extId: string;
  version: string;
  restartCount: number;
}
