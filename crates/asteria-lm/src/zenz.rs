//! zenz / genmai による候補の採点・ビーム生成および汎用 LLM 判定採点。

use std::path::Path;
use std::rc::Rc;

use crate::rerank::{CancelToken, RerankRequest, Scorer};
use crate::{Context, LmError, Model};

/// training/zenz/convert.py が書き出す zenz-v2.5 の語彙の SHA-256。
pub const VOCAB_HASH: &str = "0338c82fbd5ffaa49603733f0b14500a572abb3881c8e19beeb327cae3124408";

/// 左文脈の最大文字数。Zenzai(zenz-v2 の学習元)の既定値に合わせる。
pub const MAX_CONTEXT_CHARS: usize = 40;
/// 汎用 LLM の左文脈最大バイト数 (40文字相当以上)。
const MAX_LLM_CONTEXT_BYTES: usize = 240;

/// コンテキスト長(トークン)。前置きと候補群を収める上限。
const N_CTX: u32 = 2048;
/// 1回に採点する候補の上限。
const MAX_BATCH: usize = 32;
/// zenz/genmai 系の語彙サイズ上限 (TinySwallow / Swallow 等の 15 万語彙と区別する)。
const MAX_ZENZ_VOCAB: usize = 16_000;

const INPUT_TAG: char = '\u{EE00}';
const OUTPUT_TAG: char = '\u{EE01}';
const CONTEXT_TAG: char = '\u{EE02}';

/// zenz のトークナイザに合わせる。空白は全角空白にし、改行は落とす(Zenzai と同じ)。
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|&c| c != '\n' && c != '\r')
        .map(|c| if c == ' ' { '\u{3000}' } else { c })
        .collect()
}

fn to_katakana(reading: &str) -> String {
    reading
        .chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

/// 候補の直前までのプロンプト (`v3 = false`: v2 形式, `v3 = true`: genmai / v3 形式)。
pub fn prompt_with_style(left_context: &str, reading: &str, v3: bool) -> String {
    let context = normalize(left_context);
    let skip = context.chars().count().saturating_sub(MAX_CONTEXT_CHARS);
    let context: String = context.chars().skip(skip).collect();
    let mut out = String::new();
    if v3 && !context.is_empty() {
        out.push(CONTEXT_TAG);
        out.push_str(&context);
    }
    out.push(INPUT_TAG);
    out.push_str(&normalize(&to_katakana(reading)));
    if !v3 && !context.is_empty() {
        out.push(CONTEXT_TAG);
        out.push_str(&context);
    }
    out.push(OUTPUT_TAG);
    out
}

/// 互換用の既定 (zenz-v2) プロンプト生成関数。
pub fn prompt(left_context: &str, reading: &str) -> String {
    prompt_with_style(left_context, reading, false)
}

fn has_stray_small_kana(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (i, &ch) in chars.iter().enumerate() {
        let small = matches!(
            ch,
            'ぁ' | 'ぃ'
                | 'ぅ'
                | 'ぇ'
                | 'ぉ'
                | 'ゃ'
                | 'ゅ'
                | 'ょ'
                | 'ゎ'
                | 'ァ'
                | 'ィ'
                | 'ゥ'
                | 'ェ'
                | 'ォ'
                | 'ャ'
                | 'ュ'
                | 'ョ'
                | 'ヮ'
        );
        if small {
            let prev_kana = i
                .checked_sub(1)
                .and_then(|idx| chars.get(idx))
                .is_some_and(|&p| {
                    ('\u{3041}'..='\u{3096}').contains(&p) || ('\u{30A1}'..='\u{30FA}').contains(&p)
                });
            if !prev_kana {
                return true;
            }
        }
    }
    false
}

/// zenz / genmai による採点・候補生成器。
#[derive(Debug)]
pub struct ZenzScorer {
    model: Rc<Model>,
    ctx: Context<Rc<Model>>,
    v3_prompt: bool,
    bos_prompt: bool,
    last_generated: Vec<Vec<i32>>,
}

impl ZenzScorer {
    /// GGUF を読み込み、語彙メタデータ + 語彙ハッシュが zenz-v2.5 のものか確かめる(6.4)。
    pub fn open(path: &Path, threads: i32) -> Result<Self, LmError> {
        let model = Model::load(path)?;
        model.check_vocab(VOCAB_HASH)?;
        Self::with_path_hints(model, path, threads)
    }

    /// メタデータ有無を問わず、実トークン語彙列の SHA-256 が zenz-v2.5 と一致することを確認して開く。
    pub fn open_by_vocab_hash(path: &Path, threads: i32) -> Result<Self, LmError> {
        let model = Model::load(path)?;
        if model.vocab_hash().as_deref() != Some(VOCAB_HASH) {
            return Err(LmError::VocabMismatch);
        }
        Self::with_path_hints(model, path, threads)
    }

    /// zenz 系 / genmai 系 GGUF を互換モードで開く (`n_vocab <= 16000`)。
    pub fn open_compatible(path: &Path, threads: i32) -> Result<Self, LmError> {
        let model = Model::load(path)?;
        if model.n_vocab() > MAX_ZENZ_VOCAB {
            return Err(LmError::VocabMismatch);
        }
        Self::with_path_hints(model, path, threads)
    }

    fn with_path_hints(model: Model, path: &Path, threads: i32) -> Result<Self, LmError> {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let v3_prompt = name.contains("genmai") || name.contains("zenz-v3");
        let bos_prompt = v3_prompt || std::env::var_os("ASTERIA_ZENZ_BOS").is_some();
        let model = Rc::new(model);
        let ctx = Context::new(Rc::clone(&model), N_CTX, MAX_BATCH as u32, threads)?;
        Ok(Self {
            model,
            ctx,
            v3_prompt,
            bos_prompt,
            last_generated: Vec::new(),
        })
    }

    /// 語彙を確かめずに作る(テスト用の小さなモデルで配線を確かめる)。
    #[cfg(test)]
    pub(crate) fn with_model(model: Model, threads: i32) -> Result<Self, LmError> {
        let model = Rc::new(model);
        let ctx = Context::new(Rc::clone(&model), N_CTX, MAX_BATCH as u32, threads)?;
        Ok(Self {
            model,
            ctx,
            v3_prompt: false,
            bos_prompt: false,
            last_generated: Vec::new(),
        })
    }

    /// 読み込んだモデル(メタデータの参照に使う)。
    pub fn model(&self) -> &Model {
        &self.model
    }

    fn build_prefix(&self, left_context: &str, reading: &str) -> Result<Vec<i32>, LmError> {
        let mut prefix = Vec::new();
        if self.bos_prompt {
            let bos = self.model.bos();
            if bos >= 0 {
                prefix.push(bos);
            }
        }
        let body = self.model.tokenize(
            &prompt_with_style(left_context, reading, self.v3_prompt),
            false,
        )?;
        prefix.extend(body);
        Ok(prefix)
    }

    /// 入力中に前置き KV を先読みして温める。
    pub fn prewarm(&mut self, left_context: &str, reading: &str) {
        if let Ok(prefix) = self.build_prefix(left_context, reading) {
            if prefix.len() >= 2 {
                let _ = self.ctx.prewarm_prefix(&prefix[..prefix.len() - 1]);
            }
        }
    }

    /// 下書きなしでの候補生成。
    pub fn generate(
        &mut self,
        left_context: &str,
        reading: &str,
        beams: usize,
        cancel: Option<&CancelToken>,
    ) -> Vec<String> {
        self.generate_with_drafts(left_context, reading, &[], beams, cancel)
    }

    /// ラティス候補・直前打鍵結果から下書きを供給する高速ビーム探索生成 (`DraftedBeamSearch`, ADR 0090)。
    pub fn generate_with_drafts(
        &mut self,
        left_context: &str,
        reading: &str,
        draft_surfaces: &[String],
        beams: usize,
        cancel: Option<&CancelToken>,
    ) -> Vec<String> {
        if beams == 0 || reading.is_empty() {
            return Vec::new();
        }
        let Ok(prefix) = self.build_prefix(left_context, reading) else {
            return Vec::new();
        };
        let mut drafts: Vec<Vec<i32>> = self.last_generated.clone();
        for s in draft_surfaces.iter().take(4) {
            if let Ok(toks) = self.model.tokenize(&normalize(s), false) {
                if !toks.is_empty() && !drafts.contains(&toks) {
                    drafts.push(toks);
                }
            }
        }
        let draft_tokens = std::env::var("ASTERIA_LM_DRAFT")
            .ok()
            .or_else(|| std::env::var("KOTORI_LM_DRAFT").ok())
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(10);

        let eos = self.model.eos();
        let max_steps = reading.chars().count() + 8;
        let cancel_fn = || cancel.is_some_and(CancelToken::is_cancelled);
        let model_ref = Rc::clone(&self.model);
        let beams_res = match self.ctx.drafted_beam_generate(
            &prefix,
            &drafts,
            beams,
            max_steps,
            draft_tokens,
            |tok| tok == eos || model_ref.is_eog(tok),
            Some(&cancel_fn),
        ) {
            Ok(b) => b,
            Err(_) => return Vec::new(),
        };

        self.last_generated = beams_res.iter().take(4).map(|(t, _)| t.clone()).collect();

        let mut out = Vec::new();
        for (tokens, _) in beams_res {
            let Some(mut text) = self.model.detokenize(&tokens) else {
                continue;
            };
            if text.contains('\u{3000}') {
                text = text.replace('\u{3000}', " ");
            }
            let text = text.trim_matches(['\r', '\n']).to_owned();
            if text.is_empty() || has_stray_small_kana(&text) {
                continue;
            }
            if !out.contains(&text) {
                out.push(text);
            }
        }
        out
    }

    /// 候補ごとの log P(候補 `</s>` | プロンプト)。KV プレフィックスを共有・再利用する。
    pub fn score_all(
        &mut self,
        left_context: &str,
        reading: &str,
        candidates: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<f32>>, LmError> {
        let prefix = self.build_prefix(left_context, reading)?;
        let eos = self.model.eos();
        let mut tokens = Vec::with_capacity(candidates.len());
        for c in candidates {
            let mut t = self.model.tokenize(&normalize(c), false)?;
            t.push(eos);
            tokens.push(t);
        }
        let mut scores = Vec::with_capacity(tokens.len());
        let mut start = 0;
        while start < tokens.len() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Ok(None);
            }
            let mut end = start;
            let mut total = prefix.len();
            while end < tokens.len()
                && end - start < MAX_BATCH
                && (end == start || total + tokens[end].len() <= N_CTX as usize)
            {
                total += tokens[end].len();
                end += 1;
            }
            scores.extend(
                self.ctx
                    .score_candidates_cached(&prefix, &tokens[start..end])?,
            );
            start = end;
        }
        Ok(Some(scores))
    }
}

impl Scorer for ZenzScorer {
    fn score(&mut self, request: &RerankRequest, cancel: &CancelToken) -> Option<Vec<f32>> {
        self.score_all(
            &request.left_context,
            &request.reading,
            &request.candidates,
            Some(cancel),
        )
        .ok()
        .flatten()
    }

    fn prewarm(&mut self, left_context: &str, reading: &str) {
        ZenzScorer::prewarm(self, left_context, reading);
    }
}

/// 汎用 LLM (`TinySwallow-1.5B`, `Swallow-8B` 等) による文脈自然さ判定採点器 (`zenz_scorer.cc` 互換)。
#[derive(Debug)]
pub struct LlmScorer {
    model: Rc<Model>,
    ctx: Context<Rc<Model>>,
}

impl LlmScorer {
    /// 汎用 LLM (語彙 > 16000) を読み込む。
    pub fn open(path: &Path, threads: i32) -> Result<Self, LmError> {
        let model = Model::load(path)?;
        if model.n_vocab() <= MAX_ZENZ_VOCAB {
            return Err(LmError::VocabMismatch);
        }
        let model = Rc::new(model);
        let ctx = Context::new(Rc::clone(&model), N_CTX, MAX_BATCH as u32, threads)?;
        Ok(Self { model, ctx })
    }

    fn tail_context(left_context: &str) -> &str {
        if left_context.len() <= MAX_LLM_CONTEXT_BYTES {
            return left_context;
        }
        let mut idx = left_context.len() - MAX_LLM_CONTEXT_BYTES;
        while idx < left_context.len() && !left_context.is_char_boundary(idx) {
            idx += 1;
        }
        &left_context[idx..]
    }

    fn build_prefix(&self, left_context: &str) -> Result<Vec<i32>, LmError> {
        let mut prefix = Vec::new();
        let bos = self.model.bos();
        if bos >= 0 && self.model.add_bos() {
            prefix.push(bos);
        }
        let mut ctx = Self::tail_context(left_context);
        if ctx.is_empty() && prefix.is_empty() {
            ctx = "\n";
        }
        if !ctx.is_empty() {
            prefix.extend(self.model.tokenize(ctx, false)?);
        }
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        Ok(prefix)
    }

    pub fn prewarm(&mut self, left_context: &str) {
        if let Ok(prefix) = self.build_prefix(left_context) {
            if prefix.len() >= 2 {
                let _ = self.ctx.prewarm_prefix(&prefix[..prefix.len() - 1]);
            }
        }
    }

    /// 候補文字列のトークン数を返す（先頭境界正規化用）。
    pub fn count_tokens(&self, text: &str) -> usize {
        self.model
            .tokenize(text, false)
            .map_or(1, |t| t.len().max(1))
    }

    /// 文脈なし (`""`) での候補対数尤度（文脈コピー反復 `[copy]` 再評価用）。
    pub fn score_isolated(
        &mut self,
        candidates: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<f32>>, LmError> {
        self.score_all("", candidates, cancel)
    }

    /// 候補ごとの log P(候補 | 左文脈)。
    pub fn score_all(
        &mut self,
        left_context: &str,
        candidates: &[String],
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Vec<f32>>, LmError> {
        let prefix = self.build_prefix(left_context)?;
        let mut tokens = Vec::with_capacity(candidates.len());
        for c in candidates {
            let t = self.model.tokenize(c, false)?;
            tokens.push(t);
        }
        let mut scores = Vec::with_capacity(tokens.len());
        let mut start = 0;
        while start < tokens.len() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Ok(None);
            }
            let mut end = start;
            let mut total = prefix.len();
            while end < tokens.len()
                && end - start < MAX_BATCH
                && (end == start || total + tokens[end].len() <= N_CTX as usize)
            {
                total += tokens[end].len();
                end += 1;
            }
            scores.extend(
                self.ctx
                    .score_candidates_cached(&prefix, &tokens[start..end])?,
            );
            start = end;
        }
        Ok(Some(scores))
    }
}
