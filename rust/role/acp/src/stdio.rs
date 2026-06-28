//! The stdio JSON-RPC loop (agent-run scope Part 4) — the byte transport an editor launches the
//! adapter with. Newline-delimited JSON: read a frame per line, dispatch it on the [`AcpSession`]
//! driver, write any `session/update` notifications, then the response. Pure I/O — all protocol
//! meaning lives in `session.rs`/`encode.rs`, so this file stays tiny and the driver is testable
//! without a process.
//!
//! **Disconnect-mid-permission (review point 3) falls out of this design, not a special case:** the
//! pause is a *durable suspension* in the store (Part 2), never a held connection. If the editor's
//! stdin closes (the read loop ends) while a run is suspended, nothing is lost — the run stays
//! `Suspended`, the decision settles out-of-band, and the editor reconnects with `session/resume`
//! (which replays the transcript and continues). The connection was never the thing holding the pause.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::rpc::{ErrorResponse, Request, Response};
use crate::session::AcpSession;

/// Run the stdio loop until EOF on `reader` (the editor's stdin). Each line is one JSON-RPC frame.
/// A request (has `id`) gets a response; a notification (no `id`) is dispatched for its side effects
/// but gets no reply (ACP `session/cancel` is a notification). Writes go to `writer` (stdout),
/// flushed per frame so the editor sees streamed updates promptly.
pub async fn serve_stdio<M, R, W>(
    mut session: AcpSession<M>,
    reader: R,
    mut writer: W,
) -> std::io::Result<()>
where
    M: lb_host::ModelAccess + Send + Sync + 'static,
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                // A malformed frame with no id we can echo → a null-id parse error (JSON-RPC).
                write_json(
                    &mut writer,
                    &ErrorResponse::new(
                        serde_json::Value::Null,
                        -32700,
                        format!("parse error: {e}"),
                    ),
                )
                .await?;
                continue;
            }
        };

        let handled = session.handle(&req.method, &req.params).await;

        match (req.id, handled) {
            // A request: stream notifications, then the response (success or error).
            (Some(id), Ok(out)) => {
                for note in &out.notifications {
                    write_json(&mut writer, note).await?;
                }
                write_json(&mut writer, &Response::ok(id, out.result)).await?;
            }
            (Some(id), Err(e)) => {
                write_json(&mut writer, &ErrorResponse::new(id, e.code, e.message)).await?;
            }
            // A notification (no id): dispatch for side effects, emit any notifications it produced,
            // but send NO response (per JSON-RPC).
            (None, Ok(out)) => {
                for note in &out.notifications {
                    write_json(&mut writer, note).await?;
                }
            }
            (None, Err(_)) => { /* a failed notification has no reply channel — swallow */ }
        }
    }
    Ok(())
}

/// Write one JSON value as a newline-delimited frame and flush.
async fn write_json<W, T>(writer: &mut W, value: &T) -> std::io::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
    T: serde::Serialize,
{
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    writer.write_all(&bytes).await?;
    writer.flush().await
}
