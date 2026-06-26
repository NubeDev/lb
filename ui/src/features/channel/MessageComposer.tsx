// The composer — one input + send. Presentation + local input state only; the actual post
// is the `onSend` passed down from useChannel (FILE-LAYOUT: markup here, data/effects in the
// hook).

import { useState } from "react";
import { SendHorizontal } from "lucide-react";

interface Props {
  onSend: (body: string) => void | Promise<void>;
}

export function MessageComposer({ onSend }: Props) {
  const [body, setBody] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const text = body;
    setBody("");
    await onSend(text);
  }

  return (
    <form
      onSubmit={submit}
      className="flex items-center gap-2 border-t border-border p-3"
    >
      <input
        aria-label="message"
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder="Message #general"
        className="flex-1 rounded-md border border-border bg-bg px-3 py-2 text-sm outline-none focus:border-accent"
      />
      <button
        type="submit"
        aria-label="send"
        className="rounded-md border border-border bg-panel p-2 text-accent hover:border-accent"
      >
        <SendHorizontal size={16} />
      </button>
    </form>
  );
}
