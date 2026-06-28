//! Generated Tier-2 native sidecar. It speaks the same `lb-supervisor` framed protocol as the shipped
//! native reference extensions and keeps no durable state in the child process.

use lb_supervisor::{read_frame, write_frame, CallParams, Method, Reply, Request};
use tokio::io::{stdin, stdout};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let ws = std::env::var("LB_EXT_WS").unwrap_or_default();
    let ext_id = std::env::var("LB_EXT_ID").unwrap_or_else(|_| "devkit-build-native-3059719".into());
    let mut input = stdin();
    let mut output = stdout();

    loop {
        let body = match read_frame(&mut input).await {
            Ok(body) => body,
            Err(_) => break,
        };
        let req: Request = match serde_json::from_slice(&body) {
            Ok(req) => req,
            Err(_) => continue,
        };
        let reply = match req.method {
            Method::Init => Reply::ok(req.id, format!(r#"{{"ready":true,"ext":"devkit-build-native-3059719"}}"#)),
            Method::Health => Reply::ok(req.id, "ok"),
            Method::Shutdown => {
                let bytes = serde_json::to_vec(&Reply::ok(req.id, "bye")).unwrap();
                let _ = write_frame(&mut output, &bytes).await;
                break;
            }
            Method::Call => handle_call(&req, &ws, &ext_id),
        };
        let bytes = serde_json::to_vec(&reply).unwrap();
        if write_frame(&mut output, &bytes).await.is_err() {
            break;
        }
    }
}

fn handle_call(req: &Request, ws: &str, ext_id: &str) -> Reply {
    let params: CallParams = match serde_json::from_str(&req.params) {
        Ok(params) => params,
        Err(e) => return Reply::err(req.id, format!("bad params: {e}")),
    };
    match params.tool.as_str() {
        "ping" => Reply::ok(
            req.id,
            format!(r#"{{"ok":true,"ext":"devkit-build-native-3059719","runtime_ext":"{ext_id}","ws":"{ws}","tier":"native"}}"#),
        ),
        other => Reply::err(req.id, format!("unknown tool: {other}")),
    }
}
