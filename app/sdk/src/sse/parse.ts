// Incremental SSE frame parser — feeds on raw text chunks, emits `(event, data)` pairs. Written
// against the wire format the gateway's axum `Sse` emits (`event:`/`data:` lines, blank-line frame
// delimiter, `:` keep-alive comments). Platform-free: the stream layer decides where bytes come from.

/** One parsed SSE frame: the event name (`message` when unnamed) and the raw data payload. */
export type SseFrame = { event: string; data: string };

/** A stateful parser: call `feed(chunk)` with each decoded text chunk; complete frames come back. */
export function createSseParser(): { feed: (chunk: string) => SseFrame[] } {
  let buffer = "";
  return {
    feed(chunk: string): SseFrame[] {
      buffer += chunk;
      const frames: SseFrame[] = [];
      // A frame ends at a blank line. Keep the trailing partial in the buffer.
      let boundary: number;
      while ((boundary = buffer.search(/\r?\n\r?\n/)) !== -1) {
        const raw = buffer.slice(0, boundary);
        buffer = buffer.slice(boundary).replace(/^\r?\n\r?\n/, "");
        const frame = parseFrame(raw);
        if (frame) frames.push(frame);
      }
      return frames;
    },
  };
}

/** Parse one frame's lines. Comment-only frames (keep-alives) return null. */
function parseFrame(raw: string): SseFrame | null {
  let event = "message";
  const data: string[] = [];
  for (const line of raw.split(/\r?\n/)) {
    if (line.startsWith(":") || line === "") continue;
    const colon = line.indexOf(":");
    const field = colon === -1 ? line : line.slice(0, colon);
    // Per spec, one leading space after the colon is stripped.
    const value = colon === -1 ? "" : line.slice(colon + 1).replace(/^ /, "");
    if (field === "event") event = value;
    else if (field === "data") data.push(value);
  }
  if (data.length === 0) return null;
  return { event, data: data.join("\n") };
}
