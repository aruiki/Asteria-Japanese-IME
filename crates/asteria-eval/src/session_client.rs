//! `session_handler_main` プロセスとの同期コマンド送受信クライアント（`first_display_compare.py` `Session` 移植）。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct SessionEvent {
    pub command: String,
    pub response_ms: f64,
    pub first_candidate: Option<String>,
    pub preedit_key: String,
    pub preedit_value: String,
    pub output: String,
}

#[derive(Debug, Clone)]
pub struct ConversionEvent {
    pub final_event: SessionEvent,
    pub initial_event: SessionEvent,
    pub callback_events: Vec<SessionEvent>,
    pub operation_ms: f64,
}

pub struct Session {
    proc: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Result<Option<String>>>,
    reader_handle: Option<JoinHandle<()>>,
    pub pid: u32,
}

impl Session {
    pub fn spawn(
        exe: &Path,
        env: &[(String, String)],
        profile: &Path,
        stderr_path: &Path,
        preserve_profile: bool,
    ) -> Result<Self> {
        let cwd = exe.parent().context("exe parent")?.join(format!(
            "{}.runfiles/_main",
            exe.file_name().and_then(|n| n.to_str()).unwrap_or("")
        ));
        let err_file = std::fs::File::create(stderr_path)?;
        let mut cmd = Command::new(exe);
        cmd.arg("--dictionary=oss")
            .arg(format!("--profile={}", profile.join("session").display()))
            .arg("--use_history_rewriter=true");
        if preserve_profile {
            cmd.arg("--preserve_profile=true");
        }
        cmd.current_dir(&cwd)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(err_file));

        let mut proc = cmd
            .spawn()
            .with_context(|| format!("Session 起動失敗: {}", exe.display()))?;
        let pid = proc.id();
        let stdin = proc.stdin.take();
        let stdout = proc.stdout.take().context("stdout missing")?;
        let (tx, rx) = mpsc::channel();
        let handle = spawn_reader(stdout, tx);
        Ok(Self {
            proc,
            stdin,
            rx,
            reader_handle: Some(handle),
            pid,
        })
    }

    pub fn try_poll(&mut self) -> Result<Option<i32>> {
        Ok(self.proc.try_wait()?.and_then(|s| s.code()))
    }

    pub fn send(&mut self, command: &str) -> Result<SessionEvent> {
        let started = Instant::now();
        {
            let stdin = self.stdin.as_mut().context("session stdin closed")?;
            stdin.write_all(format!("{command}\nSHOW_OUTPUT\n").as_bytes())?;
            stdin.flush()?;
        }
        let mut output = Vec::new();
        let mut tail = false;
        let deadline = started + Duration::from_secs(60);

        loop {
            let now = Instant::now();
            if now >= deadline {
                let tail_str = output.join("\n");
                bail!(
                    "セッション応答が60秒を超過: {}",
                    tail_str
                        .chars()
                        .rev()
                        .take(2000)
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect::<String>()
                );
            }
            let rem = deadline - now;
            let msg = match self.rx.recv_timeout(rem) {
                Ok(m) => m,
                Err(_) => {
                    bail!("セッション応答が60秒を超過: {}", output.join("\n"));
                }
            };
            let line_opt = msg?;
            let Some(line) = line_opt else {
                bail!("セッションが途中で終了: {}", output.join("\n"));
            };
            let is_end = tail && line.trim() == "}";
            let starts_tail = line.starts_with("removed_candidate_words_for_debug {");
            output.push(line);
            if is_end {
                break;
            }
            tail |= starts_tail;
        }

        let text = output.join("\n");
        if text.lines().any(|l| l.starts_with("ERROR:")) {
            bail!("セッションがコマンドを拒否: {text}");
        }
        let parsed = crate::textproto::parse_output(&text).ok();
        let first_candidate = parsed
            .as_ref()
            .and_then(|p| p.candidate_window.first().map(|c| c.value.clone()));
        Ok(SessionEvent {
            command: command.to_owned(),
            response_ms: started.elapsed().as_secs_f64() * 1000.0,
            first_candidate,
            preedit_key: parsed.as_ref().map(|p| p.key.clone()).unwrap_or_default(),
            preedit_value: parsed.as_ref().map(|p| p.value.clone()).unwrap_or_default(),
            output: text,
        })
    }

    pub fn send_conversion(&mut self, command: &str, timeout_secs: f64) -> Result<ConversionEvent> {
        if !(0.0..=120.0).contains(&timeout_secs) || timeout_secs <= 0.0 {
            bail!("conversion timeout must be between 0 and 120 seconds");
        }
        let started = Instant::now();
        let deadline = started + Duration::from_secs_f64(timeout_secs);
        let initial = self.send(command)?;
        let mut event = initial.clone();
        let mut callbacks = Vec::new();
        let delay_re = Regex::new(r"delay_millisec:\s*(\d+)")?;
        let has_result = |txt: &str| txt.lines().any(|l| l.starts_with("result {"));
        let is_pending = |txt: &str| -> bool {
            if let Ok(p) = crate::textproto::parse_output(txt) {
                p.callback_has_refresh_suggestion
            } else {
                txt.contains("ASTERIA_REFRESH_SUGGESTION")
            }
        };

        while is_pending(&event.output) && !has_result(&event.output) {
            let now = Instant::now();
            if now >= deadline {
                bail!("AI conversion did not finish: {command}");
            }
            let remaining = (deadline - now).as_secs_f64();
            let delay_s = delay_re
                .captures(&event.output)
                .and_then(|c| c[1].parse::<f64>().ok())
                .map(|ms| (ms / 1000.0).clamp(0.01, 1.0))
                .unwrap_or(0.1);
            thread::sleep(Duration::from_secs_f64(remaining.min(delay_s)));
            event = self.send("ASTERIA_REFRESH")?;
            if event.output.lines().any(|l| l.trim() == "consumed: false") {
                bail!("AI conversion was canceled before delivery: {command}");
            }
            callbacks.push(event.clone());
        }

        Ok(ConversionEvent {
            final_event: event,
            initial_event: initial,
            callback_events: callbacks,
            operation_ms: started.elapsed().as_secs_f64() * 1000.0,
        })
    }

    pub fn close(mut self, failed: bool) -> Result<()> {
        if failed {
            let _ = self.proc.kill();
        } else {
            self.stdin.take();
        }
        let status = self.proc.wait()?;
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }
        if !failed && !status.success() {
            bail!("セッション終了コード: {:?}", status.code());
        }
        Ok(())
    }
}

pub fn spawn_reader<R: std::io::Read + Send + 'static>(
    stdout: R,
    tx: mpsc::Sender<Result<Option<String>>>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) => {
                    let _ = tx.send(Ok(None));
                    break;
                }
                Ok(_) => match std::str::from_utf8(&buf) {
                    Ok(s) => {
                        let trimmed = s.trim_end_matches(['\r', '\n']).to_owned();
                        if tx.send(Ok(Some(trimmed))).is_err() {
                            break;
                        }
                    }
                    Err(_) => {
                        let preview = String::from_utf8_lossy(&buf[..buf.len().min(200)]);
                        let _ = tx.send(Err(anyhow::anyhow!("non-UTF8 session stdout: {preview}")));
                        break;
                    }
                },
                Err(e) => {
                    let _ = tx.send(Err(e.into()));
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_utf8_wakes_reader_instead_of_hanging() {
        let (tx, rx) = mpsc::channel();
        let data: &[u8] = b"ERROR: bad path \x83\n";
        let h = spawn_reader(std::io::Cursor::new(data), tx);
        let msg = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(msg.is_err());
        assert!(msg.unwrap_err().to_string().contains("non-UTF8"));
        h.join().unwrap();
    }
}
