use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

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
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                log(line.unwrap_or_else(|e| format!("stdout read error: {e}")));
            }
        }
        if let Some(stderr) = child.stderr.take() {
            for line in BufReader::new(stderr).lines() {
                log(line.unwrap_or_else(|e| format!("stderr read error: {e}")));
            }
        }
        let status = child
            .wait()
            .with_context(|| format!("wait for {program}"))?;
        if !status.success() {
            bail!("{program} exited with {status}");
        }
        Ok(())
    }
}
