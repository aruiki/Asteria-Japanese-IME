//! サーバーの設定永続化・AIステータスファイル・データディレクトリ解決。

use std::fs;
use std::path::{Path, PathBuf};

use asteria_proto::ipc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub live_conversion: bool,
    pub homophone_hints: bool,
    pub ai_enabled: bool,
    pub ai_quality: String,
    pub gpu_enabled: bool,
    pub allow_integrated_gpu: bool,
    pub typo_correction: bool,
    pub suggest_window: bool,
    pub record_recent_conversions: bool,
    pub romaji_english: bool,
    pub prediction_enabled: bool,
    pub nuance_enabled: bool,
    pub standard_notation: bool,
    pub incognito: bool,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            live_conversion: false,
            homophone_hints: false,
            ai_enabled: true,
            ai_quality: "standard".to_owned(),
            gpu_enabled: true,
            allow_integrated_gpu: false,
            typo_correction: true,
            suggest_window: false,
            record_recent_conversions: false,
            romaji_english: true,
            prediction_enabled: true,
            nuance_enabled: true,
            standard_notation: true,
            incognito: false,
        }
    }
}

impl ServerSettings {
    pub fn load_from(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = tmp_path(path);
        fs::write(&tmp, body)?;
        fs::rename(&tmp, path)
    }

    pub fn to_toml(&self) -> String {
        format!(
            "live_conversion = {}\nhomophone_hints = {}\nai_enabled = {}\nai_quality = \"{}\"\ngpu_enabled = {}\nallow_integrated_gpu = {}\ntypo_correction = {}\nsuggest_window = {}\nrecord_recent_conversions = {}\nromaji_english = {}\nprediction_enabled = {}\nnuance_enabled = {}\nstandard_notation = {}\nincognito = {}\n",
            self.live_conversion,
            self.homophone_hints,
            self.ai_enabled,
            self.ai_quality.replace('"', ""),
            self.gpu_enabled,
            self.allow_integrated_gpu,
            self.typo_correction,
            self.suggest_window,
            self.record_recent_conversions,
            self.romaji_english,
            self.prediction_enabled,
            self.nuance_enabled,
            self.standard_notation,
            self.incognito,
        )
    }

    pub fn to_proto(&self) -> ipc::Config {
        ipc::Config {
            toml: self.to_toml(),
        }
    }

    pub fn apply_toml_or_json(&mut self, text: &str) {
        let trimmed = text.trim();
        if trimmed.starts_with('{') {
            if let Ok(parsed) = serde_json::from_str::<ServerSettings>(trimmed) {
                *self = parsed;
                return;
            }
        }
        for raw_line in text.lines() {
            let line = raw_line.split('#').next().unwrap_or("").trim();
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            let key = k.trim();
            let val = v.trim().trim_matches('"');
            let b = val.eq_ignore_ascii_case("true") || val == "1";
            match key {
                "live_conversion" => self.live_conversion = b,
                "homophone_hints" => self.homophone_hints = b,
                "ai_enabled" => self.ai_enabled = b,
                "ai_quality" => self.ai_quality = val.to_owned(),
                "gpu_enabled" => self.gpu_enabled = b,
                "allow_integrated_gpu" => self.allow_integrated_gpu = b,
                "typo_correction" => self.typo_correction = b,
                "suggest_window" => self.suggest_window = b,
                "record_recent_conversions" => self.record_recent_conversions = b,
                "romaji_english" => self.romaji_english = b,
                "prediction_enabled" => self.prediction_enabled = b,
                "nuance_enabled" => self.nuance_enabled = b,
                "standard_notation" => self.standard_notation = b,
                "incognito" => self.incognito = b,
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiRuntimeStatus {
    pub state: String,
    pub model: String,
    pub device: String,
    pub gpu: String,
    pub vram_gb: f64,
    pub infer_calls: u64,
    pub infer_ms: f64,
}

impl Default for AiRuntimeStatus {
    fn default() -> Self {
        Self {
            state: "disabled".to_owned(),
            model: String::new(),
            device: "CPU".to_owned(),
            gpu: String::new(),
            vram_gb: 0.0,
            infer_calls: 0,
            infer_ms: 0.0,
        }
    }
}

impl AiRuntimeStatus {
    pub fn format_raw(&self) -> String {
        format!(
            "state={}\nmodel={}\ndevice={}\ngpu={}\nvram_gb={:.2}\ninfer_calls={}\ninfer_ms={:.2}\n",
            self.state,
            self.model,
            self.device,
            self.gpu,
            self.vram_gb,
            self.infer_calls,
            self.infer_ms
        )
    }

    pub fn to_proto(&self) -> ipc::StatusResponse {
        ipc::StatusResponse {
            state: self.state.clone(),
            model: self.model.clone(),
            device: self.device.clone(),
            gpu: self.gpu.clone(),
            vram_gb: self.vram_gb,
            infer_calls: self.infer_calls,
            infer_ms: self.infer_ms,
            raw: self.format_raw(),
        }
    }
}

/// `asteria_ai_status.txt` を利用者のプロファイルフォルダに書き出す。
pub fn write_ai_status_file(dir: &Path, status: &AiRuntimeStatus) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join("asteria_ai_status.txt");
    let tmp = tmp_path(&path);
    fs::write(&tmp, status.format_raw())?;
    fs::rename(&tmp, &path)
}

/// Asteria の標準ユーザーデータ保存先を返す (Windows: `%LOCALAPPDATA%\Asteria`)。
pub fn default_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("Asteria"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .map(|p| p.join("asteria"))
    }
}

fn tmp_path(p: &Path) -> PathBuf {
    let mut os = p.as_os_str().to_os_string();
    os.push(".tmp");
    PathBuf::from(os)
}
