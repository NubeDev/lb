//! diskwarden — a GNOME tray icon that watches your disk and reclaims build junk.
//!
//!   diskwarden tray            the icon (the actual product)
//!   diskwarden scan [root]…    one read-only scan, printed
//!   diskwarden scan-json …     the same, as JSON
//!
//! Settings live in `~/.config/diskwarden/policy.toml` and are editable from the
//! tray menu. Auto-clean is OFF until you tick it.

mod disk;
mod human;
mod paths;
mod scan_loop;
mod state;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use diskwarden_reclaim::{Policy, ScanCtx, ScanReport};

use scan_loop::ScanLoop;
use state::State;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("tray") {
        "tray" => run_tray(),
        "scan" => scan(&args[1..], false),
        "scan-json" => scan(&args[1..], true),
        other => {
            eprintln!(
                "diskwarden — watch the disk, reclaim build junk\n\
                 \n\
                 usage:\n  \
                   diskwarden tray             the GNOME tray icon (default)\n  \
                   diskwarden scan <root>...   one read-only scan, printed\n  \
                   diskwarden scan-json ...    the same, as JSON\n\
                 \n\
                 settings: ~/.config/diskwarden/policy.toml (or use the tray menu)"
            );
            if other != "help" && other != "--help" && other != "-h" {
                anyhow::bail!("unknown command {other:?}");
            }
            Ok(())
        }
    }
}

/// The tray: a ksni service on the D-Bus session bus + the scan loop behind it.
fn run_tray() -> anyhow::Result<()> {
    let policy = paths::load_policy()?;
    eprintln!(
        "diskwarden: watching {:?}, scanning every {}, auto-clean {}",
        policy.general.roots,
        human::interval(policy.general.scan_interval_secs),
        if policy.general.auto_clean {
            format!(
                "over {}",
                human::bytes(policy.general.auto_clean_over_bytes)
            )
        } else {
            "off".into()
        }
    );

    let state = Arc::new(Mutex::new(State {
        report: ScanReport {
            scanned_at_secs: 0,
            findings: vec![],
        },
        free: None,
        policy,
        busy: true,
        last_error: None,
    }));

    let (tx, rx) = scan_loop::channel();

    // `assume_sni_available(false)` (the default) means spawn() fails loudly here if
    // no StatusNotifierWatcher is on the bus — i.e. if the AppIndicator extension is
    // off. Better a clear error at startup than a process that runs forever with an
    // invisible icon.
    use ksni::blocking::TrayMethods;
    let handle = tray::Tray {
        state: Arc::clone(&state),
        tx: tx.clone(),
    }
    .spawn()
    .map_err(|e| {
        anyhow::anyhow!(
            "couldn't show a tray icon: {e}\n\
             GNOME needs an AppIndicator shell extension for StatusNotifierItem.\n\
             Check with: gnome-extensions list --enabled | grep -i appindicator"
        )
    })?;

    // The scan loop owns this thread from here; it returns on Quit.
    ScanLoop {
        state,
        rx,
        redraw: Box::new(move || {
            handle.update(|_| {});
        }),
    }
    .run();

    Ok(())
}

fn scan(roots: &[String], as_json: bool) -> anyhow::Result<()> {
    let policy = paths::load_policy()?;
    let roots: Vec<PathBuf> = if roots.is_empty() {
        policy.general.roots.clone()
    } else {
        roots.iter().map(PathBuf::from).collect()
    };

    let ctx = ScanCtx {
        roots,
        now_secs: diskwarden_reclaim::size::now_secs(),
    };
    let (report, errors) = ScanReport::scan(&ctx, &policy);

    for e in &errors {
        eprintln!("warning: {e:#}");
    }

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    if report.findings.is_empty() {
        println!("nothing found under {:?}", ctx.roots);
        return Ok(());
    }

    println!(
        "{:>10}  {:>6}  {:<22}  {}",
        "SIZE", "AGE", "PROJECT", "PATH"
    );
    for f in &report.findings {
        println!(
            "{:>10}  {:>6}  {:<22}  {}{}",
            human::bytes(f.candidate.bytes),
            human::age(f.candidate.age_days_at(ctx.now_secs)),
            f.candidate.label,
            f.candidate.path.display(),
            if f.auto_cleanable() { "  ⟳ auto" } else { "" },
        );
    }

    println!(
        "\nfound {} across {} dirs\n{} cleanable now, {} would auto-clean",
        human::bytes(report.total_bytes()),
        report.findings.len(),
        human::bytes(report.reclaimable_bytes()),
        human::bytes(report.auto_clean_bytes()),
    );
    let _ = policy_hint(&report, &policy);
    Ok(())
}

/// Explain a 0-reclaimable result rather than leaving it looking broken.
fn policy_hint(report: &ScanReport, policy: &Policy) -> Option<()> {
    if report.reclaimable_bytes() > 0 {
        return None;
    }
    let newest = report.findings.iter().map(|f| f.gate.clone()).next()?;
    if let diskwarden_reclaim::Gate::TooRecent { min_age_days, .. } = newest {
        println!(
            "(everything found was used in the last {min_age_days} days, so it's protected. \
             adjust min_age_days in {})",
            paths::policy_file().ok()?.display()
        );
    }
    let _ = policy;
    Some(())
}
