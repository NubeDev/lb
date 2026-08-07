//! `LB_STORE_OPEN_UNGUARDED` → `BootConfig::store_open_unguarded`, and what boot does when the
//! store memory guard refuses (boot-memory-guard scope, issue #128, slice 2).
//!
//! Two properties, both load-bearing for the incident this scope closes:
//!   1. The env seam parses at the **binary boundary** only — exactly `1` disables the guard, any
//!      other value warns and leaves it ON, and nothing panics over a typo.
//!   2. A refused open **fails boot** and never falls back to `mem://`. A silently-empty node
//!      serving a workspace that "lost" its data is strictly worse than a down node with a legible
//!      reason (scope decision 3).
//!
//! Real store, real files: the refusal case boots against a store this test actually seeded.

use lb_node::{boot_full, BootConfig};

#[test]
fn unguarded_override_parses_only_the_exact_value_one() {
    // One `#[test]`: env is process-global, so the cases run in sequence rather than racing across
    // cargo's test threads (the `store_budget_config_test` precedent).
    std::env::remove_var("LB_STORE_OPEN_UNGUARDED");
    assert!(
        !BootConfig::from_env().store_open_unguarded,
        "unset ⇒ the guard is ON (today's protection, not opt-in)"
    );

    std::env::set_var("LB_STORE_OPEN_UNGUARDED", "1");
    assert!(BootConfig::from_env().store_open_unguarded);
    std::env::set_var("LB_STORE_OPEN_UNGUARDED", " 1 ");
    assert!(
        BootConfig::from_env().store_open_unguarded,
        "surrounding whitespace is tolerated, as everywhere else in the boot config"
    );

    // Anything else warns and leaves the guard ON — a typo must never quietly remove the guard that
    // keeps the box reachable. The assertion is also that this RETURNS: no panic at boot config.
    for bad in ["0", "true", "yes", "on", "", "   ", "11", "1x"] {
        std::env::set_var("LB_STORE_OPEN_UNGUARDED", bad);
        assert!(
            !BootConfig::from_env().store_open_unguarded,
            "'{bad}' does not disable the guard"
        );
    }

    std::env::remove_var("LB_STORE_OPEN_UNGUARDED");
    assert!(!BootConfig::from_env().store_open_unguarded);
}

/// A store the machine cannot replay fails boot with the diagnostic — and the SAME config with the
/// override set boots the same directory. `store_available_ram_bytes` is the embedder seam (a
/// cgroup limit is a truer ceiling than the host's `MemAvailable`); here it supplies the number the
/// incident box supplied for real.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn boot_fails_loudly_when_the_store_will_not_fit_and_the_override_boots_it() {
    let parent = std::env::temp_dir().join(format!("lb-boot-guard-{}", lb_store::new_ulid()));
    let path = parent.join("store");
    std::fs::create_dir_all(&parent).unwrap();
    let path = path.to_string_lossy().into_owned();

    // A real store with real records, written through the real write path.
    {
        let store = lb_store::Store::open(&path).await.unwrap();
        for k in 0..16u64 {
            lb_store::write(
                &store,
                "nube",
                "kv",
                &format!("k{k}"),
                &serde_json::json!({ "pad": "x".repeat(512) }),
            )
            .await
            .unwrap();
        }
    }

    let base = || {
        let mut c = BootConfig::default();
        c.store_path = Some(path.clone());
        c.gateway = lb_node::GatewayMode::Off;
        c.reactors = false;
        c.hello_demo = false;
        c.seed_user = None;
        c
    };

    let mut refusing = base();
    refusing.store_available_ram_bytes = Some(1024); // 1 KiB — the log is far larger
    let err = match boot_full(refusing).await {
        Ok(_) => panic!("boot must fail rather than OOM the machine"),
        Err(e) => e,
    };
    let msg = format!("{err:#}");
    assert!(
        msg.contains("will not fit in memory") && msg.contains("1024"),
        "the boot failure names both numbers: {msg}"
    );
    assert!(
        msg.contains("LB_STORE_OPEN_UNGUARDED=1"),
        "…and the way out: {msg}"
    );

    // Same store, same (impossible) RAM figure, guard disabled ⇒ it boots. Nothing was damaged by
    // the refusal, and the fallback-to-`mem://` path was never taken (the records are still here).
    let mut forced = base();
    forced.store_available_ram_bytes = Some(1024);
    forced.store_open_unguarded = true;
    let node = boot_full(forced).await.expect("the override boots");
    let v: Option<serde_json::Value> = lb_store::read(&node.node.store, "nube", "kv", "k5")
        .await
        .unwrap();
    assert!(
        v.is_some(),
        "the node opened the REAL store, never an empty mem:// one"
    );

    drop(node);
    std::fs::remove_dir_all(&parent).ok();
}
