//! 変換品質採点・要求境界照合・比較・表記ゆれ採点・回帰ゴールデン追加。
//! (`conversion_quality.py`, `compare_quality.py`, `compare_runs.py`, `adoption_check.py`,
//!  `lenient_score.py`, `quality_triage.py`, `regression_add.py`, `score_context_choice.py`)

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use asteria_common::hash::sha256_file;
use asteria_common::kana::reading_key;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentCandidates {
    pub values: Vec<String>,
    pub total: usize,
}

/// 要求境界 `[eval_trace] request=N begin/end` で stderr を切り分ける。
pub fn split_scoring_requests(
    stderr: &[u8],
    expected_count: usize,
) -> Result<Vec<(usize, Vec<u8>)>> {
    let boundary_re = Regex::new(r"^\[eval_trace\] request=(\d+) (begin|end)$")?;
    let mut requests: Vec<(usize, Vec<u8>)> = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut active: Option<usize> = None;

    for chunk in stderr.split_inclusive(|&b| b == b'\n') {
        let stripped = chunk
            .strip_suffix(b"\n")
            .unwrap_or(chunk)
            .strip_suffix(b"\r")
            .unwrap_or_else(|| chunk.strip_suffix(b"\n").unwrap_or(chunk));
        if let Ok(line_str) = std::str::from_utf8(stripped) {
            if let Some(caps) = boundary_re.captures(line_str) {
                let req_id: usize = caps[1].parse()?;
                let phase = &caps[2];
                if phase == "begin" {
                    if active.is_some() || req_id != requests.len() + 1 {
                        bail!("要求IDの重複・順序不整合・境界の入れ子があります");
                    }
                    active = Some(req_id);
                    current.clear();
                } else {
                    if active != Some(req_id) {
                        bail!("要求IDの終了境界が一致しません");
                    }
                    requests.push((req_id, std::mem::take(&mut current)));
                    active = None;
                }
                continue;
            }
        }
        if stripped.starts_with(b"[eval_trace]") {
            bail!("要求ID境界の形式が不正です");
        } else if active.is_some() {
            current.extend_from_slice(chunk);
        } else if stripped.starts_with(b"[time]") || stripped.starts_with(b"[asteria]") {
            bail!("要求境界の外側に未対応の採点ログがあります");
        }
    }
    if active.is_some() || requests.len() != expected_count {
        bail!("要求境界または要求件数が不足しています");
    }
    Ok(requests)
}

/// 1 要求分の stderr ログから `[time]` と `[asteria]` 候補プールを解析する。
pub fn scoring_trace(stderr: &[u8], expected: &[String]) -> Result<Value> {
    let text = std::str::from_utf8(stderr).context("stderr が有効な UTF-8 ではありません")?;
    let score_re =
        Regex::new(r"^\[asteria\] (.+) zenz=([^ ]+) llm=([^ ]+) (lattice|gen) score=([^ ]+)$")?;
    let time_kv_re = Regex::new(r"(\w+)=([^ ]+)")?;

    #[derive(Clone)]
    struct Cand<'a> {
        value: &'a str,
        llm: f64,
        score: f64,
    }
    let mut candidates: Vec<Cand<'_>> = Vec::new();
    let mut times: Vec<BTreeMap<String, String>> = Vec::new();

    for line in text.lines() {
        if line.starts_with("[time] ") {
            let mut map = BTreeMap::new();
            for caps in time_kv_re.captures_iter(line) {
                map.insert(caps[1].to_owned(), caps[2].to_owned());
            }
            times.push(map);
        } else if let Some(caps) = score_re.captures(line) {
            let value = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let _zenz: f64 = caps[2].parse().unwrap_or(f64::NAN);
            let llm: f64 = caps[3].parse().unwrap_or(f64::NAN);
            let score: f64 = caps[5].parse().unwrap_or(f64::NAN);
            candidates.push(Cand { value, llm, score });
        }
    }

    let cands_declared: Option<usize> = times
        .first()
        .and_then(|t| t.get("cands"))
        .and_then(|v| v.parse().ok());
    if times.len() != 1 || candidates.is_empty() || cands_declared != Some(candidates.len()) {
        return Ok(json!({
            "status": "unverified",
            "reason": "missing_or_multiple_scoring_records",
        }));
    }

    let exp_set: HashSet<&str> = expected.iter().map(String::as_str).collect();
    let matches: Vec<&Cand<'_>> = candidates
        .iter()
        .filter(|c| exp_set.contains(c.value))
        .collect();
    let llm_on = times[0].get("llm_on").map(String::as_str) == Some("1");
    let top = candidates
        .iter()
        .map(|c| c.score)
        .fold(f64::NEG_INFINITY, f64::max);
    let expected_llm_scored = if llm_on {
        Value::Bool(matches.iter().any(|c| c.llm.is_finite()))
    } else {
        Value::Null
    };
    let expected_has_max = if top.is_finite() {
        Value::Bool(matches.iter().any(|c| c.score == top))
    } else {
        Value::Null
    };

    Ok(json!({
        "status": "complete_logged_pool",
        "candidates": candidates.len(),
        "expected_in_pool": !matches.is_empty(),
        "llm_on": llm_on,
        "expected_llm_scored": expected_llm_scored,
        "expected_jointly_selectable": matches.iter().any(|c| c.score.is_finite()),
        "expected_has_max_logged_score": expected_has_max,
        "timing": times[0],
    }))
}

/// `converter_main` の文節出力行をパース・検証する。
pub fn parse_segments(lines: &[String]) -> Result<Vec<SegmentCandidates>> {
    let seg_re = Regex::new(r"^-{10} Segment (\d+)/(\d+) \[([^]]+)\] -{10}$")?;
    let cand_re = Regex::new(r"^  (\d+)/(\d+) (.*)$")?;
    let mut segments: Vec<SegmentCandidates> = Vec::new();
    let mut declared: Option<usize> = None;

    for line in lines {
        if let Some(caps) = seg_re.captures(line) {
            let number: usize = caps[1].parse()?;
            let total: usize = caps[2].parse()?;
            let kind = &caps[3];
            if kind == "HISTORY" {
                bail!("履歴文節を含む出力は独立した変換として採点できません");
            }
            if number != segments.len() || declared.is_some_and(|d| d != total) {
                bail!("文節の順序または総数が不整合です");
            }
            declared = Some(total);
            segments.push(SegmentCandidates {
                values: Vec::new(),
                total: 0,
            });
        } else if let Some(cur) = segments.last_mut() {
            if let Some(caps) = cand_re.captures(line) {
                let rank: usize = caps[1].parse()?;
                let total: usize = caps[2].parse()?;
                let value = &caps[3];
                if rank != cur.values.len() || value.is_empty() || total <= rank {
                    bail!("候補の順位、総数または本文が不正です");
                }
                if cur.total != 0 && cur.total != total {
                    bail!("候補総数が不整合です");
                }
                cur.total = total;
                cur.values.push(value.to_owned());
            }
        }
    }
    if segments.is_empty()
        || declared != Some(segments.len())
        || segments.iter().any(|s| s.values.is_empty())
    {
        bail!("変換出力の文節または第一候補が欠落しています");
    }
    Ok(segments)
}

/// 候補直積を展開せずに DP で正解文字列への到達可能性を判定する。
pub fn reachable(
    segments: &[SegmentCandidates],
    expected: &str,
    per_segment_k: Option<usize>,
) -> bool {
    let mut positions: HashSet<usize> = [0].into_iter().collect();
    for seg in segments {
        let limit = per_segment_k.unwrap_or(seg.values.len());
        let mut next = HashSet::new();
        for &pos in &positions {
            let rest = &expected[pos..];
            for val in seg.values.iter().take(limit) {
                if rest.starts_with(val.as_str()) {
                    next.insert(pos + val.len());
                }
            }
        }
        if next.is_empty() {
            return false;
        }
        positions = next;
    }
    positions.contains(&expected.len())
}

pub fn score_case(item: &Value, lines: &[String], per_segment_k: usize) -> Result<Value> {
    let segments = parse_segments(lines)?;
    let expected: Vec<String> = item["expected_output"]
        .as_array()
        .context("expected_output")?
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect();
    let top: String = segments.iter().map(|s| s.values[0].as_str()).collect();
    let ok = expected.iter().any(|e| e == &top);
    let included = expected.iter().any(|e| reachable(&segments, e, None));
    let truncated = segments.iter().any(|s| s.values.len() < s.total);
    let stage = if ok {
        "correct"
    } else if included {
        "candidate_present_not_selected"
    } else if truncated {
        "unobserved_candidates"
    } else {
        "candidate_or_segmentation"
    };
    let reachable_k = expected
        .iter()
        .any(|e| reachable(&segments, e, Some(per_segment_k)));
    Ok(json!({
        "index": item["index"],
        "input": item["input"],
        "top1": top,
        "expected": expected,
        "ok": ok,
        "domain": item.get("domain").and_then(Value::as_str).unwrap_or("unspecified"),
        "context_text": item.get("context_text").and_then(Value::as_str).unwrap_or(""),
        "candidate_coverage": included,
        "candidates_truncated": truncated,
        "per_segment_k": per_segment_k,
        "reachable_at_per_segment_k": reachable_k,
        "diagnostic_stage": stage,
        "ai_delivery": "unverified",
        "segments": segments,
    }))
}

pub fn validate_items(items: &[Value]) -> Result<()> {
    if items.is_empty() {
        bail!("評価セットは空でない配列が必要です");
    }
    let mut seen: HashSet<String> = HashSet::new();
    for item in items {
        let idx_val = item.get("index").context("index がありません")?;
        if idx_val.is_boolean() || (!idx_val.is_string() && !idx_val.is_number()) {
            bail!("問題IDが不正または重複しています");
        }
        let idx_key = idx_val.to_string();
        if !seen.insert(idx_key) {
            bail!("問題IDが不正または重複しています");
        }
        for key in ["input", "context_text"] {
            if let Some(v) = item.get(key) {
                let Some(s) = v.as_str() else {
                    bail!("{key} が不正です");
                };
                if s.chars().any(|c| matches!(c, '\r' | '\n' | '\t' | '\0')) {
                    bail!("{key} が不正です");
                }
            }
        }
        if item
            .get("input")
            .and_then(Value::as_str)
            .map_or(true, |s| s.trim().is_empty())
        {
            bail!("読みが空です");
        }
        let Some(exp) = item.get("expected_output").and_then(Value::as_array) else {
            bail!("空でない正解表記集合が必要です");
        };
        if exp.is_empty()
            || exp
                .iter()
                .any(|v| v.as_str().map_or(true, |s| s.is_empty()))
        {
            bail!("空でない正解表記集合が必要です");
        }
    }
    Ok(())
}

/// 2 つの評価結果の同一問題・同一正解集合を検証しつつ改善・回帰台帳を作る。
pub fn compare_rows(before: &[Value], after: &[Value]) -> Result<Value> {
    fn index_rows(rows: &[Value]) -> Result<BTreeMap<String, &Value>> {
        let mut map = BTreeMap::new();
        for row in rows {
            let key = row["index"].to_string();
            if map.contains_key(&key) {
                bail!("比較結果の問題IDが重複しています");
            }
            let ok = row["ok"].as_bool().context("ok が bool ではありません")?;
            let top1 = row["top1"].as_str().context("top1")?;
            let exp = row["expected"].as_array().context("expected")?;
            if ok != exp.iter().any(|e| e.as_str() == Some(top1)) {
                bail!("正誤の記録が候補と一致しません");
            }
            map.insert(key, row);
        }
        if map.is_empty() {
            bail!("比較結果が空です");
        }
        Ok(map)
    }
    let a = index_rows(before)?;
    let b = index_rows(after)?;
    let keys_a: HashSet<&String> = a.keys().collect();
    let keys_b: HashSet<&String> = b.keys().collect();
    if keys_a != keys_b {
        bail!("比較対象の問題ID集合が異なります");
    }
    let mut ledger = Vec::new();
    let (mut imp, mut reg, mut unch, mut wrong, mut changed) = (0usize, 0, 0, 0, 0);
    for (key, old) in &a {
        let new = b[key];
        for field in ["input", "expected", "context_text", "domain"] {
            if old.get(field) != new.get(field) {
                bail!("問題 {key} の読み・正解・文脈・分野が異なります");
            }
        }
        let old_ok = old["ok"].as_bool().unwrap_or(false);
        let new_ok = new["ok"].as_bool().unwrap_or(false);
        let outcome = if !old_ok && new_ok {
            imp += 1;
            "improved"
        } else if old_ok && !new_ok {
            reg += 1;
            "regressed"
        } else if new_ok {
            unch += 1;
            "unchanged_correct"
        } else {
            wrong += 1;
            "still_wrong"
        };
        let is_changed = old["top1"] != new["top1"];
        if is_changed {
            changed += 1;
        }
        ledger.push(json!({
            "index": old["index"],
            "outcome": outcome,
            "before": old["top1"],
            "after": new["top1"],
            "changed": is_changed,
            "stage": new.get("diagnostic_stage").and_then(Value::as_str).unwrap_or("unverified"),
        }));
    }
    Ok(json!({
        "items": ledger.len(),
        "improved": imp,
        "regressed": reg,
        "unchanged_correct": unch,
        "still_wrong": wrong,
        "net_improvement": (imp as isize) - (reg as isize),
        "changed": changed,
        "ledger": ledger,
    }))
}

pub fn load_result(path: &Path) -> Result<(Vec<Value>, Value)> {
    let manifest_str = format!("{}.manifest.json", path.display());
    let manifest_path = Path::new(&manifest_str);
    let manifest: Value = serde_json::from_str(&fs::read_to_string(manifest_path)?)?;
    if Some(sha256_file(path)?.as_str()) != manifest["result_sha256"].as_str() {
        bail!("結果ファイルと実行条件記録が一致しません");
    }
    let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(path)?)?;
    Ok((rows, manifest))
}

pub fn validate_conditions(before: &Value, after: &Value) -> Result<()> {
    if before.get("data").and_then(|d| d.get("sha256"))
        != after.get("data").and_then(|d| d.get("sha256"))
    {
        bail!("評価データSHAが異なります");
    }
    let b_dict = before
        .get("dictionary")
        .and_then(|d| d.get("sha256"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let a_dict = after
        .get("dictionary")
        .and_then(|d| d.get("sha256"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    if b_dict.is_none() || b_dict != a_dict {
        bail!("辞書SHAが欠落または一致しません");
    }
    for key in ["context", "case_isolated", "profile_policy"] {
        if before.get(key) != after.get(key) {
            bail!("評価条件 {key} が一致しません");
        }
    }
    Ok(())
}

/// `regression_add.py`: 回帰ゴールデンへ新しい誤変換ケースを 1 問ずつ追加する。
pub fn append_regression_golden(
    set_path: &Path,
    reading: &str,
    expected: &[String],
    context: &str,
    domain: &str,
    note: &str,
) -> Result<Value> {
    let mut items: Vec<Value> = if set_path.exists() {
        serde_json::from_str(&fs::read_to_string(set_path)?)?
    } else {
        Vec::new()
    };
    let key = (reading_key(reading), context.to_owned());
    for it in &items {
        let r = it["input"].as_str().unwrap_or("");
        let c = it.get("context_text").and_then(Value::as_str).unwrap_or("");
        if (reading_key(r), c.to_owned()) == key {
            bail!("重複: 同じ読み・同じ前の文が既にあります({})", it["index"]);
        }
    }
    let next_idx = format!("rg-{:04}", items.len() + 1);
    let mut entry = json!({
        "index": next_idx,
        "input": reading,
        "context_text": context,
        "expected_output": expected,
        "domain": domain,
    });
    if !note.is_empty() {
        entry["note"] = json!(note);
    }
    items.push(entry.clone());
    fs::write(
        set_path,
        format!("{}\n", serde_json::to_string_pretty(&items)?),
    )?;
    Ok(entry)
}

/// `lenient_score.py`: 表記ゆれ（全半角・かな漢字・送り仮名近似）を正規化して採点する。
pub fn lenient_match(top1: &str, expected: &[String]) -> bool {
    let norm = |s: &str| -> String { s.nfkc().filter(|c| !c.is_whitespace()).collect::<String>() };
    let t = norm(top1);
    expected.iter().any(|e| norm(e) == t)
}

/// `score_context_choice.py`: 文脈選択の組 (`pair`) の両側正解数を集計する。
pub fn score_context_choice(cases_path: &Path, results_path: &Path) -> Result<Value> {
    let cases: Vec<Value> = serde_json::from_str(&fs::read_to_string(cases_path)?)?;
    let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(results_path)?)?;
    let by_idx: HashMap<String, bool> = rows
        .iter()
        .map(|r| (r["index"].to_string(), r["ok"].as_bool().unwrap_or(false)))
        .collect();
    let mut pairs: BTreeMap<String, Vec<bool>> = BTreeMap::new();
    for c in &cases {
        let pair = c["pair"].as_str().unwrap_or("").to_owned();
        let ok = by_idx
            .get(&c["index"].to_string())
            .copied()
            .unwrap_or(false);
        pairs.entry(pair).or_default().push(ok);
    }
    let both = pairs
        .values()
        .filter(|v| v.len() == 2 && v[0] && v[1])
        .count();
    Ok(json!({
        "pairs": pairs.len(),
        "both_correct": both,
        "items": cases.len(),
        "item_correct": by_idx.values().filter(|&&b| b).count(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mk_output(segments: &[&[&str]], total_override: Option<usize>) -> Vec<String> {
        let mut lines = Vec::new();
        for (i, vals) in segments.iter().enumerate() {
            lines.push(format!(
                "---------- Segment {i}/{} [FREE] ----------",
                segments.len()
            ));
            let tot = total_override.unwrap_or(vals.len());
            for (j, v) in vals.iter().enumerate() {
                lines.push(format!("  {j}/{tot} {v}"));
            }
        }
        lines
    }

    fn mk_item(index: usize, context: &str, expected: &[&str]) -> Value {
        json!({
            "index": index,
            "input": "はし",
            "context_text": context,
            "expected_output": expected,
        })
    }

    #[test]
    fn candidate_scoring_and_reachability() {
        let item = json!({"index": 1, "input": "はしをつかう", "expected_output": ["箸を使う", "箸をつかう"]});
        let row = score_case(
            &item,
            &mk_output(&[&["橋を", "箸を"], &["使う", "つかう"]], None),
            5,
        )
        .unwrap();
        assert_eq!(row["ok"], false);
        assert_eq!(row["candidate_coverage"], true);
        assert_eq!(row["diagnostic_stage"], "candidate_present_not_selected");

        let segs = parse_segments(&mk_output(&[&["a", "ab"], &["bc", "c"]], None)).unwrap();
        assert!(reachable(&segs, "abc", None));
        assert!(!reachable(&segs, "ab", None));

        let segs2 = parse_segments(&mk_output(&[&["橋", "箸"], &["で", "を"]], None)).unwrap();
        assert!(reachable(&segs2, "箸を", Some(2)));
        assert!(!reachable(&segs2, "箸を", Some(1)));

        let trunc = score_case(
            &mk_item(1, "", &["箸"]),
            &mk_output(&[&["橋"]], Some(100)),
            5,
        )
        .unwrap();
        assert_eq!(trunc["diagnostic_stage"], "unobserved_candidates");

        for bad in [
            vec![],
            vec![
                "---------- Segment 1/2 [FREE] ----------".into(),
                "  0/1 橋".into(),
            ],
            vec![
                "---------- Segment 0/1 [FREE] ----------".into(),
                "  1/2 橋".into(),
            ],
            vec![
                "---------- Segment 0/1 [HISTORY] ----------".into(),
                "  0/1 橋".into(),
            ],
        ] {
            assert!(parse_segments(&bad).is_err());
        }
    }

    #[test]
    fn comparison_and_condition_validation() {
        let before = json!({
            "data": {"sha256": "same-data"},
            "dictionary": {"path": "old/mozc.data", "sha256": "same-dict"},
            "context": true, "case_isolated": false, "profile_policy": "isolated_temporary"
        });
        let mut after = before.clone();
        after["dictionary"]["path"] = json!("new/mozc.data");
        validate_conditions(&before, &after).unwrap();
        after["context"] = json!(false);
        assert!(validate_conditions(&before, &after).is_err());

        let r1 = score_case(
            &mk_item(1, "", &["箸"]),
            &mk_output(&[&["橋", "箸"]], None),
            5,
        )
        .unwrap();
        let r2 = score_case(&mk_item(2, "", &["箸"]), &mk_output(&[&["箸"]], None), 5).unwrap();
        let a1 = score_case(&mk_item(2, "", &["箸"]), &mk_output(&[&["橋"]], None), 5).unwrap();
        let a2 = score_case(&mk_item(1, "", &["箸"]), &mk_output(&[&["箸"]], None), 5).unwrap();
        let rep = compare_rows(&[r1.clone(), r2], &[a1, a2]).unwrap();
        assert_eq!(
            (
                rep["improved"].clone(),
                rep["regressed"].clone(),
                rep["net_improvement"].clone()
            ),
            (json!(1), json!(1), json!(0))
        );

        let dir = TempDir::new().unwrap();
        let rp = dir.path().join("result.json");
        fs::write(&rp, "[]").unwrap();
        fs::write(
            format!("{}.manifest.json", rp.display()),
            json!({"result_sha256": "wrong"}).to_string(),
        )
        .unwrap();
        assert!(load_result(&rp).is_err());
    }

    #[test]
    fn request_boundary_splitting_and_trace() {
        let wrap = |n: usize, body: &[u8]| -> Vec<u8> {
            [
                format!("[eval_trace] request={n} begin\n").as_bytes(),
                body,
                format!("[eval_trace] request={n} end\n").as_bytes(),
            ]
            .concat()
        };
        let first = b"[time] cands=1\r\n";
        let second = "[asteria] 箸\r\n".as_bytes();
        let raw = [
            b"model init\r\n".as_slice(),
            &wrap(1, first),
            &wrap(2, second),
        ]
        .concat();
        let split = split_scoring_requests(&raw, 2).unwrap();
        assert_eq!(split, vec![(1, first.to_vec()), (2, second.to_vec())]);
        assert!(split_scoring_requests(&wrap(2, b""), 1).is_err());
        assert!(split_scoring_requests(
            &[b"[time] cands=1\n".as_slice(), &wrap(1, b"")].concat(),
            1
        )
        .is_err());

        let log = b"[time] cands=1 llm_on=1\n[asteria] right zenz=-1 llm=-inf lattice score=-inf\n";
        let tr = scoring_trace(log, &["right".into()]).unwrap();
        assert_eq!(tr["expected_in_pool"], json!(true));
        assert_eq!(tr["expected_llm_scored"], json!(false));
        assert_eq!(tr["expected_jointly_selectable"], json!(false));
    }
}
