use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;

use anyhow::{bail, Context, Result};

pub trait Toolchain {
    fn run(
        &self,
        cwd: &Path,
        program: &str,
        args: &[&str],
        log: &mut dyn FnMut(String),
    ) -> Result<()>;

    fn ready(&self, program: &str) -> bool {
        Command::new(program)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn wasm_target_ready(&self) -> bool {
        Command::new("rustup")
            .args(["target", "list", "--installed"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("wasm32-wasip2"))
            .unwrap_or(false)
    }
}

pub struct ProcessToolchain;

impl Toolchain for ProcessToolchain {
    fn run(
        &self,
        cwd: &Path,
        program: &str,
        args: &[&str],
        log: &mut dyn FnMut(String),
    ) -> Result<()> {
        // All sanctioned process execution for devkit builds goes through this function. Callers get
        // log lines, never handles to arbitrary child processes.
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawn {program} in {}", cwd.display()))?;
        // Interleave stdout/stderr as they arrive instead of draining stdout fully before
        // touching stderr — a failing `cargo` writes its error to stderr, and draining stdout
        // first buried that line at the end of the log (looked like the build "stopped mid
        // download" in the Studio UI; see devkit-container-build-scope.md "Log ordering").
        let (tx, rx) = mpsc::channel::<String>();
        let stdout = child.stdout.take().expect("piped stdout");
        let stderr = child.stderr.take().expect("piped stderr");
        let tx_out = tx.clone();
        let out_thread = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let _ = tx_out.send(line.unwrap_or_else(|e| format!("stdout read error: {e}")));
            }
        });
        let err_thread = thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let _ = tx.send(line.unwrap_or_else(|e| format!("stderr read error: {e}")));
            }
        });
        for line in rx {
            log(line);
        }
        let _ = out_thread.join();
        let _ = err_thread.join();
        let status = child
            .wait()
            .with_context(|| format!("wait for {program}"))?;
        if !status.success() {
            bail!("{program} exited with {status}");
        }
        Ok(())
    }
}
