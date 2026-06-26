// View/DTO types for the agent surface — mirror the Rust agent contract (the `invoke` result +
// the durable session). One name across the Rust model, the DTO, and the client (FILE-LAYOUT).

/** The result of invoking the central agent: its final answer + the durable session id. */
export interface AgentResult {
  /** The agent's final text answer (the loop's last model content). */
  answer: string;
  /** The durable job/session id — survives the edge disconnecting; resumable on the hub. */
  jobId: string;
}
