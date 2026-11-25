//! エンジン本体(4章)。
//!
//! 使い方: asteria-server [--dict <システム辞書>] [--model <LM の GGUF>]
//! 辞書は既定で実行ファイルの隣の data/system.dict、なければ同梱の辞書の置き場所(12.1)
//! から読む。読み込みはバックグラウンドで行い、終わるまで(または読めなかったとき)は
//! キーをアプリへ渡す。LM は同じ data フォルダの zenz-v2.5-small-f16.gguf を探し、
//! なければラティス単体で変換する(REQ-6-4)。

#[cfg(any(unix, windows))]
use std::sync::{Arc, Mutex};

/// 推論のスレッド数(4.3 の推論ワーカー、既定 2)。
#[cfg(any(unix, windows))]
const LM_THREADS: i32 = 2;

/// 引数がなければ、候補のうち存在する最初のものを選ぶ。
#[cfg(any(unix, windows))]
fn first_existing(candidates: Vec<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    candidates.into_iter().find(|p| p.is_file())
}

/// 辞書(と LM)をバックグラウンドで読み込み、読めたらサーバーに変換の部品を持たせる。
#[cfg(any(unix, windows))]
fn load_engine_in_background(
    server: &Arc<Mutex<asteria_server::Server>>,
    args: asteria_server::Args,
) {
    let exe = std::env::current_exe().ok();
    let dict_path = args.dict.or_else(|| {
        let candidates = asteria_server::dict_candidates(exe.as_deref());
        // どれもなければ最後の候補を読みにいき、読めない警告を出す。
        let last = candidates.last().cloned();
        first_existing(candidates).or(last)
    });
    let model_path = args
        .model
        .or_else(|| first_existing(asteria_server::model_candidates(exe.as_deref())));
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let Some(path) = dict_path else {
            eprintln!("asteria-server: 辞書の置き場所を決められない。変換せずにキーを渡す");
            return;
        };
        let dict = asteria_dict::Dictionary::open(&path).map_err(|e| e.to_string());
        let dict = match dict {
            Ok(dict) => dict,
            Err(e) => {
                eprintln!(
                    "asteria-server: 辞書 {} を読めない({e})。変換せずにキーを渡す",
                    path.display()
                );
                return;
            }
        };
        eprintln!("asteria-server: 辞書 {} を読み込んだ", path.display());
        let runtime_dir = exe
            .as_deref()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let search_dirs: Vec<&std::path::Path> = match path.parent() {
            Some(d) if d != runtime_dir.as_path() => vec![d, runtime_dir.as_path()],
            _ => vec![runtime_dir.as_path()],
        };
        let find_data = |names: &[&str]| -> Option<std::path::PathBuf> {
            for dir in &search_dirs {
                for name in names {
                    let p = dir.join(name);
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
            None
        };
        let collocations = find_data(&["fast_collocations.txt"])
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| asteria_dict::CollocationTable::parse_txt(&t));
        let english = find_data(&["AsteriaEnglish.tsv", "KotoriEnglish.tsv"])
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| asteria_dict::EnglishLexicon::parse_tsv(&t));
        let (
            user_dict,
            telemetry,
            ai_quality,
            gpu_enabled,
            allow_integrated_gpu,
            bundled,
            user_hist,
        ) = server
            .lock()
            .map(|s| {
                let st = s.settings();
                (
                    s.user_dictionary(),
                    s.telemetry_handle(),
                    st.ai_quality.clone(),
                    st.gpu_enabled,
                    st.allow_integrated_gpu,
                    Some(s.bundled_defaults()),
                    Some(s.user_history()),
                )
            })
            .unwrap_or_default();
        let dict = Arc::new(dict);
        let mut engine = asteria_server::Engine::from_bundle_arc(
            Arc::clone(&dict),
            collocations,
            english,
            user_dict,
        );
        let profile_dir = asteria_server::state::default_data_dir();
        let tier = asteria_lm::catalog::LmQuality::parse(&ai_quality);
        let bundle = asteria_lm::catalog::ModelBundlePaths::discover_in(
            &search_dirs,
            profile_dir.as_deref(),
            tier,
        );
        let has_bundle_model = bundle.zenz_small.is_some()
            || bundle.genmai.is_some()
            || bundle.native_generator.is_some();
        let explicit_or_default = model_path.or_else(|| bundle.zenz_small.clone());
        if explicit_or_default.is_some() || has_bundle_model {
            let weights_probe = explicit_or_default
                .as_deref()
                .or(bundle.genmai.as_deref())
                .or(bundle.zenz_small.as_deref());
            let weights = weights_probe
                .and_then(|p| asteria_lm::Model::load_vocab_only(p).ok())
                .and_then(|m| asteria_lm::ScoreWeights::from_model(&m).ok())
                .unwrap_or_default();
            let override_path = if explicit_or_default.as_deref() != bundle.zenz_small.as_deref() {
                explicit_or_default
            } else {
                None
            };
            let _ = LM_THREADS;
            eprintln!(
                "asteria-server: LM バンドル ({}) を読み込む",
                runtime_dir.display()
            );
            let validator: asteria_lm::multi_model::ReadingValidator =
                Arc::new(move |r, t| asteria_lattice::matches_reading(&dict, r, t));
            let variants: asteria_lm::multi_model::VariantProvider = Arc::new(move |r, ctx, t| {
                let mut out = Vec::new();
                if let Some(b) = &bundled {
                    out.extend(b.variants(r, ctx, t));
                }
                if let Some(u) = &user_hist {
                    out.extend(u.variants(r, ctx, t));
                }
                out.extend(asteria_lattice::counter_arabic_variants(t));
                out
            });
            let reranker = asteria_lm::rerank::Reranker::spawn(move || {
                asteria_lm::multi_model::MultiModelScorer::open(
                    override_path.as_deref(),
                    &bundle,
                    &runtime_dir,
                    tier,
                    gpu_enabled,
                    allow_integrated_gpu,
                    telemetry,
                )
                .map(|s| {
                    s.with_reading_validator(validator)
                        .with_variant_provider(variants)
                })
            });
            engine = engine.with_reranker(reranker, weights);
        } else {
            eprintln!("asteria-server: LM がないのでラティス単体で変換する");
        }
        if let Ok(mut s) = server.lock() {
            s.set_engine(engine);
        }
    });
}

#[cfg(unix)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;

    let path = asteria_proto::default_socket_path().context("ソケットの置き場所を決められない")?;
    let dir = path.parent().context("ソケットの親ディレクトリがない")?;
    std::fs::create_dir_all(dir).with_context(|| format!("{} を作れない", dir.display()))?;
    let lock = dir.join("server.lock");
    let _guard = asteria_server::InstanceGuard::acquire_lock_file(&lock)
        .with_context(|| format!("{} をロックできない", lock.display()))?;
    eprintln!("asteria-server: {} で待ち受け", path.display());
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = asteria_server::Args::parse(&args).map_err(anyhow::Error::msg)?;
    let srv = match asteria_server::state::default_data_dir() {
        Some(dir) => asteria_server::Server::with_data_dir(None, dir),
        None => asteria_server::Server::new(),
    };
    let server = Arc::new(Mutex::new(srv));
    load_engine_in_background(&server, args);
    asteria_server::listen_unix(&path, server)
        .with_context(|| format!("{} で待ち受けできない", path.display()))
}

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use anyhow::Context;

    let name = asteria_client::default_pipe_name().context("パイプ名を決められない")?;
    let _guard =
        asteria_server::InstanceGuard::acquire().context("単一インスタンスを確保できない")?;
    eprintln!("asteria-server: {name} で待ち受け");
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let args = asteria_server::Args::parse(&args).map_err(anyhow::Error::msg)?;
    let srv = match asteria_server::state::default_data_dir() {
        Some(dir) => asteria_server::Server::with_data_dir(None, dir),
        None => asteria_server::Server::new(),
    };
    let server = Arc::new(Mutex::new(srv));
    load_engine_in_background(&server, args);
    asteria_server::listen_pipe(&name, server).with_context(|| format!("{name} で待ち受けできない"))
}

#[cfg(not(any(unix, windows)))]
fn main() -> anyhow::Result<()> {
    anyhow::bail!("asteria-server: この OS のトランスポートは未実装")
}
