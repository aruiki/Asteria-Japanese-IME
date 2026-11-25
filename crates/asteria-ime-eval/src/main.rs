//! 評価ツール(docs/SPEC.md 14 章)。
//!
//! 使い方:
//!   asteria-ime-eval repl <辞書>
//!   asteria-ime-eval run [--lm <GGUF>] [--bundle <ディレクトリ>] [--runtime-dir <ディレクトリ>] [--tier <tier>] [--k <件数>] [--score-weights <JSON>] <辞書> <出力ディレクトリ> <名前>=<評価セットの JSON>...
//!   asteria-ime-eval bench-lm [--runtime-dir <ディレクトリ>] [--tier <tier>] [--k <件数>] <辞書> <GGUFまたはバンドルDir> <評価セットの JSON>
//!   asteria-ime-eval tune [--k <件数>] <辞書> <GGUF> <開発用データ>

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use asteria_dict::Dictionary;
use asteria_ime_eval::eval::{Reorder, Rerank};
use asteria_learn::{rerank_with_histories, ContextHistory};
use asteria_lm::catalog::{LmQuality, ModelBundlePaths};
use asteria_lm::multi_model::{
    MultiModelScorer, ReadingValidator, RuntimeTelemetry, SharedTelemetry, VariantProvider,
};
use asteria_lm::rerank::Scorer;
use asteria_lm::zenz::ZenzScorer;
use asteria_lm::ScoreWeights;

/// 既定で並べ替える上位の件数(REQ-6-1 の K)。
const DEFAULT_K: usize = 16;
/// 推論のスレッド数(6.4)。
const THREADS: i32 = 4;
const CONTEXT_HISTORY_MARGIN: f32 = 6.0;

/// `run` / `bench-lm` のオプション。
#[derive(Default)]
struct RunOptions {
    lm: Option<String>,
    bundle: Option<String>,
    runtime_dir: Option<String>,
    tier: Option<String>,
    k: Option<usize>,
    weights: Option<String>,
}

/// 先頭のオプションを取り除き、残りの引数を返す。
fn parse_options(mut args: &[String]) -> Result<(RunOptions, &[String])> {
    let mut opts = RunOptions::default();
    while let [flag, value, rest @ ..] = args {
        match flag.as_str() {
            "--lm" => opts.lm = Some(value.clone()),
            "--bundle" => opts.bundle = Some(value.clone()),
            "--runtime-dir" => opts.runtime_dir = Some(value.clone()),
            "--tier" => opts.tier = Some(value.clone()),
            "--k" => opts.k = Some(value.parse().context("--k は正の整数")?),
            "--score-weights" => opts.weights = Some(value.clone()),
            _ => break,
        }
        args = rest;
    }
    Ok((opts, args))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a.starts_with("--user_profile_dir=")) {
        return run_converter_runner(&args);
    }
    match args.as_slice() {
        [cmd, dict] if cmd == "repl" => repl(dict),
        [cmd, rest @ ..] if cmd == "converter-runner" => run_converter_runner(rest),
        [cmd, rest @ ..] if cmd == "run" => match parse_options(rest)? {
            (opts, [dict, out, sets @ ..]) if !sets.is_empty() => run(&opts, dict, out, sets),
            _ => bail!(USAGE),
        },
        [cmd, rest @ ..] if cmd == "bench-lm" => match parse_options(rest)? {
            (opts, [dict, model, set]) if opts.lm.is_none() && opts.weights.is_none() => {
                bench_lm(&opts, dict, model, set)
            }
            _ => bail!(USAGE),
        },
        [cmd, rest @ ..] if cmd == "tune" => match parse_options(rest)? {
            (opts, [dict, model, set]) if opts.lm.is_none() && opts.weights.is_none() => {
                tune(opts.k.unwrap_or(DEFAULT_K), dict, model, set)
            }
            _ => bail!(USAGE),
        },
        _ => bail!(USAGE),
    }
}

const USAGE: &str = "使い方: asteria-ime-eval repl <辞書>
        asteria-ime-eval run [--lm <GGUF>] [--bundle <Dir>] [--runtime-dir <Dir>] [--tier <tier>] [--k <件数>] [--score-weights <JSON>] <辞書> <出力ディレクトリ> <名前>=<評価セット>...
        asteria-ime-eval bench-lm [--runtime-dir <Dir>] [--tier <tier>] [--k <件数>] <辞書> <GGUFまたはバンドルDir> <評価セット>
        asteria-ime-eval tune [--k <件数>] <辞書> <GGUF> <開発用データ>";

fn resolve_runtime_dir(explicit: Option<&str>, model_hint: Option<&Path>) -> PathBuf {
    if let Some(d) = explicit {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("ASTERIA_RUNTIME_DIR") {
        return PathBuf::from(d);
    }
    if let Some(p) = model_hint {
        let dir = if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(p)
        };
        if dir.join("llama.dll").is_file() {
            return dir.to_path_buf();
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("llama.dll").is_file() {
                return dir.to_path_buf();
            }
        }
    }
    PathBuf::from(".")
}

struct EvalPipelineScorer {
    inner: Box<dyn Scorer>,
    defaults: Option<Arc<ContextHistory>>,
}

impl Scorer for EvalPipelineScorer {
    fn prewarm(&mut self, left_context: &str, reading: &str) {
        self.inner.prewarm(left_context, reading);
    }

    fn score(
        &mut self,
        request: &asteria_lm::rerank::RerankRequest,
        cancel: &asteria_lm::rerank::CancelToken,
    ) -> Option<Vec<f32>> {
        self.inner.score(request, cancel)
    }

    fn score_and_generate(
        &mut self,
        request: &asteria_lm::rerank::RerankRequest,
        cancel: &asteria_lm::rerank::CancelToken,
    ) -> Option<asteria_lm::rerank::GenerationResult> {
        let mut res = self.inner.score_and_generate(request, cancel)?;
        if let Some(hist) = &self.defaults {
            let refs = [hist.as_ref()];
            if let (Some(best), Some((cur_best, &cur_max))) = (
                rerank_with_histories(
                    &refs,
                    &request.reading,
                    &request.left_context,
                    &res.candidates,
                    &res.scores,
                    CONTEXT_HISTORY_MARGIN,
                ),
                res.scores
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.total_cmp(b.1)),
            ) {
                if best != cur_best && cur_max.is_finite() && res.scores[best].is_finite() {
                    let bump = (cur_max - res.scores[best]).max(0.0) + 0.05;
                    res.scores[best] += bump;
                }
            }
        }
        Some(res)
    }
}

fn open_eval_scorer(
    bundle_dir: Option<&Path>,
    lm_file: Option<&Path>,
    runtime_dir: &Path,
    tier: LmQuality,
    dict: Option<Arc<Dictionary>>,
) -> Result<(Box<dyn Scorer>, ScoreWeights, String)> {
    let tel: SharedTelemetry = Arc::new(Mutex::new(RuntimeTelemetry::default()));
    let dir = bundle_dir
        .or_else(|| lm_file.and_then(Path::parent))
        .unwrap_or_else(|| Path::new("."));
    let bundle = ModelBundlePaths::discover(dir, None, tier);
    let gpu_on = std::env::var("ASTERIA_LM_DEVICE").as_deref() != Ok("cpu");

    let use_defaults = std::env::var("ASTERIA_EVAL_NO_CONTEXT_DEFAULTS").as_deref() != Ok("1");
    let defaults: Option<Arc<ContextHistory>> = if use_defaults {
        let h = ContextHistory::in_memory();
        h.load_bundled_defaults();
        Some(Arc::new(h))
    } else {
        None
    };

    if bundle_dir.is_some()
        || (lm_file.is_some()
            && tier >= LmQuality::Standard
            && bundle.genmai.is_some()
            && bundle.tinyswallow.is_some())
    {
        let override_p = if bundle_dir.is_some() { None } else { lm_file };
        let mut scorer = MultiModelScorer::open(
            override_p,
            &bundle,
            runtime_dir,
            tier,
            gpu_on,
            false,
            Arc::clone(&tel),
        )
        .context("MultiModelScorer 初期化失敗")?;
        scorer.wait_judge_ready();
        if let Some(d) = dict {
            let validator: ReadingValidator =
                Arc::new(move |r, t| asteria_lattice::matches_reading(&d, r, t));
            scorer = scorer.with_reading_validator(validator);
        }
        let d_vp = defaults.clone();
        let vp: VariantProvider = Arc::new(move |r, ctx, t| {
            let mut out = Vec::new();
            if let Some(h) = &d_vp {
                out.extend(h.variants(r, ctx, t));
            }
            out.extend(asteria_lattice::counter_arabic_variants(t));
            out
        });
        scorer = scorer.with_variant_provider(vp);
        let weights = ScoreWeights::from_model(scorer.model()).unwrap_or_default();
        let desc = tel
            .lock()
            .map(|t| format!("{} ({} {})", t.active_models, t.device, t.gpu_name))
            .unwrap_or_default();
        return Ok((
            Box::new(EvalPipelineScorer {
                inner: Box::new(scorer),
                defaults,
            }),
            weights,
            desc,
        ));
    }

    let target = lm_file.context("--lm または --bundle が必要です")?;
    let info = asteria_lm::init_runtime(Some(runtime_dir), gpu_on, false);
    let scorer = ZenzScorer::open_compatible(target, THREADS)
        .with_context(|| format!("{} を LM として読めない", target.display()))?;
    let weights = ScoreWeights::from_model(scorer.model()).unwrap_or_default();
    let desc = format!(
        "{} (dyn={}, gpu={})",
        target.file_name().unwrap_or_default().to_string_lossy(),
        info.using_dynamic,
        info.gpu_name
    );
    Ok((
        Box::new(EvalPipelineScorer {
            inner: Box::new(scorer),
            defaults,
        }),
        weights,
        desc,
    ))
}

struct BoxedScorer(Box<dyn Scorer>);

impl Scorer for BoxedScorer {
    fn score(
        &mut self,
        request: &asteria_lm::rerank::RerankRequest,
        cancel: &asteria_lm::rerank::CancelToken,
    ) -> Option<Vec<f32>> {
        self.0.score(request, cancel)
    }

    fn score_and_generate(
        &mut self,
        request: &asteria_lm::rerank::RerankRequest,
        cancel: &asteria_lm::rerank::CancelToken,
    ) -> Option<asteria_lm::rerank::GenerationResult> {
        self.0.score_and_generate(request, cancel)
    }
}

fn tune(k: usize, dict: &str, model: &str, set: &str) -> Result<()> {
    use asteria_ime_eval::tune;
    let dict = load_dict(dict)?;
    let items = load_items(set)?;
    let scorer = ZenzScorer::open(Path::new(model), THREADS)
        .or_else(|_| ZenzScorer::open_by_vocab_hash(Path::new(model), THREADS))
        .with_context(|| format!("{model} を LM として読めない"))?;
    let mut scorer = scorer;
    let (scored, failures) = tune::collect(&dict, &items, &mut scorer, k);
    let results = tune::grid(&scored);
    println!(
        "問題 {} 件(採点できなかった問題 {failures} 件)、K={k}",
        scored.len()
    );
    println!(
        "| λ_lattice(T={}) | Acc@1 |\n| ---: | ---: |",
        tune::TEMPERATURE
    );
    for (w, acc) in &results {
        println!("| {} | {:.1}% |", w.lambda_lattice, acc * 100.0);
    }
    let Some((w, _)) = tune::best(&results) else {
        bail!("採点できた問題がない");
    };
    println!("{}", serde_json::to_string(&w)?);
    Ok(())
}

/// 計測で各要求を採点する回数。
const BENCH_ROUNDS: usize = 5;

fn load_items(path: &str) -> Result<Vec<asteria_ime_eval::eval::Item>> {
    let text = std::fs::read_to_string(path).with_context(|| format!("{path} を読めない"))?;
    serde_json::from_str(&text).with_context(|| format!("{path} の形式が不正"))
}

fn bench_lm(opts: &RunOptions, dict: &str, model: &str, set: &str) -> Result<()> {
    use asteria_ime_eval::bench;
    let dict = Arc::new(load_dict(dict)?);
    let requests = bench::requests(&dict, &load_items(set)?, opts.k.unwrap_or(DEFAULT_K));
    let model_p = Path::new(model);
    let runtime_dir = resolve_runtime_dir(opts.runtime_dir.as_deref(), Some(model_p));
    let tier = LmQuality::parse(opts.tier.as_deref().unwrap_or("low"));
    let (mut scorer, _, desc) = if model_p.is_dir() {
        open_eval_scorer(
            Some(model_p),
            None,
            &runtime_dir,
            tier,
            Some(Arc::clone(&dict)),
        )?
    } else {
        open_eval_scorer(
            None,
            Some(model_p),
            &runtime_dir,
            tier,
            Some(Arc::clone(&dict)),
        )?
    };
    let mut boxed = BoxedScorer(std::mem::replace(&mut scorer, Box::new(DummyScorer)));
    let samples = bench::measure(&mut boxed, &requests, BENCH_ROUNDS);
    let Some(l) = bench::summarize(&samples) else {
        bail!(
            "計測できる問題がない(読みが {} 文字以上の問題が要る)",
            bench::READING_CHARS
        );
    };
    let cands: usize = requests.iter().map(|r| r.candidates.len()).sum();
    println!(
        "問題 {} 件(候補 平均 {:.1} 件)× {BENCH_ROUNDS} 回、{desc}",
        requests.len(),
        cands as f64 / requests.len().max(1) as f64
    );
    println!("| 件数 | p50 | p95 | p99 | 最大 |\n| ---: | ---: | ---: | ---: | ---: |");
    println!(
        "| {} | {:.1}ms | {:.1}ms | {:.1}ms | {:.1}ms |",
        l.samples, l.p50, l.p95, l.p99, l.max
    );
    Ok(())
}

struct DummyScorer;
impl Scorer for DummyScorer {
    fn score(
        &mut self,
        _: &asteria_lm::rerank::RerankRequest,
        _: &asteria_lm::rerank::CancelToken,
    ) -> Option<Vec<f32>> {
        None
    }
}

fn load_dict(path: &str) -> Result<Dictionary> {
    Dictionary::open(Path::new(path)).with_context(|| format!("{path} は辞書として不正"))
}

fn run(opts: &RunOptions, dict: &str, out: &str, sets: &[String]) -> Result<()> {
    let dict = Arc::new(load_dict(dict)?);
    let k = opts.k.unwrap_or(DEFAULT_K);
    let mut header = String::new();
    let tier = LmQuality::parse(opts.tier.as_deref().unwrap_or("standard"));

    let mut rerank = if opts.bundle.is_some() || opts.lm.is_some() {
        let hint = opts.bundle.as_deref().or(opts.lm.as_deref()).map(Path::new);
        let runtime_dir = resolve_runtime_dir(opts.runtime_dir.as_deref(), hint);
        let (scorer, default_w, desc) = open_eval_scorer(
            opts.bundle.as_deref().map(Path::new),
            opts.lm.as_deref().map(Path::new),
            &runtime_dir,
            tier,
            Some(Arc::clone(&dict)),
        )?;
        let weights = match &opts.weights {
            Some(json) => ScoreWeights::from_json(json).context("スコア統合の重みが不正")?,
            None => default_w,
        };
        header = format!(
            "LM リランク: {desc}、K={k}、重み {}\n\n",
            serde_json::to_string(&weights)?
        );
        Some(Rerank {
            scorer: BoxedScorer(scorer),
            weights,
        })
    } else {
        None
    };

    let mut reports = Vec::new();
    for set in sets {
        let (name, path) = set
            .split_once('=')
            .with_context(|| format!("{set}: <名前>=<パス> の形で指定する"))?;
        let items = load_items(path)?;
        let reorder = rerank.as_mut().map(|r| (r as &mut dyn Reorder, k));
        reports.push(asteria_ime_eval::eval::evaluate(
            &dict, name, &items, reorder,
        ));
    }
    std::fs::create_dir_all(out)?;
    let md = header + &asteria_ime_eval::eval::to_markdown(&reports);
    std::fs::write(
        format!("{out}/report.json"),
        serde_json::to_string_pretty(&reports)?,
    )?;
    std::fs::write(format!("{out}/report.md"), &md)?;
    print!("{md}");
    Ok(())
}

fn hira_to_kata(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

fn find_eval_dict(args: &[String]) -> Result<PathBuf> {
    for a in args {
        if let Some(p) = a.strip_prefix("--dict=") {
            return Ok(PathBuf::from(p));
        }
    }
    if let Some(p) = std::env::var_os("ASTERIA_DICT") {
        return Ok(PathBuf::from(p));
    }
    let mut candidates = vec![
        PathBuf::from("target/asteria/system.dict"),
        PathBuf::from("dist/data/system.dict"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.insert(0, dir.join("data/system.dict"));
            candidates.insert(0, dir.join("system.dict"));
        }
    }
    candidates.into_iter().find(|p| p.is_file()).context(
        "評価用システム辞書が見つかりません (ASTERIA_DICT または --dict=<path> を指定してください)",
    )
}

/// `asteria-tools eval-baseline` / `converter-runner` 互換の標準入力変換ランナー。
fn run_converter_runner(args: &[String]) -> Result<()> {
    use asteria_session::converter::{Converter, LatticeConverter};

    let dict_path = find_eval_dict(args)?;
    let dict = Arc::new(load_dict(&dict_path.to_string_lossy())?);
    let data_dir = dict_path.parent();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let find_aux = |names: &[&str]| -> Option<PathBuf> {
        for base in [data_dir, exe_dir.as_deref()].into_iter().flatten() {
            for name in names {
                let p = base.join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    };

    let mut converter = LatticeConverter::new(Arc::clone(&dict));
    if let Some(col_path) = find_aux(&["fast_collocations.txt"]) {
        if let Ok(text) = std::fs::read_to_string(col_path) {
            converter = converter
                .with_collocations(Arc::new(asteria_dict::CollocationTable::parse_txt(&text)));
        }
    }
    if let Some(eng_path) = find_aux(&["AsteriaEnglish.tsv", "KotoriEnglish.tsv"]) {
        if let Ok(text) = std::fs::read_to_string(eng_path) {
            converter = converter
                .with_english_lexicon(Arc::new(asteria_dict::EnglishLexicon::parse_tsv(&text)));
        }
    }

    let mut ctx_map = std::collections::HashMap::<String, String>::new();
    if let Some(map_path) = std::env::var_os("ASTERIA_LM_CONTEXT_MAP") {
        if let Ok(text) = std::fs::read_to_string(map_path) {
            for line in text.lines() {
                if let Some((r, ctx)) = line.split_once('\t') {
                    ctx_map.insert(hira_to_kata(r.trim()), ctx.to_owned());
                }
            }
        }
    }

    let tier = LmQuality::parse(
        &std::env::var("ASTERIA_LM_QUALITY").unwrap_or_else(|_| "standard".to_owned()),
    );
    let mut lm_rerank = if let Some(bundle_dir) = std::env::var_os("ASTERIA_BUNDLE_DIR") {
        let p = PathBuf::from(bundle_dir);
        let runtime_dir = resolve_runtime_dir(None, Some(&p));
        open_eval_scorer(Some(&p), None, &runtime_dir, tier, Some(Arc::clone(&dict)))
            .ok()
            .map(|(scorer, weights, _)| Rerank {
                scorer: BoxedScorer(scorer),
                weights,
            })
    } else if let Some(lm_path) = std::env::var_os("ASTERIA_ZENZ_MODEL") {
        let p = PathBuf::from(lm_path);
        if p.exists() {
            let runtime_dir = resolve_runtime_dir(None, Some(&p));
            let res = if p.is_dir() {
                open_eval_scorer(Some(&p), None, &runtime_dir, tier, Some(Arc::clone(&dict)))
            } else {
                open_eval_scorer(None, Some(&p), &runtime_dir, tier, Some(Arc::clone(&dict)))
            };
            res.ok().map(|(scorer, weights, _)| Rerank {
                scorer: BoxedScorer(scorer),
                weights,
            })
        } else {
            None
        }
    } else {
        None
    };

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if let Some(reading) = trimmed.strip_prefix("start ") {
            let kata = hira_to_kata(reading.trim());
            let mut segs = converter.convert(&kata);
            if let Some(ref mut r) = lm_rerank {
                let hyps = converter.sentences(&kata, DEFAULT_K);
                if hyps.len() >= 2 {
                    let cands: Vec<(String, i64)> =
                        hyps.iter().map(|s| (s.surface(), s.cost)).collect();
                    let item = asteria_ime_eval::eval::Item {
                        index: String::new(),
                        context_text: ctx_map.get(&kata).cloned().unwrap_or_default(),
                        input: kata.clone(),
                        expected_output: Vec::new(),
                    };
                    if let Some(reordered) = r.reorder_surfaces(&item, &cands) {
                        if let Some((best_surf, _)) = reordered.first() {
                            if best_surf != &cands[0].0 {
                                if let Some(hyp) = hyps.iter().find(|h| h.surface() == *best_surf) {
                                    segs = hyp
                                        .segments
                                        .iter()
                                        .map(|(len, surf)| {
                                            asteria_session::converter::SegmentCandidates {
                                                len: *len,
                                                candidates: vec![surf.clone()],
                                            }
                                        })
                                        .collect();
                                } else {
                                    segs = vec![asteria_session::converter::SegmentCandidates {
                                        len: kata.chars().count(),
                                        candidates: vec![best_surf.clone()],
                                    }];
                                }
                            }
                        }
                    }
                }
            }
            let seg_total = segs.len();
            for (idx, seg) in segs.iter().enumerate() {
                writeln!(
                    out,
                    "---------- Segment {idx}/{seg_total} [CONVERSION] ----------"
                )?;
                let cand_total = seg.candidates.len();
                for (rank, cand) in seg.candidates.iter().enumerate() {
                    if !cand.is_empty() {
                        writeln!(out, "  {rank}/{cand_total} {cand}")?;
                    }
                }
            }
            writeln!(out, "ExecCommand() return false")?;
            out.flush()?;
        }
    }
    Ok(())
}

fn repl(path: &str) -> Result<()> {
    let dict = load_dict(path)?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    write!(stdout, "> ")?;
    stdout.flush()?;
    for line in stdin.lock().lines() {
        let line = line?;
        write!(stdout, "{}> ", asteria_ime_eval::render(&dict, &line, 10))?;
        stdout.flush()?;
    }
    writeln!(stdout)?;
    Ok(())
}
