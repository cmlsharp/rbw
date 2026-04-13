use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    io::Write,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{self, Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

fn run_command(command: &[&str]) -> Result<String> {
    let (program, args) = command.split_first().context("empty command")?;
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {}", command.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!(
            "{} failed: {}",
            command.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub const DEFAULT_CLIPBOARD_TIMEOUT_SECONDS: u64 = 45;

fn clipboard_state_dir() -> PathBuf {
    let base = env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir);
    let dir = base.join("bitwarden-tui");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    dir
}

fn clipboard_token_path() -> PathBuf {
    clipboard_state_dir().join("clipboard-token")
}

fn clipboard_digest(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn schedule_clipboard_clear(text: &str, timeout_seconds: u64) -> Result<()> {
    let token = format!(
        "{}-{}",
        process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    );
    fs::write(clipboard_token_path(), &token)?;
    let exe = env::current_exe()?;
    let mut command = Command::new("setsid");
    command
        .arg("-f")
        .arg(exe)
        .arg("--clear-clipboard-helper")
        .arg("--token")
        .arg(&token)
        .arg("--clipboard-digest")
        .arg(clipboard_digest(text))
        .arg("--timeout")
        .arg(timeout_seconds.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command.spawn()?;
    Ok(())
}

/// Detached helper path for clearing the clipboard after the configured timeout.
pub fn run_clipboard_helper(token: &str, expected_digest: &str, timeout: u64) -> Result<i32> {
    std::thread::sleep(Duration::from_secs(timeout));
    let token_path = clipboard_token_path();
    let current_token = fs::read_to_string(&token_path).unwrap_or_default();
    if current_token.trim() != token {
        return Ok(0);
    }
    let current = run_command(&["wl-paste", "--no-newline"])
        .ok()
        .unwrap_or_default();
    if clipboard_digest(&current) != expected_digest {
        return Ok(0);
    }
    let _ = Command::new("wl-copy").arg("--clear").status();
    let latest = fs::read_to_string(&token_path).unwrap_or_default();
    if latest.trim() == token {
        let _ = fs::remove_file(token_path);
    }
    Ok(0)
}

/// Copies text to the live clipboard and schedules expiry cleanup.
pub fn copy_text(text: &str, timeout_seconds: u64) -> Result<()> {
    let mut child = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn wl-copy")?;
    {
        let mut stdin = child.stdin.take().context("missing wl-copy stdin")?;
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("wl-copy failed");
    }
    schedule_clipboard_clear(text, timeout_seconds)?;
    Ok(())
}
