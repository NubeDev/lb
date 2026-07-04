// The agent view — invoke the central agent and show its answer. Layout + wiring only; data lives
// in useAgent (FILE-LAYOUT). This is the UI face of the S5 story: a user with the invoke grant gets
// the agent's answer; a user without it sees the node's "denied" — the same gate the Rust
// `agent_test` proves on the backend, surfaced to the user.

import { Bot } from "lucide-react";
import { useState } from "react";

import { useAgent } from "./useAgent";

interface Props {
  ws: string;
  /** The durable session id (the job that survives an edge disconnect). */
  jobId: string;
  /** The current user's principal (the demo session identity until real login lands). */
  author: string;
  /** The caller's held capabilities (the grant the node checks; demo until real tokens). */
  caps: string[];
}

export function AgentView({ ws, jobId, author, caps }: Props) {
  const { result, running, error, run } = useAgent(ws, jobId, author, caps);
  const [goal, setGoal] = useState("");

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Bot size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Agent</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      <form
        className="flex gap-2 border-b border-border px-4 py-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (goal.trim()) void run(goal.trim());
        }}
      >
        <input
          aria-label="goal"
          className="flex-1 rounded-md border border-border bg-panel px-2 py-1 text-sm"
          placeholder="Ask the agent…"
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
        />
        <button
          type="submit"
          disabled={running}
          className="rounded-md bg-accent px-3 py-1 text-sm text-bg disabled:opacity-50"
        >
          {running ? "Running…" : "Run"}
        </button>
      </form>

      {error ? (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error === "denied" ? "You don't have access to the agent." : error}
        </div>
      ) : result ? (
        <article className="flex-1 whitespace-pre-wrap px-4 py-3 text-sm">{result.answer}</article>
      ) : (
        <div className="flex flex-1 items-center justify-center text-sm text-muted">
          Ask the agent something.
        </div>
      )}
    </section>
  );
}
