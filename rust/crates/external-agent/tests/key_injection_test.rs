//! `drive()` injects the resolved API key into the CHILD process env (agent-catalog test-and-secrets
//! scope — the sealed-key → external-agent wiring). Rule 9: NO mock — a REAL subprocess (`sh`), a real
//! pipe, driven through the real `drive`. The child echoes the env var it was given; the test asserts
//! the injected value reached it (and that, with no injection, the child inherits the process env — the
//! documented fallback). This is the leaf-level proof that a per-workspace sealed key can reach the
//! agent WITHOUT living in the node's process env.

use std::time::Duration;

use lb_external_agent::wrapper::{AgentWrapper, Decoded};
use lb_external_agent::{drive, AgentProfile, ModelEndpoint};
use lb_run_events::RunEvent;

/// A trivial real wrapper: spawn `sh -c 'printf "%s\n" "$<ENV>"'` so the child prints the value of the
/// key env var it was launched with. Each non-empty stdout line is projected as a model message
/// (`TextDelta`) so the driver collects it — exactly the shape `final_answer` reads.
struct EchoKeyWrapper {
    env_name: String,
}

impl AgentWrapper for EchoKeyWrapper {
    fn id(&self) -> &'static str {
        "echo-key"
    }

    fn command_args(&self, _profile: &AgentProfile, _goal: &str, _workspace: &str) -> Vec<String> {
        // Print the child's view of the key env var — the whole point of the test.
        vec![
            "-c".into(),
            format!("printf '%s\\n' \"${}\"", self.env_name),
        ]
    }

    fn decode_line(&self, line: &str, turn: u32) -> Decoded {
        let line = line.trim();
        if line.is_empty() {
            return Decoded::Ignore;
        }
        Decoded::Message(vec![RunEvent::TextDelta {
            turn,
            text: line.to_string(),
        }])
    }
}

/// The collected child output = the concatenation of `TextDelta`s (same as the runtime's `final_answer`).
fn text_of(events: &[RunEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            RunEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn profile(env_name: &str) -> AgentProfile {
    AgentProfile {
        id: "echo-key".into(),
        binary: "sh".into(),
        model: ModelEndpoint {
            provider: "test".into(),
            model: "test".into(),
            api_key_env: env_name.into(),
            base_url: None,
        },
    }
}

#[tokio::test]
async fn drive_injects_the_resolved_key_into_the_child_env() {
    // A UNIQUE env name that is NOT set in this process — proving the value came from the injection,
    // not the ambient env. The injected value is the "sealed key" a workspace would have set.
    let env_name = "LB_EXTAGENT_INJECT_TEST_KEY";
    assert!(
        std::env::var(env_name).is_err(),
        "the test env var must be unset in this process"
    );
    let wrapper = EchoKeyWrapper {
        env_name: env_name.into(),
    };
    let workspace = std::env::temp_dir();
    let workspace = workspace.to_str().unwrap();

    let events = drive(
        &wrapper,
        &profile(env_name),
        "unused",
        workspace,
        Duration::from_secs(30),
        Some((env_name, "sealed-key-VALUE-42")), // the resolved sealed key, injected for this child
        &[],
        None,
    )
    .await
    .expect("the child runs");

    assert_eq!(
        text_of(&events).trim(),
        "sealed-key-VALUE-42",
        "the child saw the INJECTED key value in its env (sealed key reached the agent)"
    );
}

#[tokio::test]
async fn no_injection_falls_back_to_the_process_env() {
    // With `None`, the child inherits the process env — the documented fallback (a workspace that set
    // no sealed key keeps working off the node env). We set the var in THIS process and assert the
    // child sees it, WITHOUT any injection.
    let env_name = "LB_EXTAGENT_FALLBACK_TEST_KEY";
    // SAFETY: single-threaded assignment of a test-unique var; removed at the end.
    std::env::set_var(env_name, "from-process-env-99");
    let wrapper = EchoKeyWrapper {
        env_name: env_name.into(),
    };
    let workspace = std::env::temp_dir();
    let workspace = workspace.to_str().unwrap();

    let events = drive(
        &wrapper,
        &profile(env_name),
        "unused",
        workspace,
        Duration::from_secs(30),
        None, // no injection → inherit the process env (the fallback path)
        &[],
        None,
    )
    .await
    .expect("the child runs");

    assert_eq!(
        text_of(&events).trim(),
        "from-process-env-99",
        "with no injection the child inherits the process env (the pre-sealed-key fallback)"
    );
    std::env::remove_var(env_name);
}
