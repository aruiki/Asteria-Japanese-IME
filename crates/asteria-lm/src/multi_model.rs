//! マルチモデル推論コーディネータ (`zenz` / `genmai` / `tinyswallow` / `Swallow-8B` 判定役) と推論統計共有。
//!
//! `ADR 0075 / 0090`:
//! - 2 段階モデルロード: `primary` (`genmai-310M` / `zenz`) 読込完了で即座に利用可能にし、
//!   `judge` (`TinySwallow-1.5B` 等) はバックグラウンドスレッドで遅れて結合する。
//! - 先行並列採点 (`prescore`): `genmai` が `DraftedBeamSearch` を行う裏で、`JudgeWorker` が
//!   ラティス上位候補を同時並行で採点し、待ち時間を隠蔽する。
//! - 精度保護: 読み整合・途中切れ防止 (`PrefixNormalizedTop`)・確信度連動 LLM 重み
//!   (`confidence_aware_llm_weight`)・文脈コピー抑止 (`[copy]`)。

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use crate::catalog::{LmQuality, ModelBundlePaths};
use crate::llama_dll::{init_runtime_backend, GpuDeviceInfo, GpuKind};
use crate::rerank::{CancelToken, GenerationResult, RerankRequest, Scorer};
use crate::zenz::{LlmScorer, ZenzScorer};
use crate::{LmError, Model};

const DEFAULT_THREADS: i32 = 4;
const MAX_CANDS: usize = 32;
const MAX_GENERATE_CHARS: usize = 120;
const LAMBDA_LM: f32 = 1.0;
const LAMBDA_LLM: f32 = 1.2;
const SWAP_LLM_WEIGHT: f32 = 2.0;
const GEN_PENALTY: f32 = 0.5;
const EARLY_EXIT_Z_MARGIN_CPU: f32 = 8.0;
const EARLY_EXIT_Z_MARGIN_GPU: f32 = 16.0;
/// `ln(0.5)`: プライマリ LM (`genmai`/`zenz`) の事後確率が過半数の基準。
const MAJORITY_LOGP: f32 = -std::f32::consts::LN_2;

/// 生成・表記展開候補が入力読みと矛盾しないか検証するコールバック `(reading, candidate) -> bool`。
pub type ReadingValidator = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;

/// 同音異字・表記候補を追加展開するコールバック `(reading, left_context, base_text) -> Vec<String>`。
pub type VariantProvider = Arc<dyn Fn(&str, &str, &str) -> Vec<String> + Send + Sync>;

/// サーバー `AiRuntimeStatus` へ共有するライブ推論統計。
#[derive(Debug, Clone, Default)]
pub struct RuntimeTelemetry {
    pub state: String,
    pub active_models: String,
    pub device: String,
    pub gpu_name: String,
    pub vram_gb: f64,
    pub infer_calls: u64,
    pub infer_ms: f64,
}

pub type SharedTelemetry = Arc<Mutex<RuntimeTelemetry>>;

fn is_kanji(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c) || c == '々'
}

fn ends_with_closing_punct(c: char) -> bool {
    matches!(
        c,
        '、' | '。' | '！' | '？' | '!' | '?' | '（' | '）' | '「' | '」' | '『' | '』' | '・'
    )
}

/// 2 つの候補が「かな骨格は同じで漢字のみを入れ替えた同音語対（解放/開放、家事/火事）」か判定する。
pub fn is_kanji_swap(a: &str, b: &str) -> bool {
    if a == b {
        return false;
    }
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let mut has_diff_kanji = false;
    let mut ai = 0usize;
    let mut bi = 0usize;
    while ai < ac.len() && bi < bc.len() {
        if ac[ai] == bc[bi] {
            ai += 1;
            bi += 1;
            continue;
        }
        let mut ak = false;
        while ai < ac.len() && is_kanji(ac[ai]) {
            ak = true;
            ai += 1;
        }
        let mut bk = false;
        while bi < bc.len() && is_kanji(bc[bi]) {
            bk = true;
            bi += 1;
        }
        if !ak || !bk {
            return false;
        }
        has_diff_kanji = true;
    }
    has_diff_kanji && ai == ac.len() && bi == bc.len()
}

/// 2 文の共通接頭辞・共通接尾辞を除いた中央差分 `(span_a, span_b)` を返す。
pub fn diff_spans(a: &str, b: &str) -> (String, String) {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let mut head = 0usize;
    while head < ac.len() && head < bc.len() && ac[head] == bc[head] {
        head += 1;
    }
    let mut tail = 0usize;
    while tail < ac.len() - head
        && tail < bc.len() - head
        && ac[ac.len() - 1 - tail] == bc[bc.len() - 1 - tail]
    {
        tail += 1;
    }
    (
        ac[head..ac.len() - tail].iter().collect(),
        bc[head..bc.len() - tail].iter().collect(),
    )
}

/// 生成候補が長文途中で切れている・末尾モーラ/記号を落としているか検査する。
pub fn looks_truncated_or_broken(reading: &str, lat0: &str, candidate: &str) -> bool {
    if candidate.is_empty() {
        return true;
    }
    let r_chars: Vec<char> = reading.chars().collect();
    let g_chars = candidate.chars().count();
    let l_chars = lat0.chars().count();
    if r_chars.len() >= 12 && l_chars > 0 && g_chars * 100 < l_chars * 60 {
        return true;
    }
    if let (Some(r_last), Some(g_last)) = (r_chars.last().copied(), candidate.chars().last()) {
        let l_last = lat0.chars().last();
        if ends_with_closing_punct(r_last) && l_last == Some(r_last) && g_last != r_last {
            return true;
        }
        // 「モクロン → 目論」（末尾「ん」欠落）のような語尾切れを弾く。
        if matches!(r_last, 'ん' | 'ン')
            && matches!(l_last, Some('ん' | 'ン'))
            && !matches!(g_last, 'ん' | 'ン')
            && is_kanji(g_last)
        {
            return true;
        }
    }
    false
}

/// 文脈なし短文でプライマリ LM (`genmai`/`zenz`) が過半数の確信を持つとき、
/// 読みを見ない LLM による過剰上書きを防ぐ（`ADR 0075`）。
pub fn confidence_aware_llm_weight(zlogp: &[f32], configured: f32, has_left_context: bool) -> f32 {
    if has_left_context {
        return configured;
    }
    if zlogp.iter().any(|&s| s.is_finite() && s >= MAJORITY_LOGP) {
        return configured.min(0.7);
    }
    configured
}

/// 選択された `top` 候補が別候補の厳密な先頭部分文字列である場合、
/// 生対数尤度和ではなくトークン平均スコアで比較し直す（`ADR 0075`）。
pub fn prefix_normalized_top(
    candidates: &[String],
    scores: &[f32],
    token_counts: &[usize],
    top: usize,
) -> usize {
    if top >= candidates.len()
        || scores.len() != candidates.len()
        || token_counts.len() != candidates.len()
        || candidates[top].is_empty()
        || token_counts[top] == 0
        || !scores[top].is_finite()
    {
        return top;
    }
    let mut best = top;
    let mut best_avg = scores[top] / token_counts[top] as f32;
    for i in 0..candidates.len() {
        if token_counts[i] == 0
            || !scores[i].is_finite()
            || candidates[i].len() <= candidates[top].len()
            || !candidates[i].starts_with(&candidates[top])
        {
            continue;
        }
        let avg = scores[i] / token_counts[i] as f32;
        if avg > best_avg {
            best = i;
            best_avg = avg;
        }
    }
    best
}

enum JudgeJob {
    Prewarm {
        left_context: String,
    },
    Score {
        left_context: String,
        candidates: Vec<String>,
        cancel: CancelToken,
        reply: mpsc::Sender<Option<Vec<f32>>>,
    },
    ScoreIsolated {
        candidates: Vec<String>,
        cancel: CancelToken,
        reply: mpsc::Sender<Option<Vec<f32>>>,
    },
    CountTokens {
        candidates: Vec<String>,
        reply: mpsc::Sender<Vec<usize>>,
    },
}

/// 2 段階ロード＆先行並列採点 (`prescore`) を担う汎用 LLM ワーカースレッド。
struct JudgeWorker {
    ready: Arc<AtomicBool>,
    done_init: Arc<(Mutex<bool>, Condvar)>,
    tx: Option<mpsc::Sender<JudgeJob>>,
    handle: Option<JoinHandle<()>>,
}

impl JudgeWorker {
    fn spawn(path: PathBuf, telemetry: SharedTelemetry) -> Self {
        let ready = Arc::new(AtomicBool::new(false));
        let done_init = Arc::new((Mutex::new(false), Condvar::new()));
        let (tx, rx) = mpsc::channel::<JudgeJob>();

        let r_flag = Arc::clone(&ready);
        let d_pair = Arc::clone(&done_init);
        let stem = file_stem_name(&path);

        let handle = std::thread::spawn(move || {
            let opened = LlmScorer::open(&path, DEFAULT_THREADS).ok();
            let is_ok = opened.is_some();
            if is_ok {
                r_flag.store(true, AtomicOrdering::Release);
                if let Ok(mut t) = telemetry.lock() {
                    if !t.active_models.contains(&stem) {
                        t.active_models = if t.active_models.is_empty() {
                            stem
                        } else {
                            format!("{} + {stem}", t.active_models)
                        };
                    }
                }
            }
            {
                let (lock, cv) = &*d_pair;
                if let Ok(mut done) = lock.lock() {
                    *done = true;
                    cv.notify_all();
                }
            }
            let Some(mut scorer) = opened else {
                return;
            };
            for job in rx {
                match job {
                    JudgeJob::Prewarm { left_context } => {
                        scorer.prewarm(&left_context);
                    }
                    JudgeJob::Score {
                        left_context,
                        candidates,
                        cancel,
                        reply,
                    } => {
                        let res = if cancel.is_cancelled() {
                            None
                        } else {
                            scorer
                                .score_all(&left_context, &candidates, Some(&cancel))
                                .ok()
                                .flatten()
                        };
                        let _ = reply.send(res);
                    }
                    JudgeJob::ScoreIsolated {
                        candidates,
                        cancel,
                        reply,
                    } => {
                        let res = if cancel.is_cancelled() {
                            None
                        } else {
                            scorer
                                .score_isolated(&candidates, Some(&cancel))
                                .ok()
                                .flatten()
                        };
                        let _ = reply.send(res);
                    }
                    JudgeJob::CountTokens { candidates, reply } => {
                        let counts = candidates.iter().map(|c| scorer.count_tokens(c)).collect();
                        let _ = reply.send(counts);
                    }
                }
            }
        });

        Self {
            ready,
            done_init,
            tx: Some(tx),
            handle: Some(handle),
        }
    }

    fn is_ready(&self) -> bool {
        self.ready.load(AtomicOrdering::Acquire)
    }

    fn wait_init(&self) {
        let (lock, cv) = &*self.done_init;
        if let Ok(mut done) = lock.lock() {
            while !*done {
                done = match cv.wait(done) {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                };
            }
        }
    }

    fn prewarm(&self, left_context: String) {
        if self.is_ready() {
            if let Some(tx) = &self.tx {
                let _ = tx.send(JudgeJob::Prewarm { left_context });
            }
        }
    }

    fn submit(
        &self,
        left_context: String,
        candidates: Vec<String>,
        cancel: CancelToken,
    ) -> Option<mpsc::Receiver<Option<Vec<f32>>>> {
        if !self.is_ready() || candidates.is_empty() {
            return None;
        }
        let (reply, rx) = mpsc::channel();
        self.tx
            .as_ref()?
            .send(JudgeJob::Score {
                left_context,
                candidates,
                cancel,
                reply,
            })
            .ok()?;
        Some(rx)
    }

    fn score_isolated(&self, candidates: Vec<String>, cancel: CancelToken) -> Option<Vec<f32>> {
        if !self.is_ready() || candidates.is_empty() {
            return None;
        }
        let (reply, rx) = mpsc::channel();
        self.tx
            .as_ref()?
            .send(JudgeJob::ScoreIsolated {
                candidates,
                cancel,
                reply,
            })
            .ok()?;
        rx.recv().ok().flatten()
    }

    fn count_tokens(&self, candidates: Vec<String>) -> Option<Vec<usize>> {
        if !self.is_ready() || candidates.is_empty() {
            return None;
        }
        let (reply, rx) = mpsc::channel();
        self.tx
            .as_ref()?
            .send(JudgeJob::CountTokens { candidates, reply })
            .ok()?;
        rx.recv().ok()
    }
}

impl Drop for JudgeWorker {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// 品質階層と導入済み GGUF バンドルに応じて複数モデルを統合するスコアラー＆候補生成器。
pub struct MultiModelScorer {
    primary: ZenzScorer,
    judge: Option<JudgeWorker>,
    tier: LmQuality,
    has_gpu: bool,
    telemetry: SharedTelemetry,
    validator: Option<ReadingValidator>,
    variants: Option<VariantProvider>,
}

impl MultiModelScorer {
    /// GGUF 群と実行時 GPU バックエンド (`llama.dll` / `cuda\ggml-cuda.dll`) を初期化し、
    /// `primary` 読込後に即座に返る (2 段階ロード、`judge` はバックグラウンドで起動)。
    pub fn open(
        override_model: Option<&Path>,
        bundle: &ModelBundlePaths,
        runtime_dir: &Path,
        tier: LmQuality,
        gpu_enabled: bool,
        allow_integrated: bool,
        telemetry: SharedTelemetry,
    ) -> Result<Self, LmError> {
        let (backend, gpu): (_, GpuDeviceInfo) =
            init_runtime_backend(runtime_dir, gpu_enabled, allow_integrated);

        let primary_path = override_model
            .map(Path::to_path_buf)
            .or_else(|| {
                if tier <= LmQuality::Low {
                    bundle.zenz_small.clone().or_else(|| bundle.genmai.clone())
                } else {
                    bundle.genmai.clone().or_else(|| bundle.zenz_small.clone())
                }
            })
            .or_else(|| bundle.native_generator.clone())
            .ok_or_else(|| LmError::Load("利用可能な GGUF モデルがありません".to_owned()))?;

        let primary = ZenzScorer::open_compatible(&primary_path, DEFAULT_THREADS)?;

        let judge_path = if tier >= LmQuality::High {
            bundle.judge.as_ref().or(bundle.tinyswallow.as_ref())
        } else if tier >= LmQuality::Standard {
            bundle.tinyswallow.as_ref()
        } else {
            None
        }
        .filter(|p| p.as_path() != primary_path.as_path())
        .cloned();

        if let Ok(mut t) = telemetry.lock() {
            t.state = "ready".to_owned();
            t.active_models = file_stem_name(&primary_path);
            t.device = match gpu.kind {
                GpuKind::NoneCpuOnly => {
                    if backend.using_dynamic {
                        "CPU (AVX2)".to_owned()
                    } else {
                        "CPU".to_owned()
                    }
                }
                GpuKind::Discrete | GpuKind::Integrated => {
                    if backend.is_cuda {
                        "GPU (CUDA)".to_owned()
                    } else if backend.using_dynamic {
                        "GPU (Vulkan)".to_owned()
                    } else {
                        "GPU".to_owned()
                    }
                }
            };
            t.gpu_name = gpu.name;
            t.vram_gb = gpu.total_vram_bytes as f64 / (1u64 << 30) as f64;
        }

        let judge = judge_path.map(|p| JudgeWorker::spawn(p, Arc::clone(&telemetry)));

        Ok(Self {
            primary,
            judge,
            tier,
            has_gpu: backend.has_gpu,
            telemetry,
            validator: None,
            variants: None,
        })
    }

    /// 生成・展開候補の読み整合検証器を設定する（別読み漢字すり替え・途中切れ防止）。
    pub fn with_reading_validator(mut self, validator: ReadingValidator) -> Self {
        self.validator = Some(validator);
        self
    }

    /// 同音異字・学習済み表記候補の展開プロバイダを設定する。
    pub fn with_variant_provider(mut self, provider: VariantProvider) -> Self {
        self.variants = Some(provider);
        self
    }

    /// 評価ツール向け: バックグラウンドの `judge` (`TinySwallow-1.5B` 等) 読込完了を待機する。
    pub fn wait_judge_ready(&self) {
        if let Some(j) = &self.judge {
            j.wait_init();
        }
    }

    pub fn model(&self) -> &Model {
        self.primary.model()
    }

    fn is_valid_generated(&self, reading: &str, lat0: &str, cand: &str) -> bool {
        if looks_truncated_or_broken(reading, lat0, cand) {
            return false;
        }
        self.validator
            .as_ref()
            .map_or(true, |check| check(reading, cand))
    }

    fn count_tokens_for(&self, candidates: &[String]) -> Vec<usize> {
        if let Some(counts) = self
            .judge
            .as_ref()
            .and_then(|j| j.count_tokens(candidates.to_vec()))
        {
            if counts.len() == candidates.len() {
                return counts;
            }
        }
        candidates
            .iter()
            .map(|c| {
                self.primary
                    .model()
                    .tokenize(c, false)
                    .map_or(1, |t| t.len().max(1))
            })
            .collect()
    }
}

fn file_stem_name(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl Scorer for MultiModelScorer {
    fn prewarm(&mut self, left_context: &str, reading: &str) {
        self.primary.prewarm(left_context, reading);
        if let Some(judge) = &self.judge {
            judge.prewarm(left_context.to_owned());
        }
    }

    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>> {
        let mut req_no_gen = request.clone();
        let orig_len = req_no_gen.candidates.len();
        req_no_gen.candidates.truncate(MAX_CANDS);
        let mut res = self.score_impl(&req_no_gen, false, cancel)?;
        res.scores.truncate(orig_len);
        while res.scores.len() < orig_len {
            res.scores.push(-1000.0);
        }
        Some(res.scores)
    }

    fn score_and_generate(
        &mut self,
        request: &RerankRequest,
        cancel: &CancelToken,
    ) -> Option<GenerationResult> {
        self.score_impl(request, true, cancel)
    }
}

impl MultiModelScorer {
    fn score_impl(
        &mut self,
        request: &RerankRequest,
        allow_gen: bool,
        cancel: &CancelToken,
    ) -> Option<GenerationResult> {
        let start = Instant::now();
        let params = self.tier.params();
        let lattice_cap = params.k.clamp(4, MAX_CANDS);

        let mut candidates: Vec<String> = request
            .candidates
            .iter()
            .take(lattice_cap)
            .cloned()
            .collect();
        let mut is_generated = vec![false; candidates.len()];
        let mut is_variant = vec![false; candidates.len()];

        let reading_chars = request.reading.chars().count();
        let do_gen = allow_gen
            && params.beams > 0
            && self.tier >= LmQuality::Standard
            && (self.has_gpu || reading_chars <= 60)
            && !request.reading.is_empty()
            && reading_chars <= MAX_GENERATE_CHARS
            && !cancel.is_cancelled();

        // 0) genmai 生成の裏で LLM がラティス上位候補を先行並列採点 (`prescore`, ADR 0090)
        let prescore_on = std::env::var("ASTERIA_LM_PRESCORE").as_deref() != Ok("0")
            && std::env::var("KOTORI_LM_PRESCORE").as_deref() != Ok("0");
        let prescore_list: Vec<String> =
            if do_gen && prescore_on && candidates.len() >= 2 && params.llm_top > 0 {
                candidates.iter().take(params.llm_top).cloned().collect()
            } else {
                Vec::new()
            };
        let prescore_rx = if !prescore_list.is_empty() {
            self.judge.as_ref().and_then(|j| {
                j.submit(
                    request.left_context.clone(),
                    prescore_list.clone(),
                    cancel.clone(),
                )
            })
        } else {
            None
        };

        // 1) genmai / zenz による下書き付きビーム候補生成 (`DraftedBeamSearch`) + 整合ガード
        let lat0 = request.candidates.first().cloned().unwrap_or_default();
        if do_gen {
            let gen_beams = if self.has_gpu {
                params.beams.min(8)
            } else {
                params.beams.min(4)
            };
            let draft_surfs: Vec<String> = request.candidates.iter().take(4).cloned().collect();
            let generated = self.primary.generate_with_drafts(
                &request.left_context,
                &request.reading,
                &draft_surfs,
                gen_beams,
                Some(cancel),
            );
            for g in generated {
                if candidates.len() >= MAX_CANDS {
                    break;
                }
                if candidates.contains(&g) || !self.is_valid_generated(&request.reading, &lat0, &g)
                {
                    continue;
                }
                candidates.push(g);
                is_generated.push(true);
                is_variant.push(false);
            }
        }

        // 1b) ContextHistory 等による同音異字・学習済みバリエーション展開（減点なし）
        if allow_gen {
            if let Some(vp) = self.variants.clone() {
                let base_n = candidates.len().min(6);
                let mut added = 0usize;
                for i in 0..base_n {
                    if added >= 6 || candidates.len() >= MAX_CANDS {
                        break;
                    }
                    let vars = vp(&request.reading, &request.left_context, &candidates[i]);
                    for v in vars {
                        if added >= 6 || candidates.len() >= MAX_CANDS {
                            break;
                        }
                        if candidates.contains(&v)
                            || !self.is_valid_generated(&request.reading, &lat0, &v)
                        {
                            continue;
                        }
                        candidates.push(v);
                        is_generated.push(true);
                        is_variant.push(true);
                        added += 1;
                    }
                }
            }
        }

        if candidates.is_empty() || cancel.is_cancelled() {
            return None;
        }

        // 2) プライマリ LM (`zenz` / `genmai`) で全候補を一括採点
        let zlogp = self
            .primary
            .score_all(
                &request.left_context,
                &request.reading,
                &candidates,
                Some(cancel),
            )
            .ok()
            .flatten()?;

        if cancel.is_cancelled() {
            return None;
        }

        // 先行並列採点結果の回収
        let mut cached_llm: HashMap<String, f32> = HashMap::new();
        if let Some(rx) = prescore_rx {
            if let Ok(Some(scores)) = rx.recv() {
                for (c, s) in prescore_list.into_iter().zip(scores) {
                    cached_llm.insert(c, s);
                }
            }
        }

        // 3) LLM Judge 統合 (未採点の生成/展開候補・同音対・近接候補を追加採点)
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by(|&a, &b| zlogp[b].partial_cmp(&zlogp[a]).unwrap_or(Ordering::Equal));

        let top_gap = if order.len() >= 2 {
            (zlogp[order[0]] - zlogp[order[1]]).abs()
        } else {
            f32::INFINITY
        };
        let has_swap = order.len() >= 2
            && order[1..]
                .iter()
                .take(params.llm_top + 2)
                .any(|&idx| is_kanji_swap(&candidates[order[0]], &candidates[idx]));

        let z_margin = if self.has_gpu {
            EARLY_EXIT_Z_MARGIN_GPU
        } else {
            EARLY_EXIT_Z_MARGIN_CPU
        };
        let should_run_judge = self.judge.as_ref().is_some_and(JudgeWorker::is_ready)
            && candidates.len() >= 2
            && params.llm_top > 0
            && (has_swap
                || !cached_llm.is_empty()
                || top_gap <= z_margin
                || (!request.left_context.is_empty() && top_gap <= z_margin + 4.0));

        let mut joint_scores = vec![0.0f32; candidates.len()];
        let mut includes_llm = false;

        if should_run_judge {
            let mut eval_indices: Vec<usize> = order.iter().take(params.llm_top).copied().collect();
            if !is_generated[0] && !eval_indices.contains(&0) {
                eval_indices.push(0);
            }
            for (i, &v) in is_variant.iter().enumerate() {
                if v && !eval_indices.contains(&i) && eval_indices.len() < MAX_CANDS {
                    eval_indices.push(i);
                }
            }
            let mut nearby_added = 0usize;
            for &idx in &order {
                if nearby_added >= 2 {
                    break;
                }
                if !is_generated[idx] && !eval_indices.contains(&idx) {
                    eval_indices.push(idx);
                    nearby_added += 1;
                }
            }
            for &idx in &order {
                if eval_indices.len() >= MAX_CANDS {
                    break;
                }
                if is_kanji_swap(&candidates[order[0]], &candidates[idx])
                    && !eval_indices.contains(&idx)
                {
                    eval_indices.push(idx);
                }
            }

            let missing: Vec<String> = eval_indices
                .iter()
                .map(|&idx| candidates[idx].clone())
                .filter(|c| !cached_llm.contains_key(c))
                .collect();
            let mut judge_ok = !cached_llm.is_empty();
            if !missing.is_empty() {
                if let Some(rx) = self.judge.as_ref().and_then(|j| {
                    j.submit(
                        request.left_context.clone(),
                        missing.clone(),
                        cancel.clone(),
                    )
                }) {
                    if let Ok(Some(sub_scores)) = rx.recv() {
                        judge_ok = true;
                        for (c, s) in missing.into_iter().zip(sub_scores) {
                            cached_llm.insert(c, s);
                        }
                    }
                }
            }

            if judge_ok {
                includes_llm = true;
                let has_ctx = !request.left_context.is_empty();
                let ranking_llm_weight = confidence_aware_llm_weight(&zlogp, LAMBDA_LLM, has_ctx);
                let swap_llm_weight = confidence_aware_llm_weight(&zlogp, SWAP_LLM_WEIGHT, has_ctx);

                let mut llogp_full = vec![f32::NEG_INFINITY; candidates.len()];
                for &idx in &eval_indices {
                    if let Some(&val) = cached_llm.get(&candidates[idx]) {
                        llogp_full[idx] = val;
                    }
                }
                let mut min_finite = 0.0f32;
                for &v in &llogp_full {
                    if v.is_finite() && v < min_finite {
                        min_finite = v;
                    }
                }
                let fallback_l = min_finite - 50.0;
                for i in 0..candidates.len() {
                    let l_val = if llogp_full[i].is_finite() {
                        llogp_full[i]
                    } else {
                        fallback_l
                    };
                    let gen_pen = if is_generated[i] && !is_variant[i] {
                        GEN_PENALTY
                    } else {
                        0.0
                    };
                    joint_scores[i] = LAMBDA_LM * zlogp[i] + ranking_llm_weight * l_val - gen_pen;
                }

                if let Some((mut best_idx, _)) = joint_scores
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(Ordering::Equal))
                {
                    if llogp_full[best_idx].is_finite() {
                        let extra_w = (swap_llm_weight - ranking_llm_weight).max(0.0);
                        if extra_w > 0.0 {
                            let mut swapped_best = best_idx;
                            let mut swapped_score =
                                joint_scores[best_idx] + extra_w * llogp_full[best_idx];
                            for i in 0..candidates.len() {
                                if i != best_idx
                                    && llogp_full[i].is_finite()
                                    && is_kanji_swap(&candidates[best_idx], &candidates[i])
                                {
                                    let sc = joint_scores[i] + extra_w * llogp_full[i];
                                    if sc > swapped_score {
                                        swapped_score = sc;
                                        swapped_best = i;
                                    }
                                }
                            }
                            if swapped_best != best_idx {
                                let delta = (joint_scores[best_idx] - joint_scores[swapped_best])
                                    .max(0.0)
                                    + 0.05;
                                joint_scores[swapped_best] += delta;
                                best_idx = swapped_best;
                            }
                        }
                    }

                    // [copy] 前の文脈の語を不自然に反復したときは文脈なし採点で比べ直す (ADR 0075)
                    if has_ctx && llogp_full[best_idx].is_finite() {
                        let mut copy_alt: Option<usize> = None;
                        for i in 0..candidates.len() {
                            if i == best_idx
                                || !llogp_full[i].is_finite()
                                || !is_kanji_swap(&candidates[best_idx], &candidates[i])
                            {
                                continue;
                            }
                            let (x, y) = diff_spans(&candidates[best_idx], &candidates[i]);
                            if x.chars().count() >= 2
                                && request.left_context.contains(&x)
                                && !request.left_context.contains(&y)
                                && copy_alt.map_or(true, |cur| joint_scores[i] > joint_scores[cur])
                            {
                                copy_alt = Some(i);
                            }
                        }
                        if let Some(alt_idx) = copy_alt {
                            if let Some(free) = self.judge.as_ref().and_then(|j| {
                                j.score_isolated(
                                    vec![candidates[best_idx].clone(), candidates[alt_idx].clone()],
                                    cancel.clone(),
                                )
                            }) {
                                if free.len() == 2 && free[0].is_finite() && free[1].is_finite() {
                                    let s_top = joint_scores[best_idx]
                                        + ranking_llm_weight * (free[0] - llogp_full[best_idx]);
                                    let s_alt = joint_scores[alt_idx]
                                        + ranking_llm_weight * (free[1] - llogp_full[alt_idx]);
                                    if s_alt > s_top {
                                        let offset = joint_scores[best_idx] - s_top;
                                        joint_scores[alt_idx] = s_alt + offset + 0.05;
                                        best_idx = alt_idx;
                                    }
                                }
                            }
                        }
                    }

                    // [prefix_boundary] 部分一致の途中切れを平均尤度で正規化比較 (ADR 0075)
                    let has_extension = candidates.iter().enumerate().any(|(i, c)| {
                        c.len() > candidates[best_idx].len()
                            && c.starts_with(&candidates[best_idx])
                            && joint_scores[i].is_finite()
                    });
                    if has_extension {
                        let counts = self.count_tokens_for(&candidates);
                        let norm_best =
                            prefix_normalized_top(&candidates, &joint_scores, &counts, best_idx);
                        if norm_best != best_idx {
                            let bump =
                                (joint_scores[best_idx] - joint_scores[norm_best]).max(0.0) + 0.05;
                            joint_scores[norm_best] += bump;
                        }
                    }
                }
            }
        }

        if !includes_llm {
            for i in 0..candidates.len() {
                let gen_pen = if is_generated[i] && !is_variant[i] {
                    GEN_PENALTY
                } else {
                    0.0
                };
                joint_scores[i] = LAMBDA_LM * zlogp[i] - gen_pen;
            }
        }

        // ラティスの未評価候補 (`lattice_cap..`) も末尾に残し、Acc@10 候補窓を削らない。
        if allow_gen && request.candidates.len() > lattice_cap {
            let min_s = joint_scores.iter().copied().fold(0.0f32, f32::min);
            for (offset, extra) in request.candidates.iter().skip(lattice_cap).enumerate() {
                if !candidates.contains(extra) {
                    candidates.push(extra.clone());
                    is_generated.push(false);
                    joint_scores.push(min_s - 100.0 - offset as f32 * 0.01);
                }
            }
        }

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        if let Ok(mut t) = self.telemetry.lock() {
            t.infer_calls += 1;
            t.infer_ms += elapsed_ms;
        }

        Some(GenerationResult {
            candidates,
            is_generated,
            scores: joint_scores,
            includes_llm,
            pure_lm_ranking: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_bundle_returns_load_error_and_does_not_panic() {
        let tel: SharedTelemetry = Arc::new(Mutex::new(RuntimeTelemetry::default()));
        let bundle = ModelBundlePaths::default();
        let res = MultiModelScorer::open(
            None,
            &bundle,
            Path::new("."),
            LmQuality::Standard,
            true,
            false,
            tel,
        );
        assert!(matches!(res, Err(LmError::Load(_))));
    }

    #[test]
    fn kanji_swap_detects_homophone_pairs_only() {
        assert!(is_kanji_swap("資源の解放を行う", "資源の開放を行う"));
        assert!(!is_kanji_swap("食道がんの予防", "食道癌の予防"));
        assert!(!is_kanji_swap("今日", "今日"));
    }

    #[test]
    fn confidence_guard_clamps_without_context_only() {
        assert!((confidence_aware_llm_weight(&[-0.2, -2.0], 2.0, false) - 0.7).abs() < 1e-6);
        assert!((confidence_aware_llm_weight(&[-0.2, -2.0], 2.0, true) - 2.0).abs() < 1e-6);
        assert!((confidence_aware_llm_weight(&[-2.0, -3.0], 2.0, false) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn truncation_and_tail_mora_guard_rejects_broken_candidates() {
        assert!(looks_truncated_or_broken(
            "アイウエオカキクケコサシスセソタチ",
            "あいうえおかきくけこさしすせそたち",
            "あいうえお"
        ));
        assert!(looks_truncated_or_broken("モクロン", "目論ん", "目論"));
        assert!(!looks_truncated_or_broken("モクロン", "目論ん", "目論ん"));
        assert!(looks_truncated_or_broken(
            "キョウハ。",
            "今日は。",
            "今日は"
        ));
    }

    #[test]
    fn prefix_normalization_prefers_complete_extension_when_per_token_is_higher() {
        let cands = vec![
            "責任を負わないが".to_owned(),
            "責任を負わないが完全な文".to_owned(),
        ];
        let scores = vec![-12.0, -14.0];
        let counts = vec![4, 8];
        assert_eq!(prefix_normalized_top(&cands, &scores, &counts, 0), 1);
    }

    #[test]
    fn diff_spans_extracts_changing_middle_segment() {
        let (a, b) = diff_spans("日本語を開設する", "日本語を解説する");
        assert_eq!(a, "開設");
        assert_eq!(b, "解説");
    }
}
