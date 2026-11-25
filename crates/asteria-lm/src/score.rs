//! 共有プレフィックスのバッチ採点と KV キャッシュ先読み・ビーム生成(docs/SPEC.md 6.2 モード A)。

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::ptr::NonNull;
use std::rc::Rc;

use crate::ffi::{self, LlamaContext};
use crate::{LmError, Model};

const PREFIX_CHUNK: usize = 256;

/// 推論のコンテキスト。モデルを借りる(`&Model`)か、共有して持つ(`Rc<Model>` など)。
#[derive(Debug)]
pub struct Context<M: Borrow<Model>> {
    model: M,
    raw: NonNull<LlamaContext>,
    n_ctx: usize,
    n_seq: usize,
    cached_prefix: Vec<i32>,
}

impl<M: Borrow<Model>> Context<M> {
    /// `n_ctx` トークンまで、同時に `max_candidates` 件の候補を採点できるコンテキストを作る。
    pub fn new(model: M, n_ctx: u32, max_candidates: u32, threads: i32) -> Result<Self, LmError> {
        let n_seq = max_candidates + 1;
        let raw =
            ffi::ctx_new(model.borrow().raw, n_ctx, n_seq, threads).ok_or(LmError::Context)?;
        Ok(Self {
            model,
            raw,
            n_ctx: n_ctx as usize,
            n_seq: n_seq as usize,
            cached_prefix: Vec::new(),
        })
    }

    fn reset_memory(&mut self) {
        ffi::clear(self.raw);
        self.cached_prefix.clear();
    }

    /// シーケンス 0 に `target` トークン列を載せる (共通プレフィックスは再利用する)。
    fn ensure_seq0_prefix(&mut self, target: &[i32]) -> Result<(), LmError> {
        for s in 1..self.n_seq {
            let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
        }
        let mut keep = 0usize;
        while keep < self.cached_prefix.len()
            && keep < target.len()
            && self.cached_prefix[keep] == target[keep]
        {
            keep += 1;
        }
        if !ffi::seq_rm(self.raw, 0, keep as i32, -1) {
            ffi::clear(self.raw);
            self.cached_prefix.clear();
            keep = 0;
        } else {
            self.cached_prefix.truncate(keep);
        }

        let mut at = keep;
        while at < target.len() {
            let end = (at + PREFIX_CHUNK).min(target.len());
            let chunk = &target[at..end];
            let pos: Vec<i32> = (at as i32..end as i32).collect();
            let seqs: Vec<&[i32]> = vec![&[0][..]; chunk.len()];
            let want = vec![0i8; chunk.len()];
            if ffi::decode(self.raw, chunk, &pos, &seqs, &want) != 0 {
                self.reset_memory();
                return Err(LmError::Decode);
            }
            at = end;
        }
        self.cached_prefix = target.to_vec();
        Ok(())
    }

    /// 入力途中のプレフィックス KV をバックグラウンドで事前計算してシーケンス 0 へ温めておく。
    pub fn prewarm_prefix(&mut self, prefix_without_last: &[i32]) -> Result<(), LmError> {
        if prefix_without_last.len() >= self.n_ctx {
            return Ok(());
        }
        self.ensure_seq0_prefix(prefix_without_last)
    }

    /// 前置きに続く各候補の対数確率。候補が空列なら 0 (KV は毎回リセットし決定性を保つ)。
    pub fn score_candidates(
        &mut self,
        prefix: &[i32],
        candidates: &[Vec<i32>],
    ) -> Result<Vec<f32>, LmError> {
        self.reset_memory();
        let res = self.score_candidates_impl(prefix, candidates);
        self.reset_memory();
        res
    }

    /// 前置き KV を次回呼び出しへ引き継ぐ高速版 `score_candidates` (`UsePrefix` / `KeepPrefix`)。
    pub fn score_candidates_cached(
        &mut self,
        prefix: &[i32],
        candidates: &[Vec<i32>],
    ) -> Result<Vec<f32>, LmError> {
        self.score_candidates_impl(prefix, candidates)
    }

    fn score_candidates_impl(
        &mut self,
        prefix: &[i32],
        candidates: &[Vec<i32>],
    ) -> Result<Vec<f32>, LmError> {
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        if candidates.len() + 1 > self.n_seq {
            return Err(LmError::TooManyCandidates);
        }
        let trie = Trie::new(candidates);
        if prefix.len() + trie.nodes.len() > self.n_ctx {
            return Err(LmError::TooLong);
        }
        let n_vocab = self.model.borrow().n_vocab();
        let body_len = prefix.len() - 1;
        self.ensure_seq0_prefix(&prefix[..body_len])?;

        // 前置き末尾 1 トークン + トライ節を1つ以上のバッチに分けて評価する前に seq 1..=candidates.len() を用意。
        let n_cand = candidates.len();
        let mut last_seq_slice: Vec<i32> = Vec::with_capacity(n_cand.max(1));
        for s in 1..=n_cand {
            ffi::seq_cp(self.raw, 0, s as i32);
            last_seq_slice.push(s as i32);
        }
        if n_cand == 0 {
            last_seq_slice.push(0);
        }

        let last_pos = body_len as i32;
        if trie.nodes.len() < PREFIX_CHUNK {
            let mut tokens = Vec::with_capacity(1 + trie.nodes.len());
            let mut pos = Vec::with_capacity(1 + trie.nodes.len());
            let mut seqs: Vec<&[i32]> = Vec::with_capacity(1 + trie.nodes.len());
            let mut want = Vec::with_capacity(1 + trie.nodes.len());

            tokens.push(prefix[body_len]);
            pos.push(last_pos);
            seqs.push(&last_seq_slice);
            want.push(1i8);

            for node in &trie.nodes {
                tokens.push(node.token);
                pos.push((prefix.len() + node.depth) as i32);
                seqs.push(node.seqs.as_slice());
                want.push(i8::from(node.has_child));
            }
            if ffi::decode(self.raw, &tokens, &pos, &seqs, &want) != 0 {
                self.reset_memory();
                return Err(LmError::Decode);
            }

            let mut scores = vec![0.0f32; n_cand];
            if n_cand == 0 {
                return Ok(scores);
            }
            // batch index 0 = prefix 末尾 -> 各候補の先頭トークンを採点
            let first_toks: Vec<i32> = candidates
                .iter()
                .filter_map(|c| c.first().copied())
                .collect();
            let first_lps =
                ffi::token_logprobs(self.raw, 0, n_vocab, &first_toks).ok_or(LmError::Decode)?;
            let mut first_cursor = 0usize;
            for (i, cand) in candidates.iter().enumerate() {
                if !cand.is_empty() {
                    scores[i] += first_lps[first_cursor];
                    first_cursor += 1;
                }
            }

            // 親ノードごとに子トークンを集約して C 側で 1 パス LogSumExp 計算
            let mut children_of: Vec<Vec<(usize, i32)>> = vec![Vec::new(); trie.nodes.len()];
            for (cand_idx, (cand, path)) in candidates.iter().zip(&trie.paths).enumerate() {
                for j in 1..cand.len() {
                    children_of[path[j - 1]].push((cand_idx, cand[j]));
                }
            }
            for (node_idx, pairs) in children_of.iter().enumerate() {
                if pairs.is_empty() {
                    continue;
                }
                let child_toks: Vec<i32> = pairs.iter().map(|&(_, t)| t).collect();
                let lps = ffi::token_logprobs(self.raw, node_idx as i32 + 1, n_vocab, &child_toks)
                    .ok_or(LmError::Decode)?;
                for ((cand_idx, _), lp) in pairs.iter().zip(lps) {
                    scores[*cand_idx] += lp;
                }
            }
            for s in 1..=n_cand {
                let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
            }
            return Ok(scores);
        }

        // トライが大きく 1 バッチ(256 トークン)を超える場合は、前置き末尾とトライ節をチャンク評価し都度ロジットを回収
        if ffi::decode(
            self.raw,
            &[prefix[body_len]],
            &[last_pos],
            &[&last_seq_slice],
            &[1],
        ) != 0
        {
            self.reset_memory();
            return Err(LmError::Decode);
        }
        let mut scores = vec![0.0f32; n_cand];
        let first_toks: Vec<i32> = candidates
            .iter()
            .filter_map(|c| c.first().copied())
            .collect();
        let first_lps =
            ffi::token_logprobs(self.raw, 0, n_vocab, &first_toks).ok_or(LmError::Decode)?;
        let mut first_cursor = 0usize;
        for (i, cand) in candidates.iter().enumerate() {
            if !cand.is_empty() {
                scores[i] += first_lps[first_cursor];
                first_cursor += 1;
            }
        }

        let mut children_of: Vec<Vec<(usize, i32)>> = vec![Vec::new(); trie.nodes.len()];
        for (cand_idx, (cand, path)) in candidates.iter().zip(&trie.paths).enumerate() {
            for j in 1..cand.len() {
                children_of[path[j - 1]].push((cand_idx, cand[j]));
            }
        }
        let mut start = 0usize;
        while start < trie.nodes.len() {
            let end = (start + PREFIX_CHUNK).min(trie.nodes.len());
            let slice = &trie.nodes[start..end];
            let tokens: Vec<i32> = slice.iter().map(|x| x.token).collect();
            let pos: Vec<i32> = slice
                .iter()
                .map(|x| (prefix.len() + x.depth) as i32)
                .collect();
            let seqs: Vec<&[i32]> = slice.iter().map(|x| x.seqs.as_slice()).collect();
            let want: Vec<i8> = slice.iter().map(|x| i8::from(x.has_child)).collect();
            if ffi::decode(self.raw, &tokens, &pos, &seqs, &want) != 0 {
                self.reset_memory();
                return Err(LmError::Decode);
            }
            for offset in 0..(end - start) {
                let pairs = &children_of[start + offset];
                if pairs.is_empty() {
                    continue;
                }
                let child_toks: Vec<i32> = pairs.iter().map(|&(_, t)| t).collect();
                let lps = ffi::token_logprobs(self.raw, offset as i32, n_vocab, &child_toks)
                    .ok_or(LmError::Decode)?;
                for ((cand_idx, _), lp) in pairs.iter().zip(lps) {
                    scores[*cand_idx] += lp;
                }
            }
            start = end;
        }
        for s in 1..=n_cand {
            let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
        }
        Ok(scores)
    }

    /// 共有 KV キャッシュとダブルバッファ・シーケンスによる高速ビーム探索候補生成。
    pub fn beam_generate<F, C>(
        &mut self,
        prefix: &[i32],
        beams_width: usize,
        max_steps: usize,
        is_end: F,
        cancel: Option<&C>,
    ) -> Result<Vec<(Vec<i32>, f32)>, LmError>
    where
        F: Fn(i32) -> bool,
        C: Fn() -> bool,
    {
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        if beams_width == 0 || max_steps == 0 || self.n_seq < 3 {
            return Ok(Vec::new());
        }
        if prefix.len() + max_steps + 1 > self.n_ctx {
            return Err(LmError::TooLong);
        }
        let max_beams = ((self.n_seq - 1) / 2).min(beams_width);
        let n_vocab = self.model.borrow().n_vocab();
        let body_len = prefix.len() - 1;
        self.ensure_seq0_prefix(&prefix[..body_len])?;

        // 前置きの末尾 1 トークンを seq 1 で評価し、最初の分布を得る。
        ffi::seq_cp(self.raw, 0, 1);
        if ffi::decode(
            self.raw,
            &[prefix[body_len]],
            &[body_len as i32],
            &[&[1][..]],
            &[1],
        ) != 0
        {
            self.reset_memory();
            return Err(LmError::Decode);
        }

        struct BeamState {
            tokens: Vec<i32>,
            logp: f32,
            seq: i32,
            logit_idx: i32,
        }
        let mut live = vec![BeamState {
            tokens: Vec::new(),
            logp: 0.0,
            seq: 1,
            logit_idx: 0,
        }];
        let mut done: Vec<(Vec<i32>, f32)> = Vec::new();
        let mut bank = 1usize; // bank 0 => 1..=max_beams (seq 1 使用中), bank 1 => max_beams+1..=2*max_beams

        for step in 0..max_steps {
            if cancel.is_some_and(|f| f()) {
                for s in 1..self.n_seq {
                    let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
                }
                return Ok(Vec::new());
            }
            if live.is_empty() || done.len() >= max_beams {
                break;
            }
            let mut next_cands: Vec<(f32, usize, i32)> = Vec::new();
            for (parent_idx, b) in live.iter().enumerate() {
                for (tok, lp) in ffi::topk(self.raw, b.logit_idx, n_vocab, max_beams) {
                    next_cands.push((b.logp + lp, parent_idx, tok));
                }
            }
            next_cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

            let mut grown: Vec<(usize, Vec<i32>, f32)> = Vec::new();
            for (score, parent_idx, tok) in next_cands {
                if grown.len() + done.len() >= max_beams {
                    break;
                }
                let mut toks = live[parent_idx].tokens.clone();
                if is_end(tok) {
                    let denom = (toks.len() + 1) as f32;
                    if !toks.is_empty() {
                        done.push((toks, score / denom));
                    }
                    continue;
                }
                toks.push(tok);
                grown.push((parent_idx, toks, score));
            }
            if grown.is_empty() || step + 1 == max_steps {
                break;
            }

            let bank_base = if bank == 0 { 1 } else { (max_beams + 1) as i32 };
            let mut batch_tokens = Vec::with_capacity(grown.len());
            let mut batch_pos = Vec::with_capacity(grown.len());
            let mut new_seqs = Vec::with_capacity(grown.len());
            let mut want = vec![1i8; grown.len()];
            let mut next_live = Vec::with_capacity(grown.len());

            for (i, (parent_idx, toks, score)) in grown.into_iter().enumerate() {
                let dst_seq = bank_base + i as i32;
                let _ = ffi::seq_rm(self.raw, dst_seq, -1, -1);
                ffi::seq_cp(self.raw, live[parent_idx].seq, dst_seq);
                let tok = toks[toks.len() - 1];
                let p = (prefix.len() + toks.len() - 1) as i32;
                batch_tokens.push(tok);
                batch_pos.push(p);
                new_seqs.push(vec![dst_seq]);
                next_live.push(BeamState {
                    tokens: toks,
                    logp: score,
                    seq: dst_seq,
                    logit_idx: i as i32,
                });
                want[i] = 1;
            }
            let seq_refs: Vec<&[i32]> = new_seqs.iter().map(Vec::as_slice).collect();
            if ffi::decode(self.raw, &batch_tokens, &batch_pos, &seq_refs, &want) != 0 {
                break;
            }
            for old in &live {
                let _ = ffi::seq_rm(self.raw, old.seq, -1, -1);
            }
            live = next_live;
            bank ^= 1;
        }
        for s in 1..self.n_seq {
            let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
        }
        done.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        done.truncate(max_beams);
        Ok(done)
    }

    /// ラティス文・直前生成を下書きにし、一致ステップの GPU 呼び出しを省略する高速ビーム探索 (ADR 0090)。
    #[allow(clippy::too_many_arguments)]
    pub fn drafted_beam_generate<F, C>(
        &mut self,
        prefix: &[i32],
        drafts: &[Vec<i32>],
        beams_width: usize,
        max_steps: usize,
        draft_tokens: usize,
        is_end: F,
        cancel: Option<&C>,
    ) -> Result<Vec<(Vec<i32>, f32)>, LmError>
    where
        F: Fn(i32) -> bool,
        C: Fn() -> bool,
    {
        if drafts.is_empty()
            || draft_tokens == 0
            || beams_width == 0
            || beams_width * (1 + draft_tokens) > PREFIX_CHUNK
            || self.n_seq < 2 + 2 * beams_width
        {
            return self.beam_generate(prefix, beams_width, max_steps, is_end, cancel);
        }
        if prefix.is_empty() {
            return Err(LmError::EmptyPrefix);
        }
        if max_steps == 0 {
            return Ok(Vec::new());
        }
        if prefix.len() + max_steps + draft_tokens + 1 > self.n_ctx {
            return self.beam_generate(prefix, beams_width, max_steps, is_end, cancel);
        }

        let max_beams = beams_width.min((self.n_seq - 2) / 2);
        let n_vocab = self.model.borrow().n_vocab();
        let body_len = prefix.len() - 1;
        let base_pos = prefix.len();
        self.ensure_seq0_prefix(&prefix[..body_len])?;

        // seq 1 で前置き末尾を評価して初期分布を取得 (seq 0 は先頭プレフィックスとして温存)
        ffi::seq_cp(self.raw, 0, 1);
        if ffi::decode(
            self.raw,
            &[prefix[body_len]],
            &[body_len as i32],
            &[&[1][..]],
            &[1],
        ) != 0
        {
            self.reset_memory();
            return Err(LmError::Decode);
        }

        type Dist = Rc<Vec<(i32, f32)>>;
        struct DBeam {
            tokens: Vec<i32>,
            logp: f32,
            seq: i32,
            dist: Dist,
            chain: VecDeque<(i32, Dist)>,
        }

        let find_draft = |t: &[i32]| -> Vec<i32> {
            let mut out = Vec::new();
            let len = t.len();
            for d in drafts {
                if d.len() > len && d[..len] == *t {
                    out.extend(d[len..].iter().copied().take(draft_tokens));
                    return out;
                }
            }
            for m in (1..=len.min(4)).rev() {
                let tail = &t[len - m..];
                let mut best: Option<(usize, &[i32], usize)> = None;
                for d in drafts {
                    if d.len() <= m {
                        continue;
                    }
                    for j in m..d.len() {
                        if &d[j - m..j] == tail {
                            let gap = j.abs_diff(len);
                            if best.is_none() || matches!(best, Some((bg, _, _)) if gap < bg) {
                                best = Some((gap, d.as_slice(), j));
                            }
                        }
                    }
                }
                if let Some((_, d, j)) = best {
                    out.extend(d[j..].iter().copied().take(draft_tokens));
                    return out;
                }
            }
            out
        };

        let mut free_seqs: Vec<i32> = (2..=(2 * max_beams as i32 + 1)).rev().collect();
        let init_dist: Dist = Rc::new(ffi::topk(self.raw, 0, n_vocab, max_beams));
        let mut live = vec![DBeam {
            tokens: Vec::new(),
            logp: 0.0,
            seq: 1,
            dist: init_dist,
            chain: VecDeque::new(),
        }];
        let mut done: Vec<(Vec<i32>, f32)> = Vec::new();

        for step in 0..max_steps {
            if cancel.is_some_and(|f| f()) {
                for s in 1..self.n_seq {
                    let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
                }
                return Ok(Vec::new());
            }
            if live.is_empty() || done.len() >= max_beams {
                break;
            }
            let mut next_cands: Vec<(f32, usize, i32)> = Vec::new();
            for (p, b) in live.iter().enumerate() {
                for &(tok, lp) in b.dist.iter() {
                    next_cands.push((b.logp + lp, p, tok));
                }
            }
            next_cands.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

            let mut grown_meta: Vec<(usize, Vec<i32>, f32)> = Vec::new();
            for (score, p, tok) in next_cands {
                if grown_meta.len() + done.len() >= max_beams {
                    break;
                }
                let mut t = live[p].tokens.clone();
                if is_end(tok) {
                    let denom = (t.len() + 1) as f32;
                    if !t.is_empty() {
                        done.push((t, score / denom));
                    }
                    continue;
                }
                t.push(tok);
                grown_meta.push((p, t, score));
            }
            if grown_meta.is_empty() || step + 1 == max_steps {
                break;
            }

            let mut follows = vec![false; grown_meta.len()];
            let mut inherited = vec![false; live.len()];
            for (i, (p, t, _)) in grown_meta.iter().enumerate() {
                let last_tok = t[t.len() - 1];
                if !inherited[*p]
                    && live[*p]
                        .chain
                        .front()
                        .is_some_and(|&(dt, _)| dt == last_tok)
                {
                    follows[i] = true;
                    inherited[*p] = true;
                }
            }

            let mut next_live: Vec<Option<DBeam>> = (0..grown_meta.len()).map(|_| None).collect();
            let mut todo_indices = Vec::new();

            for (i, (p, t, score)) in grown_meta.iter().enumerate() {
                if follows[i] {
                    continue;
                }
                let Some(seq) = free_seqs.pop() else {
                    for s in 1..self.n_seq {
                        let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
                    }
                    return Ok(Vec::new());
                };
                let par_end_pos = (base_pos + live[*p].tokens.len()) as i32;
                let _ = ffi::seq_rm(self.raw, seq, -1, -1);
                ffi::seq_cp_range(self.raw, live[*p].seq, seq, 0, par_end_pos);
                todo_indices.push((i, seq, t.clone(), *score));
            }

            for (i, (p, t, score)) in grown_meta.into_iter().enumerate() {
                if !follows[i] {
                    continue;
                }
                let par_seq = live[p].seq;
                let popped = live[p].chain.pop_front();
                let dist = popped.map(|(_, d)| d).unwrap_or_default();
                let rem = std::mem::take(&mut live[p].chain);
                next_live[i] = Some(DBeam {
                    tokens: t,
                    logp: score,
                    seq: par_seq,
                    dist,
                    chain: rem,
                });
            }

            for (p, par) in live.iter().enumerate() {
                if !inherited[p] {
                    let _ = ffi::seq_rm(self.raw, par.seq, -1, -1);
                    if par.seq >= 2 {
                        free_seqs.push(par.seq);
                    }
                }
            }

            if !todo_indices.is_empty() {
                let mut chains = Vec::with_capacity(todo_indices.len());
                let mut batch_tokens = Vec::new();
                let mut batch_pos = Vec::new();
                let mut batch_seqs: Vec<Vec<i32>> = Vec::new();
                let mut want = Vec::new();
                let mut ranges = Vec::with_capacity(todo_indices.len());

                for (_, seq, toks, _) in &todo_indices {
                    let draft = find_draft(toks);
                    let start_idx = batch_tokens.len();
                    let tok_pos_base = base_pos + toks.len() - 1;
                    batch_tokens.push(toks[toks.len() - 1]);
                    batch_pos.push(tok_pos_base as i32);
                    batch_seqs.push(vec![*seq]);
                    want.push(1i8);
                    for (j, &dt) in draft.iter().enumerate() {
                        batch_tokens.push(dt);
                        batch_pos.push((tok_pos_base + 1 + j) as i32);
                        batch_seqs.push(vec![*seq]);
                        want.push(1i8);
                    }
                    let count = 1 + draft.len();
                    ranges.push((start_idx, count));
                    chains.push(draft);
                }

                let seq_refs: Vec<&[i32]> = batch_seqs.iter().map(Vec::as_slice).collect();
                if ffi::decode(self.raw, &batch_tokens, &batch_pos, &seq_refs, &want) != 0 {
                    break;
                }

                for (((i, seq, toks, score), (start_idx, _)), draft) in
                    todo_indices.into_iter().zip(ranges).zip(chains)
                {
                    let dist: Dist =
                        Rc::new(ffi::topk(self.raw, start_idx as i32, n_vocab, max_beams));
                    let mut chain = VecDeque::with_capacity(draft.len());
                    for (j, dt) in draft.into_iter().enumerate() {
                        let d: Dist = Rc::new(ffi::topk(
                            self.raw,
                            (start_idx + 1 + j) as i32,
                            n_vocab,
                            max_beams,
                        ));
                        chain.push_back((dt, d));
                    }
                    next_live[i] = Some(DBeam {
                        tokens: toks,
                        logp: score,
                        seq,
                        dist,
                        chain,
                    });
                }
            }

            live = next_live.into_iter().flatten().collect();
        }
        for s in 1..self.n_seq {
            let _ = ffi::seq_rm(self.raw, s as i32, -1, -1);
        }
        done.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
        done.truncate(max_beams);
        Ok(done)
    }
}

/// 候補のトークン列のトライの節。
#[derive(Debug)]
struct Node {
    token: i32,
    /// 前置きの直後を 0 とする深さ。
    depth: usize,
    /// この節を通る候補のシーケンス(候補 i は i + 1)。
    seqs: Vec<i32>,
    /// この節の次のトークンを採点する候補があるか(ロジットが要るか)。
    has_child: bool,
}

/// 候補のトークン列をまとめたトライ。
#[derive(Debug)]
struct Trie {
    nodes: Vec<Node>,
    /// 候補ごとの、各トークンの節の添字。
    paths: Vec<Vec<usize>>,
}

impl Trie {
    fn new(candidates: &[Vec<i32>]) -> Self {
        use std::collections::HashMap;
        let mut nodes: Vec<Node> = Vec::new();
        let mut children: HashMap<(Option<usize>, i32), usize> = HashMap::new();
        let mut paths = Vec::with_capacity(candidates.len());
        for (i, cand) in candidates.iter().enumerate() {
            let seq = i as i32 + 1;
            let mut parent = None;
            let mut path = Vec::with_capacity(cand.len());
            for (depth, &token) in cand.iter().enumerate() {
                if let Some(p) = parent {
                    let p: &mut Node = &mut nodes[p];
                    p.has_child = true;
                }
                let at = *children.entry((parent, token)).or_insert_with(|| {
                    nodes.push(Node {
                        token,
                        depth,
                        seqs: Vec::new(),
                        has_child: false,
                    });
                    nodes.len() - 1
                });
                nodes[at].seqs.push(seq);
                path.push(at);
                parent = Some(at);
            }
            paths.push(path);
        }
        Self { nodes, paths }
    }
}

impl<M: Borrow<Model>> Drop for Context<M> {
    fn drop(&mut self) {
        ffi::ctx_free(self.raw);
    }
}
