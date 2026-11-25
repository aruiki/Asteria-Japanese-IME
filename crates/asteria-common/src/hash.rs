//! SHA-256 ダイジェスト計算と、既存成果物を上書きしない固定ファイル保存。

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

const BUF_SIZE: usize = 1024 * 1024;

/// バイト列の SHA-256 を小文字 16 進文字列で返す。
pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_lower(&hasher.finalize())
}

/// 任意のリーダーからストリーミングで SHA-256 を計算する。
pub fn sha256_reader<R: Read>(mut reader: R) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; BUF_SIZE];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_lower(&hasher.finalize()))
}

/// ファイルの SHA-256 をストリーミングで計算する。
pub fn sha256_file(path: &Path) -> Result<String> {
    let file =
        File::open(path).with_context(|| format!("ファイルを開けません: {}", path.display()))?;
    sha256_reader(BufReader::with_capacity(BUF_SIZE, file))
}

/// 既存の別成果物を上書きせずにファイルを保存する。
/// すでに存在する場合は内容が完全一致することを検証し、不一致ならエラーを返す。
pub fn save_fixed(path: &Path, data: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if path.exists() {
        let existing = fs::read(path)
            .with_context(|| format!("既存成果物の読み込みに失敗: {}", path.display()))?;
        if existing != data {
            bail!("既存の成果物と一致しません: {}", path.display());
        }
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("成果物の新規作成に失敗: {}", path.display()))?;
    file.write_all(data)?;
    file.flush()?;
    Ok(())
}

/// ダイジェストバイト列を小文字 16 進数へ変換する。
pub fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// ファイル書き込み時に読み取り専用属性を解除して上書き・削除できるようにする。
pub fn set_writable(path: &Path) -> io::Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(path, perms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn save_fixed_allows_identical_and_rejects_mismatch() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("sub/out.bin");
        save_fixed(&target, b"hello").unwrap();
        save_fixed(&target, b"hello").unwrap();
        assert!(save_fixed(&target, b"world").is_err());
        assert_eq!(sha256_file(&target).unwrap(), sha256_bytes(b"hello"));
    }
}
