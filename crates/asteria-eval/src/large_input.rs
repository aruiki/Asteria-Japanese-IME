//! 大量入力ケース生成・キャンペーン実行・コールド起動・操作検証・再現・集計。
//! (`large_input_cases.py`, `large_input_campaign.py`, `large_input_cold.py`,
//!  `large_input_operations.py`, `large_input_replay.py`, `large_input_summary.py`)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use asteria_common::hash::sha256_file;
use asteria_common::kana::{kata_to_hira, nfkc};
use asteria_common::paths::install_dir;
use asteria_common::romaji::RomanTable;

use crate::session_client::Session;
use crate::textproto::{parse_output, ParsedOutput};

const BUSINESS: &[(&str, &str, &str, &str)] = &[
    ("ふぁいる", "ファイル", "なまえ", "名前"),
    ("ふぉるだー", "フォルダー", "ばしょ", "場所"),
    ("しりょう", "資料", "ないよう", "内容"),
    ("せいきゅうしょ", "請求書", "きんがく", "金額"),
    ("みつもりしょ", "見積書", "きげん", "期限"),
    ("けいやくしょ", "契約書", "じょうけん", "条件"),
    ("ぎじろく", "議事録", "ないよう", "内容"),
    ("もうしこみしょ", "申込書", "なまえ", "名前"),
    ("よやく", "予約", "にちじ", "日時"),
    ("かいぎ", "会議", "にってい", "日程"),
    ("めーる", "メール", "あてさき", "宛先"),
    ("れんらくさき", "連絡先", "でんわばんごう", "電話番号"),
    ("おきゃくさま", "お客様", "じゅうしょ", "住所"),
    ("しゃいん", "社員", "しめい", "氏名"),
    ("あかうんと", "アカウント", "せってい", "設定"),
    ("ぱすわーど", "パスワード", "こうしんび", "更新日"),
    ("さーばー", "サーバー", "せってい", "設定"),
    ("ねっとわーく", "ネットワーク", "じょうたい", "状態"),
    ("あぷり", "アプリ", "ばーじょん", "バージョン"),
    ("ぶらうざー", "ブラウザー", "せってい", "設定"),
    ("でーたべーす", "データベース", "なまえ", "名前"),
    ("ばっくあっぷ", "バックアップ", "ほぞんさき", "保存先"),
    ("しゃしん", "写真", "ほぞんさき", "保存先"),
    ("どうが", "動画", "さいず", "サイズ"),
    ("ぷろじぇくと", "プロジェクト", "しんちょく", "進捗"),
    ("さぎょう", "作業", "てじゅん", "手順"),
    ("しょうひん", "商品", "ざいこ", "在庫"),
    ("ちゅうもん", "注文", "すうりょう", "数量"),
    ("はっそう", "発送", "よていび", "予定日"),
    ("のうひん", "納品", "きげん", "期限"),
    ("そうきん", "送金", "きんがく", "金額"),
    ("しはらい", "支払い", "きげん", "期限"),
    ("けいひ", "経費", "ないよう", "内容"),
    ("こうつうひ", "交通費", "きんがく", "金額"),
    ("しゅっちょう", "出張", "にってい", "日程"),
    ("ほうこくしょ", "報告書", "ないよう", "内容"),
    ("しすてむ", "システム", "せってい", "設定"),
    ("えらー", "エラー", "ないよう", "内容"),
    (
        "いんすとーらー",
        "インストーラー",
        "ばーじょん",
        "バージョン",
    ),
    ("にほんごにゅうりょく", "日本語入力", "せってい", "設定"),
];

const PREFIX: &[(&str, &str)] = &[
    ("まず", "まず"),
    ("つぎに", "次に"),
    ("さいごに", "最後に"),
    ("ねんのため", "念のため"),
    ("さぎょうまえに", "作業前に"),
    ("さぎょうごに", "作業後に"),
    ("かいぎのまえに", "会議の前に"),
    ("もういちど", "もう一度"),
];

const ACTION: &[(&str, &str)] = &[
    ("かくにんしてください", "確認してください"),
    ("かくにんしました", "確認しました"),
    ("へんこうしてください", "変更してください"),
    ("へんこうしました", "変更しました"),
    ("とうろくしてください", "登録してください"),
    ("とうろくしました", "登録しました"),
    ("こうしんしてください", "更新してください"),
    ("こうしんしました", "更新しました"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Predicate {
    pub label: String,
    pub value: String,
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LargeInputCase {
    pub id: String,
    pub family: String,
    pub reading: String,
    pub romaji: String,
    pub expected: Vec<String>,
    pub predicates: Vec<Predicate>,
    pub preceding: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surname_targets: Vec<String>,
}

pub fn norm(text: &str) -> String {
    nfkc(text)
}

/// `large_input_cases.py`: 固定 Mozc 原典と業務テンプレートから大量ケースを生成する。
pub fn generate_cases(
    mozc: &Path,
    repo: &Path,
    book_limit: usize,
    name_limit: usize,
    seed: u64,
) -> Result<(Vec<LargeInputCase>, Value)> {
    let table_path = mozc.join("data/preedit/romanji-hiragana.tsv");
    let table = RomanTable::from_tsv_file(&table_path)?;
    let mut cases = Vec::new();
    let mut excluded: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();

    let mut add = |raw_reading: &str,
                   family: &str,
                   expected: Vec<String>,
                   predicates: Vec<Predicate>,
                   preceding: &str,
                   surname_targets: Vec<String>| {
        let reading = kata_to_hira(raw_reading);
        let r_len = reading.chars().count();
        if reading.is_empty() || r_len > 160 {
            *excluded.entry("empty_or_over_160".into()).or_insert(0) += 1;
            return;
        }
        let Ok(keys) = table.romanize(&reading, true) else {
            *excluded.entry("unsupported_for_romaji".into()).or_insert(0) += 1;
            return;
        };
        if !seen.insert(reading.clone()) {
            *excluded.entry("duplicate_reading".into()).or_insert(0) += 1;
            return;
        }
        let id = asteria_common::hash::sha256_bytes(format!("{family}\0{reading}").as_bytes())
            [..20]
            .to_owned();
        cases.push(LargeInputCase {
            id,
            family: family.to_owned(),
            reading,
            romaji: keys,
            expected,
            predicates,
            preceding: preceding.to_owned(),
            surname_targets,
        });
    };

    for name in ["daily", "idioms", "nuance", "notation", "typo", "homophone"] {
        let p = repo.join(format!("eval/sets/asteria-{name}.json"));
        sources.insert(p.to_string_lossy().into_owned(), sha256_file(&p)?);
        let rows: Vec<Value> = serde_json::from_str(&fs::read_to_string(&p)?)?;
        let fam = format!("curated-{name}");
        for r in rows {
            let exp: Vec<String> = r["expected_output"]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let prec = r.get("context_text").and_then(Value::as_str).unwrap_or("");
            add(
                r["input"].as_str().unwrap_or(""),
                &fam,
                exp,
                Vec::new(),
                prec,
                Vec::new(),
            );
        }
    }

    for &(nr, nv, pr, pv) in BUSINESS {
        for &(xr, xv) in PREFIX {
            for &(ar, av) in ACTION {
                add(
                    &format!("{xr}{nr}の{pr}を{ar}"),
                    "business",
                    vec![format!("{xv}{nv}の{pv}を{av}")],
                    Vec::new(),
                    "",
                    Vec::new(),
                );
            }
        }
    }

    for name in ["oss.tsv", "regression.tsv"] {
        let p = mozc.join("data/test/quality_regression_test").join(name);
        sources.insert(p.to_string_lossy().into_owned(), sha256_file(&p)?);
        let mut grouped: BTreeMap<String, Vec<Predicate>> = BTreeMap::new();
        for line in fs::read_to_string(&p)?.lines() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 4 {
                continue;
            }
            grouped
                .entry(parts[1].to_owned())
                .or_default()
                .push(Predicate {
                    label: parts[0].to_owned(),
                    value: parts[2].to_owned(),
                    command: parts[3].to_owned(),
                });
        }
        for (reading, preds) in grouped {
            add(
                &reading,
                "upstream-regression",
                Vec::new(),
                preds,
                "",
                Vec::new(),
            );
        }
    }

    let myoji = mozc.join("data/test/quality_regression_test/myoji.tsv");
    sources.insert(myoji.to_string_lossy().into_owned(), sha256_file(&myoji)?);
    let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in fs::read_to_string(&myoji)?.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 4 {
            names
                .entry(parts[1].to_owned())
                .or_default()
                .push(parts[2].to_owned());
        }
    }
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut name_keys: Vec<String> = names.keys().cloned().collect();
    name_keys.shuffle(&mut rng);
    for r in name_keys.into_iter().take(name_limit) {
        let targets = names.remove(&r).unwrap_or_default();
        add(&r, "surname", Vec::new(), Vec::new(), "", targets);
    }

    let sent_path = mozc.join("data/test/stress_test/sentences.txt");
    sources.insert(
        sent_path.to_string_lossy().into_owned(),
        sha256_file(&sent_path)?,
    );
    let mut readings: Vec<String> = fs::read_to_string(&sent_path)?
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    readings.shuffle(&mut rng);
    for r in readings.into_iter().take(book_limit) {
        add(
            &r,
            "natural-reading",
            Vec::new(),
            Vec::new(),
            "",
            Vec::new(),
        );
    }

    let mut fam_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut oracle_cases = 0usize;
    for c in &cases {
        *fam_counts.entry(c.family.clone()).or_insert(0) += 1;
        if !c.expected.is_empty()
            || c.predicates
                .iter()
                .any(|p| p.command == "Conversion Match" || p.command == "Conversion Not Match")
        {
            oracle_cases += 1;
        }
    }
    let manifest = json!({
        "seed": seed,
        "source_sha256": sources,
        "romanji_table_sha256": sha256_file(&table_path)?,
        "unique_inputs": cases.len(),
        "families": fam_counts,
        "excluded": excluded,
        "meaning_oracle_cases": oracle_cases,
        "business_template_families": BUSINESS.len(),
        "native_tsf": false,
    });
    Ok((cases, manifest))
}

/// `large_input_cases.cpu_subset`: 層化抽出（元の配列を変更しない）。
pub fn cpu_subset(cases: &[LargeInputCase], seed: u64) -> Vec<LargeInputCase> {
    let mut groups: BTreeMap<String, Vec<LargeInputCase>> = BTreeMap::new();
    for c in cases {
        groups.entry(c.family.clone()).or_default().push(c.clone());
    }
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut result = Vec::new();
    for (family, mut group) in groups {
        group.shuffle(&mut rng);
        let limit = match family.as_str() {
            "business" => 200,
            "upstream-regression" => 100,
            "surname" => 150,
            "natural-reading" => 287,
            _ => group.len(),
        };
        result.extend(group.into_iter().take(limit));
    }
    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    #[serde(flatten)]
    pub parsed: ParsedOutput,
    pub ms: f64,
}

/// 1 ケースの入力・Space・確定イベントを不具合 (`defects`) と要確認 (`reviews`) に分類する。
pub fn assess_events(
    case: &LargeInputCase,
    typed: &EventRecord,
    space: &EventRecord,
    commit: Option<&EventRecord>,
    check_commit: bool,
) -> Result<(Vec<String>, Vec<String>)> {
    if check_commit && commit.is_none() {
        bail!("commit event missing");
    }
    let mut defects = Vec::new();
    let mut reviews = BTreeSet::new();

    if norm(&typed.parsed.key) != norm(&case.reading) {
        defects.push("typed_reading_mismatch".into());
    }
    if space.parsed.value.is_empty() && space.parsed.result.is_empty() {
        defects.push("space_empty".into());
    }
    if check_commit {
        if let Some(c) = commit {
            if !space.parsed.value.is_empty() && norm(&c.parsed.result) != norm(&space.parsed.value)
            {
                defects.push("commit_different_from_preedit".into());
            }
        }
    }
    if space.ms > 3000.0 {
        defects.push("space_over_3000ms".into());
    }

    let value = if space.parsed.value.is_empty() {
        &space.parsed.result
    } else {
        &space.parsed.value
    };
    let n_val = norm(value);
    if !case.expected.is_empty() && !case.expected.iter().any(|v| norm(v) == n_val) {
        reviews.insert("expected_surface_mismatch".into());
    }
    for p in &case.predicates {
        if p.command == "Conversion Match" && !n_val.contains(&norm(&p.value)) {
            reviews.insert("upstream_match_missing".into());
        } else if p.command == "Conversion Not Match" && n_val.contains(&norm(&p.value)) {
            reviews.insert("upstream_forbidden_surface".into());
        }
    }
    if !case.surname_targets.is_empty() {
        let mut visible: HashSet<&str> = HashSet::new();
        for c in &typed.parsed.candidates {
            visible.insert(c.as_str());
        }
        for c in &space.parsed.candidates {
            visible.insert(c.as_str());
        }
        if !case
            .surname_targets
            .iter()
            .any(|v| visible.contains(v.as_str()))
        {
            reviews.insert("surname_not_observed_in_exposed_candidates".into());
        } else if case
            .surname_targets
            .iter()
            .any(|v| !visible.contains(v.as_str()))
        {
            reviews.insert("surname_some_variants_not_observed".into());
        }
    }
    Ok((defects, reviews.into_iter().collect()))
}

/// `large_input_cold.py`: `asteria_ai_status.txt` の読み取り（PermissionError は未確認扱い）。
pub fn status_text(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            "status_read_error=permission_denied\n".into()
        }
        Err(e) => format!("status_read_error={e}\n"),
    }
}

pub fn ready_status(text: &str, pid: u32, cpu: bool) -> bool {
    let mut fields: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            fields.insert(k, v);
        }
    }
    fields.get("pid").copied() == Some(pid.to_string().as_str())
        && matches!(fields.get("state").copied(), Some("llm" | "zenz"))
        && (!cpu || fields.get("device").copied() == Some("CPU"))
}

pub fn summarize_cold(
    rows: &[Value],
    cases: &[Value],
    completed_phases: &[&str],
    error: Option<&str>,
) -> Value {
    let complete = completed_phases == ["no-warmup", "loaded"]
        && rows.len() == 2 * cases.len()
        && !cases.is_empty()
        && error.is_none();
    let defects: usize = rows
        .iter()
        .filter_map(|r| r["defects"].as_array().map(Vec::len))
        .sum();
    let mismatches = rows
        .iter()
        .filter(|r| {
            r["reviews"].as_array().is_some_and(|a| {
                a.iter()
                    .any(|x| x.as_str() == Some("expected_surface_mismatch"))
            })
        })
        .count();
    let max_space = rows
        .iter()
        .filter_map(|r| r["events"]["space"]["ms"].as_f64())
        .reduce(f64::max);
    let max_key = rows
        .iter()
        .flat_map(|r| {
            r["key_events"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|e| e["ms"].as_f64())
        })
        .reduce(f64::max);
    json!({
        "complete": complete,
        "input_path_checks_passed": complete && defects == 0,
        "defects": defects,
        "reference_mismatches": mismatches,
        "reference_matches": rows.len().saturating_sub(mismatches),
        "scenarios": rows.len(),
        "error": error,
        "max_space_ms": max_space,
        "max_key_ms": max_key,
    })
}

pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    let idx = ((p * (ordered.len() as f64)).ceil() as usize).saturating_sub(1);
    Some(ordered[idx.min(ordered.len() - 1)])
}

/// `large_input_summary.py`: 原票と整合性を検査しながら集計サマリと疑義一覧を作成する。
pub fn summarize_campaign(cases: &[LargeInputCase], rows: &[Value]) -> Result<(Value, Vec<Value>)> {
    let mut by_id: HashMap<&str, &LargeInputCase> = HashMap::new();
    let mut readings = HashSet::new();
    for c in cases {
        if by_id.insert(c.id.as_str(), c).is_some() {
            bail!("duplicate corpus ID");
        }
        if !readings.insert(c.reading.as_str()) {
            bail!("duplicate corpus reading");
        }
    }
    let mut seen = HashSet::new();
    let mut modes: BTreeMap<String, usize> = BTreeMap::new();
    let mut defects: BTreeMap<String, usize> = BTreeMap::new();
    let mut reviews: BTreeMap<String, usize> = BTreeMap::new();
    let mut families: BTreeMap<String, usize> = BTreeMap::new();
    let mut latencies: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut findings = Vec::new();
    let mut primary_ids: HashSet<String> = HashSet::new();
    let mut gold_ids: HashSet<String> = HashSet::new();
    let mut gold_flags: HashSet<String> = HashSet::new();

    for row in rows {
        let sid = row["scenario_id"].as_str().context("scenario_id")?;
        if !seen.insert(sid.to_owned()) {
            bail!("duplicate scenario: {sid}");
        }
        let cid = row["case_id"].as_str().context("case_id")?;
        let case = by_id.get(cid).copied().context("unknown case_id")?;
        let row_exp: Vec<String> = row["expected"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
        if row["reading"].as_str() != Some(case.reading.as_str())
            || row["family"].as_str() != Some(case.family.as_str())
            || row_exp != case.expected
        {
            bail!("row differs from pinned corpus: {sid}");
        }
        let mode = row["mode"].as_str().context("mode")?;
        if sid != format!("{}:{}", case.id, mode) {
            bail!("scenario ID differs from mode");
        }
        *modes.entry(mode.to_owned()).or_insert(0) += 1;
        let row_defects = row["defects"].as_array().cloned().unwrap_or_default();
        let row_reviews = row["reviews"].as_array().cloned().unwrap_or_default();
        for d in &row_defects {
            if let Some(s) = d.as_str() {
                *defects.entry(s.to_owned()).or_insert(0) += 1;
            }
        }
        for r in &row_reviews {
            if let Some(s) = r.as_str() {
                *reviews.entry(s.to_owned()).or_insert(0) += 1;
            }
        }
        if mode == "immediate" {
            primary_ids.insert(case.id.clone());
            *families.entry(case.family.clone()).or_insert(0) += 1;
            let has_gold = !case.expected.is_empty()
                || case.predicates.iter().any(|p| {
                    p.command == "Conversion Match" || p.command == "Conversion Not Match"
                });
            if has_gold {
                gold_ids.insert(case.id.clone());
                if row_reviews.iter().any(|v| {
                    matches!(
                        v.as_str(),
                        Some(
                            "expected_surface_mismatch"
                                | "upstream_match_missing"
                                | "upstream_forbidden_surface"
                        )
                    )
                }) {
                    gold_flags.insert(case.id.clone());
                }
            }
        }
        if let Some(evs) = row["events"].as_object() {
            for (ev_name, ev) in evs {
                if let Some(ms) = ev.get("ms").and_then(Value::as_f64) {
                    latencies
                        .entry(format!("{mode}:{ev_name}"))
                        .or_default()
                        .push(ms);
                }
            }
        }
        if !row_defects.is_empty() || !row_reviews.is_empty() {
            let mut f = row.clone();
            f["predicates"] = json!(case.predicates);
            f["surname_targets"] = json!(case.surname_targets);
            findings.push(f);
        }
    }

    let lat_json: BTreeMap<String, Value> = latencies
        .into_iter()
        .map(|(k, v)| {
            let max_v = v.iter().copied().reduce(f64::max);
            (
                k,
                json!({
                    "n": v.len(),
                    "p50": percentile(&v, 0.5),
                    "p95": percentile(&v, 0.95),
                    "p99": percentile(&v, 0.99),
                    "max": max_v,
                }),
            )
        })
        .collect();

    Ok((
        json!({
            "unique_inputs_requested": cases.len(),
            "unique_primary_inputs_executed": primary_ids.len(),
            "primary_complete": primary_ids.len() == by_id.len(),
            "scenario_counts": modes,
            "scenarios_total": rows.len(),
            "primary_families": families,
            "primary_meaning_oracle_cases": gold_ids.len(),
            "primary_meaning_oracle_flagged": gold_flags.len(),
            "defect_flags": defects,
            "review_flags": reviews,
            "latency_ms": lat_json,
            "native_tsf": false,
            "strict_mismatch_is_not_automatically_a_semantic_bug": true,
        }),
        findings,
    ))
}

/// `large_input_replay.py`: 選択された ID 集合を検証・抽出する。
pub fn selected_cases(
    cases: &[LargeInputCase],
    wanted: &HashSet<String>,
) -> Result<Vec<LargeInputCase>> {
    let selected: Vec<LargeInputCase> = cases
        .iter()
        .filter(|c| wanted.contains(&c.id))
        .cloned()
        .collect();
    let got_ids: HashSet<String> = selected.iter().map(|c| c.id.clone()).collect();
    if wanted.is_empty() || got_ids != *wanted {
        bail!("empty or unknown selected IDs");
    }
    if selected.len() != wanted.len() {
        bail!("duplicate selected IDs in corpus");
    }
    Ok(selected)
}

/// `large_input_operations.py`: 基本ローマ字・未完の n・編集・文字種(F7/F10)・取消ケースを生成する。
pub fn operation_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    for (keys, reading) in [
        ("shinbun", "しんぶん"),
        ("nihon", "にほん"),
        ("shashin", "しゃしん"),
        ("chotto", "ちょっと"),
        ("gakkou", "がっこう"),
        ("kitte", "きって"),
        ("shinnyuu", "しんゆう"),
        ("shinnnyuu", "しんにゅう"),
        ("shin'nyuu", "しんにゅう"),
        ("shin'you", "しんよう"),
        ("shinnbun", "しんぶん"),
    ] {
        cases.push(json!({
            "label": format!("ordinary-romaji:{keys}"),
            "commands": [format!("SEND_KEYS\t{keys}"), "SEND_KEY\tSpace"],
            "reading": reading,
            "display": Value::Null,
        }));
    }
    for (label, cmds) in [
        (
            "incomplete-romaji-backspace",
            vec!["SEND_KEYS\tsus", "SEND_KEY\tBackSpace", "SEND_KEYS\tshi"],
        ),
        (
            "kana-backspace",
            vec!["SEND_KEYS\tsushi", "SEND_KEY\tBackSpace", "SEND_KEYS\tshi"],
        ),
        (
            "cursor-edit",
            vec![
                "SEND_KEYS\tsushi",
                "SEND_KEY\tLeft",
                "SEND_KEY\tBackSpace",
                "SEND_KEYS\tsu",
            ],
        ),
        (
            "cancel-conversion",
            vec!["SEND_KEYS\tsushi", "SEND_KEY\tSpace", "SEND_KEY\tESC"],
        ),
    ] {
        cases.push(
            json!({"label": label, "commands": cmds, "reading": "すし", "display": Value::Null}),
        );
    }
    for (keys, _r, kata) in [
        ("sushi", "すし", Some("スシ")),
        ("gakkou", "がっこう", Some("ガッコウ")),
        ("chotto", "ちょっと", Some("チョット")),
        ("shin'you", "しんよう", Some("シンヨウ")),
        ("asteria", "あsてりあ", None),
    ] {
        cases.push(json!({
            "label": format!("f10:{keys}"),
            "commands": [format!("SEND_KEYS\t{keys}"), "SEND_KEY\tF10"],
            "reading": Value::Null,
            "display": keys,
        }));
        if let Some(k) = kata {
            cases.push(json!({
                "label": format!("f7:{keys}"),
                "commands": [format!("SEND_KEYS\t{keys}"), "SEND_KEY\tF7"],
                "reading": Value::Null,
                "display": k,
            }));
        }
    }
    for keys in [
        "https://github.com",
        "name@example.com",
        "Asteria2026",
        "C:/Users/test/file.txt",
        "1.5GB",
        "abc_123-xyz",
    ] {
        cases.push(json!({
            "label": format!("f10-mixed:{keys}"),
            "commands": [format!("SEND_KEYS\t{keys}"), "SEND_KEY\tF10"],
            "reading": Value::Null,
            "display": keys,
        }));
    }
    for keys in ["sushi", "chotto", "shinbun", "12345"] {
        cases.push(json!({
            "label": format!("escape-clear:{keys}"),
            "commands": [format!("SEND_KEYS\t{keys}"), "SEND_KEY\tESC"],
            "reading": "",
            "display": "",
        }));
    }
    cases
}

/// `large_input_campaign.py` の主要キャンペーン実行。
#[allow(clippy::too_many_arguments)]
pub fn run_campaign(
    exe: &Path,
    cases_path: &Path,
    install: Option<&Path>,
    out: &Path,
    limit: Option<usize>,
    interaction_samples: usize,
    settle_secs: f64,
    key_interval_secs: f64,
    cpu: bool,
    warm_secs: f64,
) -> Result<()> {
    let exe = fs::canonicalize(exe)?;
    let inst = install
        .map(fs::canonicalize)
        .transpose()?
        .unwrap_or_else(install_dir);
    fs::create_dir_all(out)?;
    let out = fs::canonicalize(out)?;
    let mut cases: Vec<LargeInputCase> = fs::read_to_string(cases_path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if let Some(lim) = limit {
        cases.truncate(lim);
    }
    if cases.is_empty() {
        bail!("empty cases");
    }

    let mut env: Vec<(String, String)> = env::vars()
        .filter(|(k, _)| !k.starts_with("ASTERIA_"))
        .collect();
    env.push((
        "ASTERIA_MODEL_DIR".into(),
        inst.to_string_lossy().into_owned(),
    ));
    env.push((
        "ASTERIA_RUNTIME_DIR".into(),
        inst.to_string_lossy().into_owned(),
    ));
    env.push(("ASTERIA_LM_TIME".into(), "1".into()));
    if cpu {
        env.push(("ASTERIA_LM_DEVICE".into(), "cpu".into()));
    }

    let prof_hash = &asteria_common::hash::sha256_bytes(out.to_string_lossy().as_bytes())[..20];
    let profile = env::temp_dir().join(format!("asteria-campaign-{prof_hash}"));
    fs::create_dir_all(&profile)?;

    let mut session = Session::spawn(&exe, &env, &profile, &out.join("engine.stderr.txt"), false)?;
    let raw_gz = File::create(out.join("raw-events.jsonl.gz"))?;
    let mut raw_writer = BufWriter::new(GzEncoder::new(raw_gz, Compression::default()));
    let results_f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("results.jsonl"))?;
    let mut res_writer = BufWriter::new(results_f);

    let send_ev = |sess: &mut Session, cmd: &str| -> Result<EventRecord> {
        let ev = if cmd == "SEND_KEY\tSpace" || cmd == "SEND_KEY\tTab" {
            let conv = sess.send_conversion(cmd, 90.0)?;
            (conv.final_event.output, conv.final_event.response_ms)
        } else {
            let e = sess.send(cmd)?;
            (e.output, e.response_ms)
        };
        let parsed = parse_output(&ev.0)?;
        Ok(EventRecord { parsed, ms: ev.1 })
    };

    session.send("SET_CONFIG\tasteria_lm_quality\tASTERIA_LM_STANDARD\nSET_CONFIG\tasteria_lm_model\tASTERIA_LM_MODEL_LLM\nSEND_KEY\tON\nSEND_KEYS\tai")?;
    thread::sleep(Duration::from_secs_f64(warm_secs));
    session.send("SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA")?;

    let started = Instant::now();
    for (idx, case) in cases.iter().enumerate() {
        session.send("SEND_KEY\tESC\nSEND_KEY\tESC\nRESET_CONTEXT\nSEND_KEY\tON\nSWITCH_COMPOSITION_MODE\tHIRAGANA")?;
        let typed = if !case.preceding.is_empty() {
            let mut chars = case.romaji.chars();
            let first = chars.next().context("empty romaji")?;
            let rest: String = chars.collect();
            let prec_json = serde_json::to_string(&case.preceding)?;
            let ev = send_ev(
                &mut session,
                &format!("SEND_KEY_WITH_OPTION\t{first}\tcontext.preceding_text={prec_json}"),
            )?;
            if rest.is_empty() {
                ev
            } else {
                send_ev(&mut session, &format!("SEND_KEYS\t{rest}"))?
            }
        } else {
            send_ev(&mut session, &format!("SEND_KEYS\t{}", case.romaji))?
        };
        let space = send_ev(&mut session, "SEND_KEY\tSpace")?;
        let commit = send_ev(&mut session, "SEND_KEY\tEnter")?;
        let (defects, reviews) = assess_events(case, &typed, &space, Some(&commit), true)?;
        let row = json!({
            "scenario_id": format!("{}:immediate", case.id),
            "case_id": case.id,
            "family": case.family,
            "reading": case.reading,
            "mode": "immediate",
            "expected": case.expected,
            "defects": defects,
            "reviews": reviews,
            "events": {"typed": typed, "space": space, "commit": commit},
        });
        serde_json::to_writer(&mut res_writer, &row)?;
        res_writer.write_all(b"\n")?;
        serde_json::to_writer(&mut raw_writer, &row)?;
        raw_writer.write_all(b"\n")?;
        if (idx + 1) % 100 == 0 || idx + 1 == cases.len() {
            res_writer.flush()?;
            let prog = json!({"phase": "immediate", "done": idx + 1, "total": cases.len(), "seconds": started.elapsed().as_secs()});
            fs::write(
                out.join("progress.json"),
                serde_json::to_string_pretty(&prog)?,
            )?;
        }
    }
    let _ = (interaction_samples, settle_secs, key_interval_secs);
    res_writer.flush()?;
    raw_writer.flush()?;
    session.close(false)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_common::romaji::RomanTable;
    use tempfile::TempDir;

    fn sample_case() -> LargeInputCase {
        LargeInputCase {
            id: "c1".into(),
            family: "curated".into(),
            reading: "でんきをけしてね".into(),
            romaji: "denkiwokeshitene".into(),
            expected: vec!["電気を消してね".into()],
            predicates: Vec::new(),
            preceding: String::new(),
            surname_targets: Vec::new(),
        }
    }

    fn mk_ev(key: &str, value: &str, result: &str, cands: &[&str]) -> EventRecord {
        EventRecord {
            parsed: ParsedOutput {
                key: key.into(),
                value: value.into(),
                result: result.into(),
                candidates: cands.iter().map(|s| (*s).to_owned()).collect(),
                ..Default::default()
            },
            ms: 20.0,
        }
    }

    #[test]
    fn assess_events_separates_transport_defects_and_semantic_reviews() {
        let mut c = sample_case();
        let typed_bad = mk_ev("でんきをけして", "", "", &[]);
        let space_ok = mk_ev("", "電気を消してね", "", &[]);
        let commit_ok = mk_ev("", "", "電気を消してね", &[]);
        let (def, rev) = assess_events(&c, &typed_bad, &space_ok, Some(&commit_ok), true).unwrap();
        assert!(def.contains(&"typed_reading_mismatch".to_owned()));
        assert!(rev.is_empty());

        let typed_ok = mk_ev("でんきをけしてね", "", "", &[]);
        let space_wrong = mk_ev("", "電気を消して寝", "", &[]);
        let (def2, rev2) = assess_events(
            &c,
            &typed_ok,
            &space_wrong,
            Some(&mk_ev("", "", "電気を消して寝", &[])),
            true,
        )
        .unwrap();
        assert!(def2.is_empty());
        assert_eq!(rev2, vec!["expected_surface_mismatch"]);

        c.surname_targets = vec!["道島".into(), "道嶋".into()];
        let (_, rev3) = assess_events(
            &c,
            &typed_ok,
            &mk_ev("", "電気を消してね", "", &["道島"]),
            Some(&commit_ok),
            true,
        )
        .unwrap();
        assert_eq!(rev3, vec!["surname_some_variants_not_observed"]);
    }

    #[test]
    fn summary_and_replay_enforce_evidence_integrity() {
        let c = sample_case();
        let row = json!({
            "scenario_id": "c1:immediate", "case_id": "c1", "reading": c.reading,
            "family": c.family, "expected": c.expected, "mode": "immediate",
            "defects": [], "reviews": [], "events": {}
        });
        assert!(summarize_campaign(std::slice::from_ref(&c), &[row.clone(), row.clone()]).is_err());

        let replay = json!({
            "scenario_id": "c1:settled", "case_id": "c1", "reading": c.reading,
            "family": c.family, "expected": c.expected, "mode": "settled",
            "defects": [], "reviews": [], "events": {}
        });
        let (sum, _) = summarize_campaign(std::slice::from_ref(&c), &[row, replay]).unwrap();
        assert_eq!(sum["unique_primary_inputs_executed"], json!(1));
        assert_eq!(sum["scenarios_total"], json!(2));

        let unknown: HashSet<String> = ["unknown".into()].into_iter().collect();
        assert!(selected_cases(std::slice::from_ref(&c), &unknown).is_err());
    }

    #[test]
    fn cold_status_and_cpu_subset_are_deterministic() {
        let dir = TempDir::new().unwrap();
        assert_eq!(status_text(&dir.path().join("missing")), "");
        let status = "state=zenz\npid=123\ndevice=CPU\n";
        assert!(ready_status(status, 123, true));
        assert!(!ready_status(status, 124, true));
        assert!(!ready_status(&status.replace("CPU", "GPU"), 123, true));

        let mut cases: Vec<LargeInputCase> = (0..220)
            .map(|i| LargeInputCase {
                id: format!("{i}"),
                family: "business".into(),
                reading: format!("r{i}"),
                romaji: format!("r{i}"),
                expected: Vec::new(),
                predicates: Vec::new(),
                preceding: String::new(),
                surname_targets: Vec::new(),
            })
            .collect();
        cases.push(LargeInputCase {
            id: "curated".into(),
            family: "curated-daily".into(),
            reading: "rc".into(),
            romaji: "rc".into(),
            expected: Vec::new(),
            predicates: Vec::new(),
            preceding: String::new(),
            surname_targets: Vec::new(),
        });
        let s1 = cpu_subset(&cases, 20261013);
        let s2 = cpu_subset(&cases, 20261013);
        assert_eq!(s1, s2);
        assert_eq!(s1.len(), 201);

        let table = RomanTable::from_tsv_str("a\tあ\nn\tん\nnn\tん\nkka\tっ\tk\n.\t。\n");
        assert_eq!(table.romanize("あんっ。", true).unwrap(), "an'xtu.");
    }
}
