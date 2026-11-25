//! モデル品質階層・追加モデルカタログ・実ファイル SHA-256 識別 (`asteria_model_catalog`, `asteria_model_identity`)。

use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LmQuality {
    Eco,
    Low,
    Standard,
    High,
    Unreal,
}

impl LmQuality {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "eco" => Self::Eco,
            "low" => Self::Low,
            "high" => Self::High,
            "unreal" => Self::Unreal,
            _ => Self::Standard,
        }
    }

    pub fn params(self) -> QualityParams {
        match self {
            Self::Eco => QualityParams {
                beams: 2,
                llm_top: 0,
                k: 4,
            },
            Self::Low => QualityParams {
                beams: 2,
                llm_top: 0,
                k: 4,
            },
            Self::Standard => QualityParams {
                beams: 6,
                llm_top: 4,
                k: 8,
            },
            Self::High => QualityParams {
                beams: 8,
                llm_top: 32,
                k: 16,
            },
            Self::Unreal => QualityParams {
                beams: 8,
                llm_top: 32,
                k: 16,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityParams {
    pub beams: usize,
    pub llm_top: usize,
    pub k: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExtraModel {
    pub id: &'static str,
    pub tier: LmQuality,
    pub title: &'static str,
    pub file: &'static str,
    pub url: &'static str,
    pub size: u64,
    pub sha256: &'static str,
    pub vram_gb: f64,
    pub license: &'static str,
    pub source: &'static str,
}

/// High・Unreal 用追加判定モデル (`asteria_model_catalog.cc` 準拠)。
pub const EXTRA_MODELS: &[ExtraModel] = &[
    ExtraModel {
        id: "swallow-8b-judge-high",
        tier: LmQuality::High,
        title: "Llama 3.1 Swallow 8B 軽量版(判定役)",
        file: "llama-3.1-swallow-8b-v0.5-q3_k_m.gguf",
        url: "https://huggingface.co/aruiki/Llama-3.1-Swallow-8B-v0.5-GGUF/resolve/a96b0217d2d148f32e0d5dd36e47523439b9dbfc/llama-3.1-swallow-8b-v0.5-q3_k_m.gguf",
        size: 4_018_918_816,
        sha256: "64e16e946d8feef02c86cc990581b133129f33df961d4ddeafe152b3b8628c2c",
        vram_gb: 6.0,
        license: "Llama 3.1 Community License・Gemma Terms of Use",
        source: "https://huggingface.co/tokyotech-llm/Llama-3.1-Swallow-8B-v0.5",
    },
    ExtraModel {
        id: "swallow-8b-judge",
        tier: LmQuality::Unreal,
        title: "Llama 3.1 Swallow 8B(判定役)",
        file: "llama-3.1-swallow-8b-v0.5-q4_k_m.gguf",
        url: "https://huggingface.co/aruiki/Llama-3.1-Swallow-8B-v0.5-GGUF/resolve/a6258e14d4cce4e70881794c2f2f000da55367f5/llama-3.1-swallow-8b-v0.5-q4_k_m.gguf",
        size: 4_920_735_136,
        sha256: "292a4e2696251015daddc0393a63f33fc4b1cebae2d62eb35670561e17de4277",
        vram_gb: 8.0,
        license: "Llama 3.1 Community License・Gemma Terms of Use",
        source: "https://huggingface.co/tokyotech-llm/Llama-3.1-Swallow-8B-v0.5",
    },
];

pub fn extra_model_for(tier: LmQuality) -> Option<&'static ExtraModel> {
    EXTRA_MODELS.iter().find(|m| m.tier == tier)
}

pub fn extra_model_dir(profile_dir: &Path) -> PathBuf {
    profile_dir.join("models")
}

pub fn extra_model_path(profile_dir: &Path, model: &ExtraModel) -> PathBuf {
    extra_model_dir(profile_dir).join(model.file)
}

pub fn extra_model_installed(profile_dir: &Path, model: &ExtraModel) -> bool {
    fs::metadata(extra_model_path(profile_dir, model))
        .is_ok_and(|m| m.is_file() && m.len() == model.size)
}

/// モデルファイルを読み取り共有状態で開きつつ SHA-256 指紋を確定する (`AsteriaModelIdentity`)。
#[derive(Debug)]
pub struct ModelIdentity {
    pub path: PathBuf,
    pub sha256: String,
    _hold: File,
}

impl ModelIdentity {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let mut opts = fs::OpenOptions::new();
        opts.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // FILE_SHARE_READ のみ許可し、書き込み・リネームによる差し替えを防ぐ。
            opts.share_mode(1);
        }
        let file = opts.open(path)?;
        let mut reader = BufReader::new(&file);
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let sha256 = hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok(Self {
            path: path.to_path_buf(),
            sha256,
            _hold: file,
        })
    }
}

/// 推論統計カウンタ (`[stats]` 互換)。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferStats {
    pub zenz_work_ms: u64,
    pub zenz_decode_ms: u64,
    pub zenz_calls: u64,
    pub zenz_tokens: u64,
    pub llm_work_ms: u64,
    pub llm_decode_ms: u64,
    pub llm_calls: u64,
    pub llm_tokens: u64,
}

impl InferStats {
    pub fn total_work_ms(&self) -> u64 {
        self.zenz_work_ms + self.llm_work_ms
    }

    pub fn total_calls(&self) -> u64 {
        self.zenz_calls + self.llm_calls
    }
}

/// 同梱モデルファイル配置候補。
#[derive(Debug, Clone, Default)]
pub struct ModelBundlePaths {
    pub zenz_small: Option<PathBuf>,
    pub genmai: Option<PathBuf>,
    pub tinyswallow: Option<PathBuf>,
    pub native_generator: Option<PathBuf>,
    pub judge: Option<PathBuf>,
}

impl ModelBundlePaths {
    pub fn discover(model_dir: &Path, profile_dir: Option<&Path>, tier: LmQuality) -> Self {
        Self::discover_in(&[model_dir], profile_dir, tier)
    }

    pub fn discover_in(dirs: &[&Path], profile_dir: Option<&Path>, tier: LmQuality) -> Self {
        let pick = |name: &str| dirs.iter().map(|d| d.join(name)).find(|p| p.is_file());
        let judge = profile_dir.and_then(|prof| {
            extra_model_for(tier).and_then(|m| {
                let p = extra_model_path(prof, m);
                extra_model_installed(prof, m).then_some(p)
            })
        });
        Self {
            zenz_small: pick("zenz-v2.5-small-q8_0.gguf")
                .or_else(|| pick("zenz-v2.5-small-f16.gguf")),
            genmai: pick("genmai-310M-q8_0.gguf"),
            tinyswallow: pick("tinyswallow-1.5b-q5_k_m.gguf"),
            native_generator: pick("zenz-v2.5-native-generator-20261017-q8_0.gguf"),
            judge,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn catalog_tiers_and_identity_work() {
        assert_eq!(LmQuality::parse("unreal"), LmQuality::Unreal);
        assert!(extra_model_for(LmQuality::Standard).is_none());
        let high = extra_model_for(LmQuality::High).unwrap();
        assert_eq!(high.id, "swallow-8b-judge-high");

        let dir = TempDir::new().unwrap();
        let file = dir.path().join("m.gguf");
        fs::write(&file, b"abc").unwrap();
        let id = ModelIdentity::open(&file).unwrap();
        assert_eq!(
            id.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(!extra_model_installed(dir.path(), high));
    }
}
