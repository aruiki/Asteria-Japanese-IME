//! 設定画面 (`web/index.html`) ↔ 変換サーバー間の JSON ブリッジ (`asteria_settings_bridge.cc` 移植)。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use asteria_learn::{RecentConversions, UserDictionary};
use asteria_lm::catalog::{
    extra_model_dir, extra_model_installed, extra_model_path, LmQuality, EXTRA_MODELS,
};
use asteria_lm::llama_dll::detect_runtime_gpu;
use asteria_proto::ipc::{self, request, response};
use serde_json::{json, Value};

use crate::downloader::{DownloadSink, ModelDownloader};

pub const INDEX_HTML: &str = include_str!("../web/index.html");
pub const SETTINGS_CSS: &str = include_str!("../web/settings.css");
pub const SETTINGS_JS: &str = include_str!("../web/settings.js");

/// `index.html` の外部 CSS / JS 参照をインライン展開した単一 HTML を返す (`NavigateToString` 用)。
pub fn bundled_html() -> String {
    let html = INDEX_HTML.replace(
        r#"<link rel="stylesheet" href="settings.css">"#,
        &format!("<style>{SETTINGS_CSS}</style>"),
    );
    html.replace(
        r#"<script src="settings.js" defer></script>"#,
        &format!("<script>{SETTINGS_JS}</script>"),
    )
}

pub trait BridgeHost: Send + Sync + 'static {
    fn ipc_call(&self, body: request::Body) -> Result<response::Body, String>;
    fn save_text_dialog(&self, default_name: &str, text: &str) -> bool;
    fn open_support_page(&self);
    fn push_event(&self, event: Value);
    fn gpu_vram_gb(&self) -> f64;
    fn data_dir(&self) -> PathBuf;
}

struct SinkAdapter {
    host: Arc<dyn BridgeHost>,
    progress: Arc<Mutex<HashMap<String, (u64, u64)>>>,
}

impl DownloadSink for SinkAdapter {
    fn on_progress(&self, id: &str, received: u64, total: u64) {
        if let Ok(mut map) = self.progress.lock() {
            map.insert(id.to_owned(), (received, total));
        }
        self.host.push_event(json!({
            "event": "download",
            "id": id,
            "received": received,
            "total": total,
        }));
    }

    fn on_done(&self, id: &str, outcome: crate::downloader::DownloadOutcome, message: &str) {
        let state = match outcome {
            crate::downloader::DownloadOutcome::Done => "done",
            crate::downloader::DownloadOutcome::Cancelled => "cancelled",
            crate::downloader::DownloadOutcome::Failed => "error",
        };
        self.host.push_event(json!({
            "event": "downloadDone",
            "id": id,
            "state": state,
            "message": message,
        }));
    }
}

pub struct SettingsBridge {
    host: Arc<dyn BridgeHost>,
    downloads: Mutex<HashMap<String, ModelDownloader>>,
    progress: Arc<Mutex<HashMap<String, (u64, u64)>>>,
    cached_toml: Mutex<String>,
}

impl SettingsBridge {
    pub fn new(host: Arc<dyn BridgeHost>) -> Self {
        Self {
            host,
            downloads: Mutex::new(HashMap::new()),
            progress: Arc::new(Mutex::new(HashMap::new())),
            cached_toml: Mutex::new(String::new()),
        }
    }

    pub fn handle_json(&self, raw: &str) -> String {
        let Ok(req) = serde_json::from_str::<Value>(raw) else {
            return json!({"ok": false, "message": "JSON 解析に失敗しました。"}).to_string();
        };
        let id = req.get("id").cloned();
        let result = self.dispatch(&req);
        if let Some(req_id) = id {
            json!({ "id": req_id, "result": result }).to_string()
        } else {
            result.to_string()
        }
    }

    pub fn dispatch(&self, req: &Value) -> Value {
        let op = req.get("op").and_then(Value::as_str).unwrap_or("");
        match op {
            "load" => self.load(),
            "save" => self.save(req),
            "defaults" => json!({ "ok": true, "config": default_config_json() }),
            "status" => self.status(),
            "clear" => self.clear(req.get("what").and_then(Value::as_str).unwrap_or("")),
            "open" => self.open(req.get("target").and_then(Value::as_str).unwrap_or("")),
            "diagnostics" => self.diagnostics(),
            "saveDiagnostics" => {
                let text = self
                    .diagnostics()
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned();
                let saved = self.host.save_text_dialog("asteria-diagnostics.txt", &text);
                json!({ "ok": true, "saved": saved })
            }
            "models" => self.models(),
            "recentConversions" => self.recent_conversions(),
            "exportRecentConversions" => self.export_recent_conversions(),
            "download" => self.download(req.get("model").and_then(Value::as_str).unwrap_or("")),
            "cancelDownload" => {
                self.cancel_download(req.get("model").and_then(Value::as_str).unwrap_or(""))
            }
            "deleteModel" => {
                self.delete_model(req.get("model").and_then(Value::as_str).unwrap_or(""))
            }
            "userDictList" => self.user_dict_list(),
            "userDictAdd" => self.user_dict_add(req),
            "userDictImport" => {
                self.user_dict_import(req.get("tsv").and_then(Value::as_str).unwrap_or(""))
            }
            "userDictExport" => self.user_dict_export(),
            other => fail(&format!("知らない操作です: {other}")),
        }
    }

    fn load(&self) -> Value {
        let toml_text = match self
            .host
            .ipc_call(request::Body::GetConfig(ipc::GetConfig {}))
        {
            Ok(response::Body::Config(c)) => c.toml,
            _ => {
                return fail(
                    "今の設定を読み込めませんでした。変換エンジンを起動し直してから、もう一度開いてください。",
                );
            }
        };
        if let Ok(mut g) = self.cached_toml.lock() {
            *g = toml_text.clone();
        }
        let cfg = parse_toml_to_config_json(&toml_text);
        let env = self.environment_json();
        json!({ "ok": true, "config": cfg, "env": env })
    }

    fn save(&self, req: &Value) -> Value {
        let cfg = req
            .get("config")
            .cloned()
            .unwrap_or_else(default_config_json);
        let toml_text = config_json_to_toml(&cfg);
        match self.host.ipc_call(request::Body::SetConfig(ipc::SetConfig {
            toml: toml_text.clone(),
        })) {
            Ok(response::Body::Ack(_)) => {
                let _ = self.host.ipc_call(request::Body::Reload(ipc::Reload {}));
                if let Ok(mut g) = self.cached_toml.lock() {
                    *g = toml_text;
                }
                json!({
                    "ok": true,
                    "config": cfg,
                    "env": self.environment_json(),
                    "notices": [],
                })
            }
            _ => fail("設定を保存できませんでした。もう一度お試しください。"),
        }
    }

    fn environment_json(&self) -> Value {
        let gpu = detect_runtime_gpu(&self.host.data_dir());
        let gpu_label = if gpu.name.is_empty() {
            "CPU".to_owned()
        } else {
            format!(
                "{} ({:.1} GiB)",
                gpu.name,
                gpu.total_vram_bytes as f64 / (1u64 << 30) as f64
            )
        };
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "gpu": gpu_label,
            "imeHotKeyDisabled": false,
            "supportReminder": true,
            "keymapCustomized": false,
            "romanTableCustomized": false,
            "highContrast": false,
        })
    }

    fn status(&self) -> Value {
        match self
            .host
            .ipc_call(request::Body::GetStatus(ipc::GetStatus {}))
        {
            Ok(response::Body::Status(st)) => json!({
                "ok": true,
                "state": st.state,
                "ai": {
                    "model": st.model,
                    "device": st.device,
                    "gpu": st.gpu,
                    "vramGb": st.vram_gb,
                    "inferCalls": st.infer_calls,
                    "inferMs": st.infer_ms,
                },
                "models": self.models().get("models").cloned().unwrap_or(json!([])),
                "usage": [],
            }),
            _ => json!({
                "ok": true,
                "state": "offline",
                "ai": {},
                "models": self.models().get("models").cloned().unwrap_or(json!([])),
                "usage": [],
            }),
        }
    }

    fn clear(&self, what: &str) -> Value {
        match what {
            "recentConversions" => {
                let _ = self.host.ipc_call(request::Body::ClearRecentConversions(
                    ipc::ClearRecentConversions {},
                ));
                json!({ "ok": true })
            }
            "history" | "prediction" | "unusedPrediction" => {
                match self
                    .host
                    .ipc_call(request::Body::SendCommand(ipc::SendCommand {
                        session_id: 0,
                        kind: ipc::CommandKind::ClearUserHistory.into(),
                        argument: 0,
                    })) {
                    Ok(_) => json!({ "ok": true }),
                    Err(e) => fail(&format!("消せませんでした: {e}")),
                }
            }
            other => fail(&format!("知らない消し方です: {other}")),
        }
    }

    fn open(&self, target: &str) -> Value {
        if target == "support" {
            self.host.open_support_page();
            return json!({ "ok": true });
        }
        json!({ "ok": true })
    }

    fn diagnostics(&self) -> Value {
        let status_file = fs::read_to_string(self.host.data_dir().join("asteria_ai_status.txt"))
            .unwrap_or_default();
        let text = format!(
            "Asteria 日本語入力 診断情報\nバージョン: {}\nGPU VRAM: {:.1} GiB\n\n[AI 状態]\n{}",
            env!("CARGO_PKG_VERSION"),
            self.host.gpu_vram_gb(),
            status_file
        );
        json!({ "ok": true, "text": text })
    }

    fn models(&self) -> Value {
        let vram = self.host.gpu_vram_gb();
        let data_dir = self.host.data_dir();
        let dl = self.downloads.lock().ok();
        let prog = self.progress.lock().ok();
        let mut list = Vec::new();
        for m in EXTRA_MODELS {
            let tier = match m.tier {
                LmQuality::Unreal => "unreal",
                LmQuality::High => "high",
                _ => "standard",
            };
            let running = dl
                .as_ref()
                .and_then(|map| map.get(m.id))
                .is_some_and(ModelDownloader::is_running);
            let mut item = json!({
                "id": m.id,
                "tier": tier,
                "title": m.title,
                "size": m.size,
                "vramGb": m.vram_gb,
                "license": m.license,
                "source": m.source,
                "installed": extra_model_installed(&data_dir, m),
                "downloading": running,
                "fits": vram + 0.25 >= m.vram_gb,
            });
            if let Some(received) = prog.as_ref().and_then(|p| p.get(m.id)).map(|(r, _)| *r) {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("received".into(), json!(received));
                }
            }
            list.push(item);
        }
        json!({ "ok": true, "models": list, "gpuVramGb": vram })
    }

    fn recent_conversions(&self) -> Value {
        if let Ok(response::Body::RecentConversions(resp)) = self.host.ipc_call(
            request::Body::GetRecentConversions(ipc::GetRecentConversions {}),
        ) {
            let entries: Vec<Value> = resp
                .entries
                .into_iter()
                .map(|e| {
                    json!({
                        "time": e.time,
                        "firstKept": e.first_kept,
                        "reading": e.reading,
                        "committed": e.committed,
                        "preceding": e.preceding,
                        "candidates": e.candidates,
                    })
                })
                .collect();
            return json!({ "ok": true, "entries": entries });
        }
        let path = self.host.data_dir().join("asteria_recent_conversions.txt");
        let entries: Vec<Value> = RecentConversions::open(path)
            .recent()
            .into_iter()
            .map(|e| {
                json!({
                    "time": e.time,
                    "firstKept": e.first_kept,
                    "reading": e.reading,
                    "committed": e.committed,
                    "preceding": e.preceding,
                    "candidates": e.candidates,
                })
            })
            .collect();
        json!({ "ok": true, "entries": entries })
    }

    fn export_recent_conversions(&self) -> Value {
        let rc = self.recent_conversions();
        let doc = json!({
            "version": 1,
            "asteria": env!("CARGO_PKG_VERSION"),
            "note": "Asteria の「誤変換の報告」で書き出したファイルです。外へは自動で送りません。",
            "entries": rc.get("entries").cloned().unwrap_or(json!([])),
        });
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        let saved = self
            .host
            .save_text_dialog("asteria-recent-conversions.json", &text);
        json!({ "ok": true, "saved": saved })
    }

    fn download(&self, id: &str) -> Value {
        let Some(m) = EXTRA_MODELS.iter().find(|m| m.id == id) else {
            return fail(&format!("知らないモデルです: {id}"));
        };
        if self.host.gpu_vram_gb() + 0.25 < m.vram_gb {
            return fail("この PC の GPU のメモリでは動かせません。");
        }
        let dir = extra_model_dir(&self.host.data_dir());
        if fs::create_dir_all(&dir).is_err() {
            return fail("保存先のフォルダを作れませんでした。");
        }
        let Ok(mut dl_map) = self.downloads.lock() else {
            return fail("内部状態エラー");
        };
        if dl_map.get(m.id).is_some_and(ModelDownloader::is_running) {
            return json!({ "ok": true });
        }
        let sink = Arc::new(SinkAdapter {
            host: Arc::clone(&self.host),
            progress: Arc::clone(&self.progress),
        });
        let dest = extra_model_path(&self.host.data_dir(), m);
        let worker = ModelDownloader::start(
            m.id.to_owned(),
            m.url.to_owned(),
            dest,
            m.size,
            m.sha256.to_owned(),
            sink,
        );
        dl_map.insert(m.id.to_owned(), worker);
        json!({ "ok": true })
    }

    fn cancel_download(&self, id: &str) -> Value {
        if let Ok(map) = self.downloads.lock() {
            if let Some(d) = map.get(id) {
                d.cancel();
            }
        }
        json!({ "ok": true })
    }

    fn delete_model(&self, id: &str) -> Value {
        let Some(m) = EXTRA_MODELS.iter().find(|m| m.id == id) else {
            return fail(&format!("知らないモデルです: {id}"));
        };
        if let Ok(map) = self.downloads.lock() {
            if map.get(m.id).is_some_and(ModelDownloader::is_running) {
                return fail("受け取りの途中です。先に取り消してください。");
            }
        }
        let path = extra_model_path(&self.host.data_dir(), m);
        let mut part_os = path.as_os_str().to_os_string();
        part_os.push(".part");
        let _ = fs::remove_file(part_os);
        if path.exists() && fs::remove_file(&path).is_err() {
            return fail("消せませんでした。変換エンジンが使っている間は消せないことがあります。");
        }
        let _ = self.host.ipc_call(request::Body::Reload(ipc::Reload {}));
        json!({ "ok": true, "config": default_config_json() })
    }

    fn user_dict_path(&self) -> PathBuf {
        self.host.data_dir().join("user_dictionary.tsv")
    }

    fn open_user_dict(&self) -> UserDictionary {
        let mut ud = UserDictionary::new();
        let path = self.user_dict_path();
        if path.is_file() {
            let _ = ud.load_file(&path);
        }
        ud
    }

    fn user_dict_list(&self) -> Value {
        let ud = self.open_user_dict();
        let items: Vec<Value> = ud
            .entries()
            .iter()
            .map(|e| {
                json!({
                    "reading": e.reading,
                    "surface": e.surface,
                    "pos": e.pos,
                    "comment": e.comment,
                })
            })
            .collect();
        json!({ "ok": true, "entries": items })
    }

    fn user_dict_add(&self, req: &Value) -> Value {
        let reading = req.get("reading").and_then(Value::as_str).unwrap_or("");
        let surface = req.get("surface").and_then(Value::as_str).unwrap_or("");
        let pos = req.get("pos").and_then(Value::as_str).unwrap_or("名詞");
        let comment = req.get("comment").and_then(Value::as_str).unwrap_or("");
        let mut ud = self.open_user_dict();
        ud.add(reading, surface, pos, comment);
        if ud.save_file(&self.user_dict_path()).is_err() {
            return fail("ユーザー辞書を保存できませんでした。");
        }
        let _ = self.host.ipc_call(request::Body::Reload(ipc::Reload {}));
        self.user_dict_list()
    }

    fn user_dict_import(&self, tsv: &str) -> Value {
        let mut ud = self.open_user_dict();
        let added = ud.import_tsv(tsv);
        if ud.save_file(&self.user_dict_path()).is_err() {
            return fail("ユーザー辞書を保存できませんでした。");
        }
        let _ = self.host.ipc_call(request::Body::Reload(ipc::Reload {}));
        json!({ "ok": true, "added": added })
    }

    fn user_dict_export(&self) -> Value {
        let tsv = self.open_user_dict().export_tsv();
        let saved = self
            .host
            .save_text_dialog("asteria-user-dictionary.tsv", &tsv);
        json!({ "ok": true, "saved": saved, "tsv": tsv })
    }
}

fn fail(msg: &str) -> Value {
    json!({ "ok": false, "message": msg })
}

pub fn default_config_json() -> Value {
    json!({
        "asteria_lm_enabled": true,
        "asteria_lm_quality": 1,
        "asteria_live_conversion": false,
        "asteria_homophone_hints": false,
        "asteria_romaji_english": true,
        "asteria_use_integrated_gpu": false,
        "asteria_recent_conversions": false,
        "incognito_mode": false,
    })
}

fn parse_toml_to_config_json(toml_text: &str) -> Value {
    let mut cfg = default_config_json();
    let Some(obj) = cfg.as_object_mut() else {
        return cfg;
    };
    for line in toml_text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = v.trim().trim_matches('"');
        let b = val.eq_ignore_ascii_case("true") || val == "1";
        match key {
            "ai_enabled" => {
                obj.insert("asteria_lm_enabled".into(), json!(b));
            }
            "ai_quality" => {
                let q = match val {
                    "eco" | "low" => 0,
                    "high" => 2,
                    "unreal" => 3,
                    _ => 1,
                };
                obj.insert("asteria_lm_quality".into(), json!(q));
            }
            "live_conversion" => {
                obj.insert("asteria_live_conversion".into(), json!(b));
            }
            "homophone_hints" => {
                obj.insert("asteria_homophone_hints".into(), json!(b));
            }
            "romaji_english" => {
                obj.insert("asteria_romaji_english".into(), json!(b));
            }
            "allow_integrated_gpu" => {
                obj.insert("asteria_use_integrated_gpu".into(), json!(b));
            }
            "record_recent_conversions" => {
                obj.insert("asteria_recent_conversions".into(), json!(b));
            }
            "incognito" => {
                obj.insert("incognito_mode".into(), json!(b));
            }
            _ => {}
        }
    }
    cfg
}

fn config_json_to_toml(cfg: &Value) -> String {
    let b = |k: &str, def: bool| cfg.get(k).and_then(Value::as_bool).unwrap_or(def);
    let q = cfg
        .get("asteria_lm_quality")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let q_str = match q {
        0 => "low",
        2 => "high",
        3 => "unreal",
        _ => "standard",
    };
    format!(
        "ai_enabled = {}\nai_quality = \"{q_str}\"\nlive_conversion = {}\nhomophone_hints = {}\nromaji_english = {}\nallow_integrated_gpu = {}\nrecord_recent_conversions = {}\nincognito = {}\n",
        b("asteria_lm_enabled", true),
        b("asteria_live_conversion", false),
        b("asteria_homophone_hints", false),
        b("asteria_romaji_english", true),
        b("asteria_use_integrated_gpu", false),
        b("asteria_recent_conversions", false),
        b("incognito_mode", false),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeHost {
        dir: PathBuf,
        saved: Mutex<Option<String>>,
    }

    impl BridgeHost for FakeHost {
        fn ipc_call(&self, body: request::Body) -> Result<response::Body, String> {
            match body {
                request::Body::GetConfig(_) => Ok(response::Body::Config(ipc::Config {
                    toml: "live_conversion = true\nai_quality = \"high\"\n".into(),
                })),
                request::Body::SetConfig(_) => Ok(response::Body::Ack(ipc::Ack {})),
                request::Body::GetStatus(_) => Ok(response::Body::Status(ipc::StatusResponse {
                    state: "ready".into(),
                    model: "zenz-v2.5".into(),
                    ..Default::default()
                })),
                request::Body::GetRecentConversions(_) => Ok(response::Body::RecentConversions(
                    ipc::RecentConversionsResponse {
                        entries: Vec::new(),
                    },
                )),
                _ => Ok(response::Body::Ack(ipc::Ack {})),
            }
        }
        fn save_text_dialog(&self, _default_name: &str, text: &str) -> bool {
            *self.saved.lock().unwrap() = Some(text.to_owned());
            true
        }
        fn open_support_page(&self) {}
        fn push_event(&self, _event: Value) {}
        fn gpu_vram_gb(&self) -> f64 {
            8.0
        }
        fn data_dir(&self) -> PathBuf {
            self.dir.clone()
        }
    }

    #[test]
    fn html_inlined_and_bridge_ops_work() {
        let html = bundled_html();
        assert!(html.contains("<style>"));
        assert!(html.contains("<script>"));
        assert!(!html.contains("settings.css"));

        let dir = std::env::temp_dir().join(format!("asteria-cfg-test-{}", std::process::id()));
        let host = Arc::new(FakeHost {
            dir: dir.clone(),
            saved: Mutex::new(None),
        });
        let bridge = SettingsBridge::new(host.clone());
        let resp: Value =
            serde_json::from_str(&bridge.handle_json(r#"{"id":1,"op":"load"}"#)).unwrap();
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["ok"], true);
        assert_eq!(resp["result"]["config"]["asteria_live_conversion"], true);
        assert_eq!(resp["result"]["config"]["asteria_lm_quality"], 2);

        let models = bridge.dispatch(&json!({"op":"models"}));
        assert_eq!(models["models"].as_array().unwrap().len(), 2);

        let diag = bridge.dispatch(&json!({"op":"saveDiagnostics"}));
        assert_eq!(diag["saved"], true);
        assert!(host
            .saved
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .contains("Asteria"));
    }
}
