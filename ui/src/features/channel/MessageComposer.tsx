// The composer — one input + send. Presentation + local input state only; the actual post
// is the `onSend` passed down from useChannel (FILE-LAYOUT: markup here, data/effects in the
// hook).

import { useState } from "react";
import { SendHorizontal } from "lucide-react";

interface Props {
  channel: string;
  onSend: (body: string) => void | Promise<void>;
}

export function MessageComposer({ channel, onSend }: Props) {
  const [body, setBody] = useState("");
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const text = body.trim();
    if (!text || busy) return;
    setBody("");
    setBusy(true);
    try {
      await onSend(text);
    } finally {
      setBusy(false);
    }
  }

  return (
    <form
      onSubmit={submit}
      className="flex items-center gap-2 border-t border-border bg-panel/70 p-3"
    >
      <input
        aria-label="message"
        value={body}
        onChange={(e) => setBody(e.target.value)}
        placeholder={`Message #${channel}`}
        className="control-field min-w-0 flex-1"
      />
      <button
        type="submit"
        aria-label="send"
        disabled={!body.trim() || busy}
        className="soft-button h-9 px-3"
      >
        <SendHorizontal size={16} />
      </button>
    </form>
  );
}
