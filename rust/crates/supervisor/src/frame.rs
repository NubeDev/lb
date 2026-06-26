//! `Content-Length` framing over the child's stdio (native-tier scope, re-authored from rubix-cube's
//! `stdio.rs`). One frame = `Content-Length: N\r\n\r\n` followed by exactly N bytes of JSON. This is
//! the LSP/JSON-RPC framing — chosen because it is unambiguous over a byte stream (a length prefix,
//! not a delimiter that could appear in the payload) and is the proven sidecar wire format.
//!
//! Two free verbs over any async reader/writer, so the same code frames a real child's pipes and a
//! test's in-memory duplex. A frame above `MAX_FRAME` is rejected (a malformed/hostile child cannot
//! make the host allocate unbounded memory).

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::SupervisorError;

/// The largest single frame the supervisor will read (16 MiB) — matches the rubix child contract.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Write `payload` as one `Content-Length`-framed message to `w`, flushing it.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    payload: &[u8],
) -> Result<(), SupervisorError> {
    let header = format!("Content-Length: {}\r\n\r\n", payload.len());
    w.write_all(header.as_bytes())
        .await
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
    w.write_all(payload)
        .await
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
    w.flush()
        .await
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
    Ok(())
}

/// Read exactly one `Content-Length`-framed message from `r`, returning its body bytes. Tolerates a
/// header split across reads (the header is read a byte at a time until the blank line). Errors on a
/// closed stream (EOF — the child died), a malformed header, or an over-large frame.
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R) -> Result<Vec<u8>, SupervisorError> {
    let len = read_content_length(r).await?;
    if len > MAX_FRAME {
        return Err(SupervisorError::Transport(format!(
            "frame too large: {len} > {MAX_FRAME}"
        )));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)
        .await
        .map_err(|e| SupervisorError::Transport(format!("reading body: {e}")))?;
    Ok(body)
}

/// Read the header block (one byte at a time, so a partial read can't desync the stream) up to the
/// terminating `\r\n\r\n`, parse the `Content-Length`, and return it.
async fn read_content_length<R: AsyncRead + Unpin>(r: &mut R) -> Result<usize, SupervisorError> {
    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = r
            .read(&mut byte)
            .await
            .map_err(|e| SupervisorError::Transport(format!("reading header: {e}")))?;
        if n == 0 {
            return Err(SupervisorError::Transport("child closed stream".into()));
        }
        header.push(byte[0]);
        if header.ends_with(b"\r\n\r\n") {
            break;
        }
        if header.len() > 8192 {
            return Err(SupervisorError::Transport("header too long".into()));
        }
    }
    parse_content_length(&header)
}

/// Parse `Content-Length: N` (case-insensitive on the field name) out of a header block.
fn parse_content_length(header: &[u8]) -> Result<usize, SupervisorError> {
    let text = std::str::from_utf8(header)
        .map_err(|e| SupervisorError::Transport(format!("header not utf8: {e}")))?;
    for line in text.split("\r\n") {
        if let Some((name, value)) = line.split_once(':') {
            if name.trim().eq_ignore_ascii_case("content-length") {
                return value
                    .trim()
                    .parse::<usize>()
                    .map_err(|e| SupervisorError::Transport(format!("bad content-length: {e}")));
            }
        }
    }
    Err(SupervisorError::Transport(
        "missing Content-Length header".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::duplex;

    #[tokio::test]
    async fn round_trips_a_frame() {
        let (mut a, mut b) = duplex(1024);
        write_frame(&mut a, br#"{"hi":1}"#).await.unwrap();
        let got = read_frame(&mut b).await.unwrap();
        assert_eq!(got, br#"{"hi":1}"#);
    }

    #[tokio::test]
    async fn reads_two_frames_in_sequence() {
        let (mut a, mut b) = duplex(1024);
        write_frame(&mut a, b"one").await.unwrap();
        write_frame(&mut a, b"two").await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap(), b"one");
        assert_eq!(read_frame(&mut b).await.unwrap(), b"two");
    }

    #[tokio::test]
    async fn eof_is_a_transport_error() {
        let (a, mut b) = duplex(1024);
        drop(a); // child closed its end
        let err = read_frame(&mut b).await.unwrap_err();
        assert!(matches!(err, SupervisorError::Transport(_)));
    }

    #[test]
    fn parses_case_insensitive_header() {
        assert_eq!(
            parse_content_length(b"content-length: 42\r\n\r\n").unwrap(),
            42
        );
    }
}
