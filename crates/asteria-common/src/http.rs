//! HTTPS ダウンロードと固定サイズ・Range・SHA-256 検証。

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

use crate::hash::hex_lower;

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(timeout).build()
}

/// URL 全体を取得し、HTTP 200 と最大バイト数上限を検証する。
pub fn fetch_bytes(
    url: &str,
    max_bytes: usize,
    user_agent: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    if !url.starts_with("https://") {
        bail!("公式 HTTPS URL が必要です: {url}");
    }
    let resp = agent(timeout)
        .get(url)
        .set("User-Agent", user_agent)
        .call()
        .with_context(|| format!("HTTP GET 失敗: {url}"))?;
    if resp.status() != 200 {
        bail!("全体応答ではありません (HTTP {}): {url}", resp.status());
    }
    let mut reader = resp.into_reader().take((max_bytes as u64) + 1);
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    if buf.len() > max_bytes {
        bail!("取得サイズが上限 ({max_bytes} bytes) を超えました: {url}");
    }
    Ok(buf)
}

/// HTTP Range リクエストで固定バイト区間を取得し、206 と Content-Range・実サイズを検証する。
pub fn fetch_range(
    url: &str,
    offset: u64,
    length: usize,
    expected_total_size: u64,
    user_agent: &str,
    timeout: Duration,
) -> Result<Vec<u8>> {
    if !url.starts_with("https://") {
        bail!("公式 HTTPS URL が必要です: {url}");
    }
    if length == 0 {
        bail!("取得長が 0 です");
    }
    let end = offset + (length as u64) - 1;
    let range_hdr = format!("bytes={offset}-{end}");
    let expected_cr = format!("bytes {offset}-{end}/{expected_total_size}");
    let resp = agent(timeout)
        .get(url)
        .set("Range", &range_hdr)
        .set("Accept-Encoding", "identity")
        .set("User-Agent", user_agent)
        .call()
        .with_context(|| format!("Range GET 失敗: {url}"))?;
    if resp.status() != 206 || resp.header("Content-Range") != Some(expected_cr.as_str()) {
        bail!(
            "固定した範囲だけの取得をサーバーが受理しませんでした (status={}, Content-Range={:?})",
            resp.status(),
            resp.header("Content-Range")
        );
    }
    let mut reader = resp.into_reader().take((length as u64) + 1);
    let mut buf = Vec::with_capacity(length);
    reader.read_to_end(&mut buf)?;
    if buf.len() != length {
        bail!(
            "取得した範囲の長さ ({}) が一致しません (期待 {length})",
            buf.len()
        );
    }
    Ok(buf)
}

/// URL から `.partial` へストリーミング保存し、サイズ・SHA-256 を検証してから原子置換する。
pub fn download_verified(
    url: &str,
    dest: &Path,
    expected_bytes: Option<u64>,
    expected_sha256: &str,
    user_agent: &str,
    timeout: Duration,
) -> Result<()> {
    if !url.starts_with("https://") {
        bail!("公式 HTTPS URL が必要です: {url}");
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .context("不正な出力ファイル名")?;
    let partial = dest.with_file_name(format!("{name}.partial"));
    if partial.exists() {
        let _ = fs::remove_file(&partial);
    }

    let resp = agent(timeout)
        .get(url)
        .set("User-Agent", user_agent)
        .call()
        .with_context(|| format!("ダウンロード失敗: {url}"))?;
    if resp.status() != 200 {
        bail!("全体応答ではありません (HTTP {}): {url}", resp.status());
    }

    let mut reader = resp.into_reader();
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    let mut total: u64 = 0;

    let result: Result<()> = (|| {
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total += n as u64;
            if let Some(limit) = expected_bytes {
                if total > limit {
                    bail!("固定した取得サイズ ({limit} bytes) を超えました");
                }
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n])?;
        }
        file.flush()?;
        Ok(())
    })();

    drop(file);
    if let Err(err) = result {
        let _ = fs::remove_file(&partial);
        return Err(err);
    }

    if let Some(limit) = expected_bytes {
        if total != limit {
            let _ = fs::remove_file(&partial);
            bail!("取得サイズ不一致: {total} != {limit}");
        }
    }
    let got_sha = hex_lower(&hasher.finalize());
    if got_sha != expected_sha256 {
        let _ = fs::remove_file(&partial);
        bail!("SHA-256 不一致: {got_sha} != {expected_sha256}");
    }
    fs::rename(&partial, dest)?;
    Ok(())
}
