//! A **real HTTP token endpoint** on a real socket, for the XOAUTH2 refresh tests.
//!
//! Hand-rolled rather than mocked (testing §0): `reqwest` really connects, really posts the form, and
//! really parses the response — so the tests prove the exchange shape (RFC 6749 §6) and the caching
//! behaviour, not our idea of them. It counts hits, which is how "exactly one refresh" is provable.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A running token endpoint. Drop it and the accept loop stops.
pub struct TestTokenEndpoint {
    addr: SocketAddr,
    hits: Arc<AtomicUsize>,
    bodies: Arc<Mutex<Vec<String>>>,
    _task: tokio::task::JoinHandle<()>,
}

impl TestTokenEndpoint {
    /// Serve `status` + `body` for every request.
    pub async fn start(status: u16, body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local_addr");
        let hits = Arc::new(AtomicUsize::new(0));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let task_hits = hits.clone();
        let task_bodies = bodies.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let hits = task_hits.clone();
                let bodies = task_bodies.clone();
                let body = body.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = stream.read(&mut buf).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buf[..n]).to_string();
                    hits.fetch_add(1, Ordering::SeqCst);
                    if let Some((_, form)) = request.split_once("\r\n\r\n") {
                        bodies.lock().unwrap().push(form.to_string());
                    }
                    let reason = if status == 200 { "OK" } else { "Error" };
                    let response = format!(
                        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.flush().await;
                });
            }
        });
        Self {
            addr,
            hits,
            bodies,
            _task: task,
        }
    }

    /// A token endpoint that mints `access_token` with the given lifetime.
    pub async fn minting(access_token: &str, expires_in: u64) -> Self {
        Self::start(
            200,
            format!(r#"{{"access_token":"{access_token}","expires_in":{expires_in}}}"#),
        )
        .await
    }

    pub fn url(&self) -> String {
        format!("http://{}/token", self.addr)
    }

    /// How many requests the endpoint has served — the "exactly one refresh" assertion.
    pub fn hits(&self) -> usize {
        self.hits.load(Ordering::SeqCst)
    }

    /// The form bodies received, so a test can assert the grant shape actually sent.
    pub fn bodies(&self) -> Vec<String> {
        self.bodies.lock().unwrap().clone()
    }
}
