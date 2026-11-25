//! Asteria の候補の窓(カード 98)。Mozc の mozc_renderer と同じ約束で `RendererCommand` を受ける。
//!
//! 約束(Mozc の ipc/ipc_path_manager.cc・ipc/win32_ipc.cc・renderer/renderer_server.cc):
//! - プロファイルの `renderer.ipc` に `IPCPathInfo`(鍵・pid・版)をそのまま書く。
//! - 名前付きパイプ `\.\pipe\kotori.<鍵>.renderer` をメッセージ単位で開く。1 回の書き込みが
//!   `RendererCommand` 1 つ。返事は空。

// 生成したコード(Mozc の .proto のコメントをそのまま持つ)には lint をかけない。
#[allow(clippy::all, clippy::pedantic, rustdoc::all)]
pub mod mozc {
    // engine_builder.proto(package mozc)。
    include!(concat!(env!("OUT_DIR"), "/mozc.rs"));
    pub mod commands {
        include!(concat!(env!("OUT_DIR"), "/mozc.commands.rs"));
    }
    pub mod config {
        include!(concat!(env!("OUT_DIR"), "/mozc.config.rs"));
    }
    pub mod user_dictionary {
        include!(concat!(env!("OUT_DIR"), "/mozc.user_dictionary.rs"));
    }
    pub mod ipc {
        include!(concat!(env!("OUT_DIR"), "/mozc.ipc.rs"));
    }
}

use mozc::commands::RendererCommand;
use prost::Message;

/// Mozc の `IPC_PROTOCOL_VERSION`(ipc/ipc.h)。
pub const IPC_PROTOCOL_VERSION: u32 = 3;
/// Mozc の `kIPCPrefix`(base/const.h、Asteria のブランド)。
pub const IPC_PREFIX: &str = r"\.\pipe\kotori.";
/// 鍵の長さ(Mozc の kKeySize)。
pub const KEY_SIZE: usize = 32;

/// 鍵からパイプの名前を作る。
pub fn pipe_name(key: &str) -> String {
    format!("{IPC_PREFIX}{key}.renderer")
}

/// 16 バイトの乱数から鍵(32 文字の 16 進)を作る。
pub fn key_from_random(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Mozc の鍵として正しいか(32 文字、英数字。ipc_path_manager.cc の IsValidKey)。
pub fn is_valid_key(key: &str) -> bool {
    key.len() == KEY_SIZE && key.bytes().all(|c| c.is_ascii_alphanumeric())
}

/// `renderer.ipc` に書く内容。
pub fn path_info(key: &str, process_id: u32, product_version: &str) -> Vec<u8> {
    mozc::ipc::IpcPathInfo {
        protocol_version: Some(IPC_PROTOCOL_VERSION),
        product_version: Some(product_version.to_string()),
        key: Some(key.to_string()),
        process_id: Some(process_id),
        thread_id: Some(0),
    }
    .encode_to_vec()
}

/// パイプのセキュリティ記述子(Mozc の WinSandbox::kSharablePipe と同じ)。
pub fn pipe_sddl(user_sid: &str) -> String {
    format!("D:(A;;;;;OW)(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;AC)(A;;GA;;;{user_sid})S:(ML;;NX;;;LW)")
}

/// 受けた 1 通を読む。壊れた内容は None(窓は前の状態のまま)。
pub fn decode_command(bytes: &[u8]) -> Option<RendererCommand> {
    RendererCommand::decode(bytes).ok()
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use mozc::commands::renderer_command::CommandType;

    #[test]
    fn pipe_name_matches_mozc() {
        assert_eq!(
            pipe_name("0123456789abcdef0123456789abcdef"),
            r"\.\pipe\kotori.0123456789abcdef0123456789abcdef.renderer"
        );
    }

    #[test]
    fn key_is_valid_for_mozc() {
        let key = key_from_random(&[0xab; 16]);
        assert_eq!(key.len(), KEY_SIZE);
        assert!(is_valid_key(&key));
        assert!(!is_valid_key("short"));
        assert!(!is_valid_key(&"-".repeat(KEY_SIZE)));
    }

    #[test]
    fn path_info_round_trips() {
        let bytes = path_info("k".repeat(32).as_str(), 42, "3.34.6518.100");
        let info = mozc::ipc::IpcPathInfo::decode(bytes.as_slice()).unwrap_or_default();
        assert_eq!(info.key(), "k".repeat(32));
        assert_eq!(info.process_id(), 42);
        assert_eq!(info.protocol_version(), IPC_PROTOCOL_VERSION);
        assert_eq!(info.product_version(), "3.34.6518.100");
    }

    #[test]
    fn decodes_update_command() {
        let cmd = RendererCommand {
            r#type: Some(CommandType::Update as i32),
            visible: Some(true),
            ..Default::default()
        };
        let got = decode_command(&cmd.encode_to_vec());
        assert_eq!(got.as_ref().map(|c| c.r#type()), Some(CommandType::Update));
        assert_eq!(got.map(|c| c.visible()), Some(true));
    }

    #[test]
    fn rejects_garbage() {
        assert!(decode_command(&[0xff, 0xff, 0xff]).is_none());
    }
}

#[cfg(windows)]
pub mod win;
