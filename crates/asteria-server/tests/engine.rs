//! サーバーでの変換(状態機械の組み込み、docs/SPEC.md 11.1、REQ-10-3、REQ-4-5)。

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use asteria_proto::ipc::{self, request, response, ErrorCode};
use asteria_server::{Engine, Server};
use asteria_session::{Converter, SegmentCandidates};

/// 「キョウ」を「今日/京」に、「パニック」でパニックし、それ以外は読みのひらがなにする。
struct Fake;

impl Converter for Fake {
    fn convert(&self, reading: &str) -> Vec<SegmentCandidates> {
        match reading {
            "キョウ" => vec![SegmentCandidates {
                len: 3,
                candidates: vec!["今日".into(), "京".into()],
            }],
            "パニック" => panic!("変換器の不具合"),
            r => vec![SegmentCandidates {
                len: r.chars().count(),
                candidates: vec![r.into()],
            }],
        }
    }
}

fn engine() -> Engine {
    Engine::with_converter(Arc::new(Fake))
}

fn create(server: &mut Server, scope: ipc::InputScope) -> u64 {
    match server.handle(Some(request::Body::CreateSession(ipc::CreateSession {
        app_id: "test.exe".into(),
        input_scope: scope.into(),
    }))) {
        response::Body::SessionCreated(c) => c.session_id,
        other => panic!("{other:?}"),
    }
}

fn key(server: &mut Server, id: u64, vk: u32, text: &str) -> ipc::Output {
    match server.handle(Some(request::Body::SendKey(ipc::SendKey {
        session_id: id,
        virtual_key: vk,
        text: text.into(),
        modifiers: None,
        key_up: false,
    }))) {
        response::Body::Output(o) => o,
        other => panic!("{other:?}"),
    }
}

fn type_text(server: &mut Server, id: u64, text: &str) -> ipc::Output {
    let mut out = ipc::Output::default();
    for c in text.chars() {
        out = key(server, id, c.to_ascii_uppercase() as u32, &c.to_string());
    }
    out
}

const SPACE: u32 = 0x20;
const ENTER: u32 = 0x0D;

#[test]
fn keys_are_converted_and_committed() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let out = type_text(&mut server, id, "kyou");
    assert!(out.consumed);
    assert_eq!(out.input_mode(), ipc::InputMode::Hiragana);
    assert_eq!(out.preedit.len(), 1);
    assert_eq!(out.preedit[0].text, "きょう");
    assert_eq!(out.preedit[0].attribute(), ipc::SegmentAttribute::Input);
    assert_eq!(out.cursor, 3);

    let out = key(&mut server, id, SPACE, "");
    assert_eq!(out.preedit[0].text, "今日");
    assert_eq!(out.preedit[0].attribute(), ipc::SegmentAttribute::Focused);
    assert_eq!(out.candidate_window, None);
    let out = key(&mut server, id, SPACE, "");
    let w = out.candidate_window.unwrap();
    assert!(w.visible);
    assert_eq!(w.focused_index, 1);
    assert_eq!(w.candidates[1].text, "京");

    let out = key(&mut server, id, ENTER, "");
    assert_eq!(out.committed_text, "京");
    assert!(out.preedit.is_empty());
    // 待機中の Enter はアプリへ渡す。
    assert!(!key(&mut server, id, ENTER, "").consumed);
}

#[test]
fn password_fields_key_up_and_missing_engine_pass_through() {
    let mut server = Server::with_engine(engine());
    let pw = create(&mut server, ipc::InputScope::Password);
    assert!(!type_text(&mut server, pw, "a").consumed, "REQ-10-3");

    let id = create(&mut server, ipc::InputScope::Default);
    let up = server.handle(Some(request::Body::SendKey(ipc::SendKey {
        session_id: id,
        virtual_key: 0x41,
        text: "a".into(),
        modifiers: None,
        key_up: true,
    })));
    assert!(matches!(up, response::Body::Output(o) if !o.consumed));

    // 辞書を読み込む前はアプリへ渡し、読み込んだあとは同じセッションで変換する。
    let mut server = Server::new();
    let id = create(&mut server, ipc::InputScope::Default);
    assert!(!type_text(&mut server, id, "a").consumed);
    server.set_engine(engine());
    assert_eq!(type_text(&mut server, id, "a").preedit[0].text, "あ");
}

#[test]
fn panic_resets_only_that_session() {
    let mut server = Server::with_engine(engine());
    let bad = create(&mut server, ipc::InputScope::Default);
    let good = create(&mut server, ipc::InputScope::Default);
    type_text(&mut server, good, "kyou");
    type_text(&mut server, bad, "panikku");
    // 変換器のパニックはキーをアプリへ渡して、そのセッションを作り直す(REQ-4-5)。
    let out = key(&mut server, bad, SPACE, "");
    assert!(!out.consumed);
    assert_eq!(type_text(&mut server, bad, "a").preedit[0].text, "あ");
    // ほかのセッションは続きから使える。
    assert_eq!(key(&mut server, good, SPACE, "").preedit[0].text, "今日");
}

#[test]
fn commands_commit_and_cancel() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let command = |server: &mut Server, kind: ipc::CommandKind| {
        server.handle(Some(request::Body::SendCommand(ipc::SendCommand {
            session_id: id,
            kind: kind.into(),
            argument: 0,
        })))
    };
    type_text(&mut server, id, "kyou");
    key(&mut server, id, SPACE, "");
    match command(&mut server, ipc::CommandKind::Commit) {
        response::Body::Output(o) => assert_eq!(o.committed_text, "今日"),
        other => panic!("{other:?}"),
    }
    type_text(&mut server, id, "a");
    match command(&mut server, ipc::CommandKind::Cancel) {
        response::Body::Output(o) => assert!(o.preedit.is_empty() && o.committed_text.is_empty()),
        other => panic!("{other:?}"),
    }
    match command(&mut server, ipc::CommandKind::Reconvert) {
        response::Body::Error(e) => assert_eq!(e.code(), ErrorCode::Unimplemented),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dict_candidates_prefers_exe_dir() {
    use std::path::Path;
    let exe = Path::new("/opt").join("asteria").join("asteria-server");
    let c = asteria_server::dict_candidates(Some(&exe));
    let next_to_exe = Path::new("/opt")
        .join("asteria")
        .join("data")
        .join("system.dict");
    assert_eq!(c[0], next_to_exe);
    assert_eq!(c.get(1), asteria_server::default_dict_path().as_ref());
    // 実行ファイルの場所が分からなければ既定の置き場所だけ。
    assert_eq!(
        asteria_server::dict_candidates(None),
        asteria_server::default_dict_path()
            .into_iter()
            .collect::<Vec<_>>()
    );
}

fn set_context(server: &mut Server, id: u64, text: &str) -> response::Body {
    server.handle(Some(request::Body::SetContext(ipc::SetContext {
        session_id: id,
        left_context: text.into(),
        ..Default::default()
    })))
}

#[test]
fn set_context_is_kept_and_grows_on_commit() {
    // 辞書を読み込む前に届いた左文脈も、状態機械を作るときに渡す。
    let mut server = Server::new();
    let id = create(&mut server, ipc::InputScope::Default);
    assert!(matches!(
        set_context(&mut server, id, "明日の"),
        response::Body::Ack(_)
    ));
    assert_eq!(server.left_context(id), Some("明日の"));
    server.set_engine(engine());
    type_text(&mut server, id, "kyou");
    key(&mut server, id, SPACE, "");
    key(&mut server, id, ENTER, "");
    assert_eq!(server.left_context(id), Some("明日の今日"));

    // 置き換える。
    set_context(&mut server, id, "昨日");
    assert_eq!(server.left_context(id), Some("昨日"));

    // パスワード欄の文脈は受け取らない(REQ-10-3)。知らないセッションはエラー。
    let pw = create(&mut server, ipc::InputScope::Password);
    set_context(&mut server, pw, "secret");
    assert_eq!(server.left_context(pw), Some(""));
    assert!(matches!(
        set_context(&mut server, 999, "x"),
        response::Body::Error(e) if e.code() == ErrorCode::UnknownSession
    ));
    assert_eq!(server.left_context(999), None);
}

#[test]
fn select_candidate_command_moves_the_selection() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let select = |server: &mut Server, index: u32| match server.handle(Some(
        request::Body::SendCommand(ipc::SendCommand {
            session_id: id,
            kind: ipc::CommandKind::SelectCandidate.into(),
            argument: index,
        }),
    )) {
        response::Body::Output(o) => o,
        other => panic!("{other:?}"),
    };
    type_text(&mut server, id, "kyou");
    key(&mut server, id, SPACE, "");
    key(&mut server, id, SPACE, "");
    let out = select(&mut server, 0);
    assert_eq!(out.preedit[0].text, "今日");
    let w = out.candidate_window.unwrap();
    assert!(w.visible);
    assert_eq!(w.focused_index, 0);
    // 範囲外は無視する。
    let out = select(&mut server, 5);
    assert_eq!(out.candidate_window.unwrap().focused_index, 0);
}

#[test]
fn poll_update_without_lm_reports_no_change() {
    let mut server = Server::with_engine(engine());
    let id = create(&mut server, ipc::InputScope::Default);
    let poll = |server: &mut Server, id: u64| {
        server.handle(Some(request::Body::PollUpdate(ipc::PollUpdate {
            session_id: id,
        })))
    };
    type_text(&mut server, id, "kyou");
    key(&mut server, id, SPACE, "");
    assert!(matches!(
        poll(&mut server, id),
        response::Body::Output(o) if !o.consumed && o.preedit.is_empty()
    ));
    assert!(matches!(
        poll(&mut server, 999),
        response::Body::Error(e) if e.code() == ErrorCode::UnknownSession
    ));
}

#[test]
fn args_and_model_location() {
    use asteria_server::Args;
    use std::path::{Path, PathBuf};
    let args = |a: &[&str]| Args::parse(&a.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert_eq!(args(&[]), Ok(Args::default()));
    assert_eq!(
        args(&["--model", "m.gguf", "--dict", "d.dict"]),
        Ok(Args {
            dict: Some(PathBuf::from("d.dict")),
            model: Some(PathBuf::from("m.gguf")),
        })
    );
    assert!(args(&["--dict"]).is_err());
    assert!(args(&["--verbose"]).is_err());

    // LM のモデルは辞書と同じ data フォルダを探す。
    let exe = Path::new("/opt").join("asteria").join("asteria-server");
    let c = asteria_server::model_candidates(Some(&exe));
    let expected = Path::new("/opt")
        .join("asteria")
        .join("data")
        .join(asteria_server::MODEL_FILE);
    assert_eq!(c[0], expected);
}

#[test]
fn config_status_recent_and_context_learning_work_end_to_end() {
    let dir = std::env::temp_dir().join(format!("asteria-server-m4-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    {
        let mut server = Server::with_data_dir(Some(engine()), dir.clone());
        let id = create(&mut server, ipc::InputScope::Default);
        let status = match server.handle(Some(request::Body::GetStatus(ipc::GetStatus {}))) {
            response::Body::Status(s) => s,
            other => panic!("{other:?}"),
        };
        assert_eq!(status.state, "disabled");
        assert!(dir.join("asteria_ai_status.txt").is_file());

        let cfg = match server.handle(Some(request::Body::GetConfig(ipc::GetConfig {}))) {
            response::Body::Config(c) => c,
            other => panic!("{other:?}"),
        };
        assert!(cfg.toml.contains("live_conversion = false"));
        server.handle(Some(request::Body::SetConfig(ipc::SetConfig {
            toml:
                "live_conversion = true\nhomophone_hints = true\nrecord_recent_conversions = true\n"
                    .into(),
        })));

        // 「今日/京」の2番目を選んで確定し、学習と直近変換記録を確認
        server.handle(Some(request::Body::SetContext(ipc::SetContext {
            session_id: id,
            left_context: "明日の前に".into(),
            following_text: "です".into(),
            ..Default::default()
        })));
        type_text(&mut server, id, "kyou");
        key(&mut server, id, SPACE, "");
        key(&mut server, id, SPACE, ""); // 2番目「京」を選択
        let out = key(&mut server, id, ENTER, "");
        assert_eq!(out.committed_text, "京");

        let recent = match server.handle(Some(request::Body::GetRecentConversions(
            ipc::GetRecentConversions {},
        ))) {
            response::Body::RecentConversions(r) => r,
            other => panic!("{other:?}"),
        };
        assert_eq!(recent.entries.len(), 1);
        assert_eq!(recent.entries[0].committed, "京");
        assert!(!recent.entries[0].first_kept);

        // パスワード欄の入力は記録・学習されない
        let pw = create(&mut server, ipc::InputScope::Password);
        type_text(&mut server, pw, "kyou");
        key(&mut server, pw, ENTER, "");
        let recent_after_pw = match server.handle(Some(request::Body::GetRecentConversions(
            ipc::GetRecentConversions {},
        ))) {
            response::Body::RecentConversions(r) => r,
            other => panic!("{other:?}"),
        };
        assert_eq!(recent_after_pw.entries.len(), 1);

        server.handle(Some(request::Body::ClearRecentConversions(
            ipc::ClearRecentConversions {},
        )));
        let cleared = match server.handle(Some(request::Body::GetRecentConversions(
            ipc::GetRecentConversions {},
        ))) {
            response::Body::RecentConversions(r) => r,
            other => panic!("{other:?}"),
        };
        assert!(cleared.entries.is_empty());
    }

    // 設定・文脈学習の永続化と再起動後の ExactSentence 再利用を確認
    let mut reopened = Server::with_data_dir(Some(engine()), dir.clone());
    let cfg2 = match reopened.handle(Some(request::Body::GetConfig(ipc::GetConfig {}))) {
        response::Body::Config(c) => c,
        other => panic!("{other:?}"),
    };
    assert!(cfg2.toml.contains("live_conversion = true"));
    assert!(cfg2.toml.contains("homophone_hints = true"));

    let id2 = create(&mut reopened, ipc::InputScope::Default);
    reopened.handle(Some(request::Body::SetContext(ipc::SetContext {
        session_id: id2,
        left_context: "明日の前に".into(),
        following_text: String::new(),
        ..Default::default()
    })));
    type_text(&mut reopened, id2, "kyou");
    let out2 = key(&mut reopened, id2, SPACE, "");
    assert_eq!(out2.preedit[0].text, "京");

    reopened.handle(Some(request::Body::SendCommand(ipc::SendCommand {
        session_id: id2,
        kind: ipc::CommandKind::ClearUserHistory.into(),
        argument: 0,
    })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn prediction_nuance_typo_and_user_dict_reload_work() {
    let dir = std::env::temp_dir().join(format!("asteria-server-phase-a-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut b = asteria_dict::DictBuilder::new();
    b.add("キョウ", "今日", 1, 1, 2000, 0).unwrap();
    b.add("キョウト", "京都", 1, 1, 1500, 0).unwrap();
    b.add("カイトウ", "回答", 1, 1, 1500, 0).unwrap();
    b.add("シュッパツ", "出発", 1, 1, 1500, 0).unwrap();
    b.set_connection(2, 2, vec![0; 4]).unwrap();
    let d = asteria_dict::Dictionary::from_bytes(b.build().unwrap()).unwrap();

    let mut server = Server::with_data_dir(None, dir.clone());
    let eng = Engine::from_bundle(d, None, None, server.user_dictionary());
    server.set_engine(eng);

    let id = create(&mut server, ipc::InputScope::Default);

    // 1. 予測候補と Tab (CommitPrediction) 確定
    type_text(&mut server, id, "kyou");
    let view = match server.handle(Some(request::Body::SendCommand(ipc::SendCommand {
        session_id: id,
        kind: ipc::CommandKind::CommitPrediction.into(),
        argument: 0,
    }))) {
        response::Body::Output(o) => o,
        other => panic!("{other:?}"),
    };
    assert!(
        view.committed_text == "今日" || view.committed_text == "京都",
        "unexpected committed prediction: {}",
        view.committed_text
    );

    // 2. ニュアンス候補展開 (回答 -> 解答)
    type_text(&mut server, id, "kaitou");
    key(&mut server, id, SPACE, "");
    let w = key(&mut server, id, SPACE, "");
    let cands: Vec<String> = w
        .candidate_window
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.text)
        .collect();
    let cancel = |srv: &mut Server, sid: u64| {
        srv.handle(Some(request::Body::SendCommand(ipc::SendCommand {
            session_id: sid,
            kind: ipc::CommandKind::Cancel.into(),
            argument: 0,
        })));
    };
    assert!(cands.iter().any(|c| c == "回答") && cands.iter().any(|c| c == "解答"));
    cancel(&mut server, id);

    // 3. ユーザー辞書更新 + Reload で変換先頭に即反映
    std::fs::write(dir.join("user_dictionary.tsv"), "キョウ\t今日庵\t名詞\t\n").unwrap();
    server.handle(Some(request::Body::Reload(ipc::Reload {})));
    type_text(&mut server, id, "kyou");
    let after_reload = key(&mut server, id, SPACE, "");
    assert_eq!(after_reload.preedit[0].text, "今日庵");
    cancel(&mut server, id);

    // 4. 拗音抜けタイポ補正 (しっぱつ -> しゅっぱつ)
    type_text(&mut server, id, "sippatu");
    key(&mut server, id, SPACE, "");
    let w_typo = key(&mut server, id, SPACE, "");
    let typo_cands: Vec<String> = w_typo
        .candidate_window
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.text)
        .collect();
    assert!(
        typo_cands.iter().any(|c| c == "出発"),
        "expected typo candidate 出発 in {typo_cands:?}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
