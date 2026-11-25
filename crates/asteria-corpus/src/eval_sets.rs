//! 評価セット・文脈選択セット・固有名詞辞書・ATOK 調整セット・表生成。
//! (`build_eval_sets.py`, `build_context_choice.py`, `build_context_pairs.py`, `build_kana_preference.py`,
//!  `build_name_lexicon.py`, `fetch_wikipedia_new_articles.py`, `import_sonnet_choice.py`,
//!  `gen_context_defaults.py`, `gen_notation_table.py`, `make_atok_sets.py`, `make_same_mistake_set.py`)

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use bzip2::read::BzDecoder;
use quick_xml::events::Event;
use quick_xml::Reader as XmlReader;
use regex::Regex;
use serde_json::{json, Value};
use sudachi::analysis::stateless_tokenizer::StatelessTokenizer;
use sudachi::analysis::Tokenize;
use sudachi::dic::dictionary::JapaneseDictionary;
use sudachi::prelude::*;

use crate::frequency::load_sudachi_dict;
use asteria_common::hash::{sha256_bytes, sha256_file};
use asteria_common::http::fetch_bytes;
use asteria_common::kana::{kata_to_hira, nfkc};

pub const KANA_NATURAL: &[(&str, &str)] = &[
    ("事", "こと"),
    ("時", "とき"),
    ("所", "ところ"),
    ("物", "もの"),
    ("為", "ため"),
    ("様", "よう"),
    ("通り", "とおり"),
    ("殆ど", "ほとんど"),
    ("共に", "ともに"),
    ("全て", "すべて"),
    ("様々", "さまざま"),
    ("又", "また"),
    ("及び", "および"),
    ("但し", "ただし"),
    ("既に", "すでに"),
    ("更に", "さらに"),
    ("未だ", "いまだ"),
    ("先ず", "まず"),
    ("是非", "ぜひ"),
    ("下さい", "ください"),
    ("頂く", "いただく"),
    ("頂き", "いただき"),
    ("致し", "いたし"),
    ("致す", "いたす"),
    ("出来る", "できる"),
    ("出来", "でき"),
    ("有難う", "ありがとう"),
    ("有り難う", "ありがとう"),
    ("宜しく", "よろしく"),
    ("可哀想", "かわいそう"),
    ("凄く", "すごく"),
    ("凄い", "すごい"),
    ("沢山", "たくさん"),
    ("勿論", "もちろん"),
    ("何故", "なぜ"),
    ("色々", "いろいろ"),
    ("一緒", "いっしょ"),
    ("身体", "からだ"),
    ("体", "からだ"),
    ("等", "など"),
    ("迄", "まで"),
    ("位", "くらい"),
    ("程", "ほど"),
    ("毎", "ごと"),
    ("頃", "ころ"),
];

pub fn split_of(doc_key: &str) -> &'static str {
    let bytes = sha256_bytes(doc_key.as_bytes());
    let last_hex = bytes.as_bytes().last().copied().unwrap_or(b'0');
    let nibble = match last_hex {
        b'0'..=b'9' => last_hex - b'0',
        b'a'..=b'f' => last_hex - b'a' + 10,
        _ => 0,
    };
    if nibble % 2 == 0 {
        "judge"
    } else {
        "dev"
    }
}

pub fn width_variants(text: &str) -> BTreeSet<String> {
    let half = nfkc(text);
    let to_full = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_digit() || c == '%' {
                    char::from_u32(c as u32 + 0xfee0).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect()
    };
    let to_half = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if ('０'..='９').contains(&c) || c == '％' {
                    char::from_u32(c as u32 - 0xfee0).unwrap_or(c)
                } else {
                    c
                }
            })
            .collect()
    };
    let mut set = BTreeSet::new();
    set.insert(text.to_owned());
    set.insert(to_half(text));
    set.insert(to_full(text));
    if half != text {
        set.insert(to_full(&half));
        set.insert(half);
    }
    set
}

pub fn kana_variants(tokens: &[String], limit: usize) -> BTreeSet<String> {
    let k2k: HashMap<&str, &str> = KANA_NATURAL.iter().copied().collect();
    let mut k2kanji: HashMap<&str, BTreeSet<&str>> = HashMap::new();
    for &(kanji, kana) in KANA_NATURAL {
        k2kanji.entry(kana).or_default().insert(kanji);
    }
    let mut merged: Vec<String> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if i + 1 < tokens.len() {
            let pair = format!("{}{}", tokens[i], tokens[i + 1]);
            if k2k.contains_key(pair.as_str()) || k2kanji.contains_key(pair.as_str()) {
                merged.push(pair);
                i += 2;
                continue;
            }
        }
        merged.push(tokens[i].clone());
        i += 1;
    }
    let mut out = vec![String::new()];
    for token in &merged {
        let mut opts: BTreeSet<&str> = BTreeSet::new();
        opts.insert(token.as_str());
        if let Some(&kana) = k2k.get(token.as_str()) {
            opts.insert(kana);
        }
        if let Some(kanjis) = k2kanji.get(token.as_str()) {
            for &kj in kanjis {
                opts.insert(kj);
            }
        }
        let mut next = Vec::new();
        for prefix in &out {
            for &o in &opts {
                next.push(format!("{prefix}{o}"));
                if next.len() >= limit {
                    break;
                }
            }
            if next.len() >= limit {
                break;
            }
        }
        out = next;
    }
    out.into_iter().collect()
}

pub fn expected_for(text: &str, tokens: &[String]) -> Vec<String> {
    let mut variants: BTreeSet<String> = BTreeSet::new();
    let mut base_set = kana_variants(tokens, 16);
    base_set.insert(text.to_owned());
    for v in base_set {
        for w in width_variants(&v) {
            variants.insert(w);
        }
    }
    let mut out: Vec<String> = variants.into_iter().collect();
    out.sort_by(|a, b| (a != text).cmp(&(b != text)).then_with(|| a.cmp(b)));
    out
}

pub fn split_sentences(text: &str) -> Vec<String> {
    let clean = text.replace('\n', "");
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in clean.chars() {
        cur.push(ch);
        if matches!(ch, '。' | '！' | '？' | '!' | '?') {
            let trimmed = cur.trim().to_owned();
            if !trimmed.is_empty() {
                out.push(trimmed);
            }
            cur.clear();
        }
    }
    let rem = cur.trim().to_owned();
    if !rem.is_empty() {
        out.push(rem);
    }
    out
}

pub fn split_chunks(sentence: &str, low: usize, high: usize) -> Vec<String> {
    if sentence.chars().count() <= high {
        return vec![sentence.to_owned()];
    }
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut piece = String::new();
    for ch in sentence.chars() {
        piece.push(ch);
        if ch == '、' {
            if buf.chars().count() + piece.chars().count() > high && buf.chars().count() >= low {
                out.push(std::mem::take(&mut buf));
            }
            buf.push_str(&piece);
            piece.clear();
        }
    }
    if !piece.is_empty() {
        if buf.chars().count() + piece.chars().count() > high && buf.chars().count() >= low {
            out.push(std::mem::take(&mut buf));
        }
        buf.push_str(&piece);
    }
    if buf.chars().count() >= low {
        out.push(buf);
    }
    out
}

pub fn clean_wikitext(text: &str) -> Result<String> {
    let re1 = Regex::new(r"\{\{[^{}]*\}\}")?;
    let re2 = Regex::new(r"\[\[(?:[^|\]]*\|)?([^\]]*)\]\]")?;
    let re3 = Regex::new(r"\[https?://\S+ ([^\]]*)\]")?;
    let re4 = Regex::new(r"<[^>]+>|'''?|==+[^=]+==+")?;
    let t = re1.replace_all(text, "");
    let t = re2.replace_all(&t, "$1");
    let t = re3.replace_all(&t, "$1");
    let t = re4.replace_all(&t, "");
    let mut out = String::new();
    for line in t.lines() {
        if line.is_empty()
            || line.starts_with('*')
            || line.starts_with('{')
            || line.starts_with('|')
            || line.starts_with('!')
            || line.starts_with(':')
            || line.starts_with(';')
            || line.starts_with('#')
            || line.starts_with("[[")
        {
            continue;
        }
        out.push_str(line);
    }
    Ok(out)
}

pub fn surface_reading(surface: &str, reading: &str) -> String {
    if !surface.is_empty()
        && surface
            .chars()
            .all(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
    {
        return nfkc(surface);
    }
    if surface == "%" || surface == "％" {
        return "パーセント".into();
    }
    if reading.is_empty()
        || reading == "*"
        || !reading
            .chars()
            .any(|c| ('ァ'..='ヶ').contains(&c) || c == 'ー')
    {
        return surface.to_owned();
    }
    reading.to_owned()
}

pub fn sudachi_reading(
    tokenizer: &StatelessTokenizer<&JapaneseDictionary>,
    text: &str,
) -> Result<String> {
    let morphemes = tokenizer.tokenize(text, Mode::C, false)?;
    let mut raw = String::new();
    for m in morphemes.iter() {
        raw.push_str(&surface_reading(&m.surface(), m.reading_form()));
    }
    Ok(kata_to_hira(&raw))
}

pub fn sudachi_tokens(
    tokenizer: &StatelessTokenizer<&JapaneseDictionary>,
    text: &str,
) -> Result<Vec<String>> {
    let morphemes = tokenizer.tokenize(text, Mode::C, false)?;
    Ok(morphemes.iter().map(|m| m.surface().to_owned()).collect())
}

/// オプションの `mecab` CLI による UniDic 読み検証（未導入環境では None を返し Sudachi 単体で継続可）。
pub fn try_mecab_reading(text: &str) -> Option<String> {
    let mut child = Command::new("mecab")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes());
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut raw = String::new();
    for line in stdout.lines() {
        if line == "EOS" {
            break;
        }
        if let Some((surf, feats)) = line.split_once('\t') {
            let cols: Vec<&str> = feats.split(',').collect();
            let kana = cols.get(9).or_else(|| cols.get(7)).copied().unwrap_or("");
            raw.push_str(&surface_reading(surf, kana));
        }
    }
    Some(kata_to_hira(&raw))
}

fn tail_chars(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_owned()
    } else {
        chars[chars.len() - max_chars..].iter().collect()
    }
}

/// 出典候補 (doc, text, context) 群。
pub type SourceCandidates = (
    Vec<(String, String, String)>,
    Vec<(String, String, String)>,
    Vec<(&'static str, String, String, String)>,
);

/// Wikinews bz2 XML + RealPersonaChat + JMultiWOZ + Wikipedia 新記事から評価候補を抽出する。
pub fn collect_source_candidates(sources: &Path) -> Result<SourceCandidates> {
    let mut short_out = Vec::new();
    let mut doc_out = Vec::new();
    let mut wiki_out = Vec::new();

    let rpc_dir = sources
        .join("realpersonachat/extracted/real-persona-chat-1.0.0/real_persona_chat/dialogues");
    if rpc_dir.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&rpc_dir)?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort();
        for p in files {
            let d: Value = serde_json::from_str(&fs::read_to_string(&p)?)?;
            let did = &d["dialogue_id"];
            let doc = format!(
                "rpc-{}",
                did.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| did.to_string())
            );
            if let Some(utts) = d["utterances"].as_array() {
                for u in utts {
                    if let Some(t) = u["text"].as_str() {
                        for s in split_sentences(t) {
                            let len = s.chars().count();
                            if (4..=40).contains(&len) {
                                short_out.push((doc.clone(), s, String::new()));
                            }
                        }
                    }
                }
            }
        }
    }

    let mwoz_path = sources.join("jmultiwoz/extracted/JMultiWOZ_1.0/dialogues.json");
    if mwoz_path.is_file() {
        let w: BTreeMap<String, Value> = serde_json::from_str(&fs::read_to_string(&mwoz_path)?)?;
        for (_, d) in w {
            let did = &d["dialogue_id"];
            let doc = format!(
                "mwoz-{}",
                did.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| did.to_string())
            );
            let mut previous = String::new();
            if let Some(turns) = d["turns"].as_array() {
                for t in turns {
                    let speaker = t["speaker"].as_str().unwrap_or("");
                    let utt = t["utterance"].as_str().unwrap_or("");
                    if speaker == "USER" {
                        for s in split_sentences(utt) {
                            let len = s.chars().count();
                            if (4..=40).contains(&len) {
                                short_out.push((doc.clone(), s, String::new()));
                            }
                        }
                    } else if speaker == "SYSTEM" {
                        for s in split_sentences(utt) {
                            for c in split_chunks(&s, 8, 40) {
                                doc_out.push((doc.clone(), c, tail_chars(&previous, 60)));
                            }
                        }
                    }
                    previous = utt.replace('\n', "");
                }
            }
        }
    }

    let wikinews_path = sources.join("wikinews/jawikinews-latest-pages-articles.xml.bz2");
    if wikinews_path.is_file() {
        let bz = BzDecoder::new(BufReader::new(File::open(&wikinews_path)?));
        let mut reader = XmlReader::from_reader(BufReader::new(bz));
        let mut buf = Vec::new();
        let mut cur_tag = String::new();
        let (mut ns, mut title, mut text) = (String::new(), String::new(), String::new());
        let mut in_page = false;
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().local_name().as_ref()).into_owned();
                    if name == "page" {
                        in_page = true;
                        ns.clear();
                        title.clear();
                        text.clear();
                    }
                    cur_tag = name;
                }
                Ok(Event::Text(e)) if in_page => {
                    let txt = e.unescape()?.into_owned();
                    match cur_tag.as_str() {
                        "ns" => ns.push_str(&txt),
                        "title" => title.push_str(&txt),
                        "text" => text.push_str(&txt),
                        _ => {}
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().local_name().as_ref()).into_owned();
                    if name == "page" && in_page {
                        if ns.trim() == "0" && !text.is_empty() && !text.starts_with('#') {
                            let body = clean_wikitext(&text)?;
                            let mut before = String::new();
                            let doc = format!("news-{title}");
                            for s in split_sentences(&body) {
                                for c in split_chunks(&s, 8, 40) {
                                    doc_out.push((doc.clone(), c.clone(), tail_chars(&before, 60)));
                                    before.push_str(&c);
                                }
                            }
                        }
                        in_page = false;
                    }
                    cur_tag.clear();
                }
                Ok(Event::Eof) => break,
                Err(e) => bail!("Wikinews XML パースエラー: {e}"),
                _ => {}
            }
            buf.clear();
        }
    }

    let wiki_path = sources.join("wikipedia-new-v2/articles.jsonl");
    if wiki_path.is_file() {
        let tech_re = Regex::new(
            r"科学|技術|工学|医学|医療|医薬|薬|疾患|病|化学|物理|生物|生化学|遺伝|数学|統計|情報|計算機|コンピュータ|ソフトウェア|プログラミング|インターネット|電子|電気|機械|半導体|通信|鉄道車両|航空機|宇宙|天文|地質|気象|法律|法令|条約|裁判|経済|金融|会計|経営|建築|土木|農業|水産|材料|化合物|元素|言語学|心理学",
        )?;
        let paren_re = Regex::new(r"（[^（）]*）|\([^()]*\)")?;
        for line in BufReader::new(File::open(&wiki_path)?).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let a: Value = serde_json::from_str(&line)?;
            let title = a["title"]
                .as_str()
                .unwrap_or("")
                .split(" (")
                .next()
                .unwrap_or("");
            let tech = a["categories"].as_array().is_some_and(|cats| {
                cats.iter()
                    .filter_map(Value::as_str)
                    .any(|c| tech_re.is_match(c))
            });
            let mut before = String::new();
            let doc = format!("wiki-{}", a["pageid"]);
            for s in split_sentences(a["extract"].as_str().unwrap_or("")) {
                let s = paren_re.replace_all(&s, "");
                for c in split_chunks(&s, 8, 40) {
                    let domain = if !title.is_empty() && c.contains(title) {
                        Some("names")
                    } else if tech {
                        Some("tech")
                    } else {
                        None
                    };
                    if let Some(dom) = domain {
                        wiki_out.push((dom, doc.clone(), c.clone(), tail_chars(&before, 60)));
                    }
                    before.push_str(&c);
                }
            }
        }
    }

    Ok((short_out, doc_out, wiki_out))
}

/// `gen_notation_table.py`: 日常表記の比率 TSV から C++ 埋め込み表 (`.inc`) を生成する。
pub fn gen_notation_table(tsv: &Path, meta_path: &Path, out: &Path) -> Result<usize> {
    let min_share = 0.7f64;
    let meta: Value = serde_json::from_str(&fs::read_to_string(meta_path)?)?;
    let mut rows: Vec<(String, String, f64, u64)> = Vec::new();
    for line in fs::read_to_string(tsv)?.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 5 {
            continue;
        }
        let share: f64 = f[2].parse()?;
        if share >= min_share {
            rows.push((f[0].to_owned(), f[1].to_owned(), share, f[3].parse()?));
        }
    }
    rows.sort_by(|a, b| {
        b.0.chars()
            .count()
            .cmp(&a.0.chars().count())
            .then_with(|| b.2.total_cmp(&a.2))
            .then_with(|| a.0.cmp(&b.0))
    });
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let sources: Vec<&str> = meta["sources"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut body = String::from("// 生成ファイル(gen_notation_table)。手で編集しない。\n");
    body.push_str(&format!(
        "// 出典: {}; {}; {}\n",
        sources.join(", "),
        meta["documents"].as_str().unwrap_or(""),
        meta["tokenizer"].as_str().unwrap_or("")
    ));
    body.push_str(&format!(
        "// 元のTSV SHA-256: {}; 多数派の割合 >= {min_share}; {}件\n",
        meta["sha256"].as_str().unwrap_or(""),
        rows.len()
    ));
    for (minor, major, share, n) in &rows {
        body.push_str(&format!(
            "{{\"{}\", \"{}\", {share:.4}f}},  // {n}\n",
            esc(minor),
            esc(major)
        ));
    }
    fs::write(out, body.as_bytes())?;
    Ok(rows.len())
}

/// `gen_context_defaults.py`: 検証済み JSON から `asteria_context_defaults.inc` を生成する。
pub fn gen_context_defaults_inc(json_path: &Path, out: &Path) -> Result<Value> {
    let data: Vec<Value> = serde_json::from_str(&fs::read_to_string(json_path)?)?;
    let mut lines = Vec::new();
    for g in &data {
        let reading = g["reading"].as_str().context("reading")?;
        let senses = g["senses"].as_array().context("senses")?;
        for (i, chosen) in senses.iter().enumerate() {
            let cues: Vec<&str> = chosen["cues"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            if cues.is_empty() {
                continue;
            }
            let c_surf = chosen["surface"].as_str().context("surface")?;
            let joined = cues.join("、");
            for (j, rejected) in senses.iter().enumerate() {
                if i == j {
                    continue;
                }
                let r_surf = rejected["surface"].as_str().context("surface")?;
                lines.push((reading, c_surf, r_surf, joined.clone()));
            }
        }
    }
    let sha = sha256_file(json_path)?;
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut body = String::from("// 生成ファイル(gen_context_defaults)。手で編集しない。\n");
    body.push_str(&format!(
        "// 元: mozc/data/context_cues_20261027.json(Sonnet が書いた手がかりの語を検証したもの) SHA-256 {sha}; {}件\n",
        lines.len()
    ));
    for (a, b, c, d) in &lines {
        body.push_str(&format!(
            "{{\"{}\", \"{}\", \"{}\", \"{}\"}},\n",
            esc(a),
            esc(b),
            esc(c),
            esc(d)
        ));
    }
    fs::write(out, body.as_bytes())?;
    Ok(json!({ "groups": data.len(), "entries": lines.len() }))
}

/// `fetch_wikipedia_new_articles.py`: MediaWiki API から新規記事を取得する。
pub fn fetch_wikipedia_new_articles(
    out: &Path,
    start: &str,
    end: &str,
    per_month: usize,
) -> Result<Value> {
    const API: &str = "https://ja.wikipedia.org/w/api.php";
    const UA: &str = "AsteriaIME-eval-builder/0.1 (local evaluation set construction; contact via project maintainer)";

    let parse_ym = |s: &str| -> Result<(i32, u32)> {
        let (ys, ms) = s.split_once('-').context("YYYY-MM")?;
        Ok((ys.parse()?, ms.parse()?))
    };
    let (mut y, mut m) = parse_ym(start)?;
    let (ey, em) = parse_ym(end)?;
    fs::create_dir_all(out)?;

    let get = |params: &[(&str, &str)]| -> Result<Value> {
        let mut url = format!("{API}?format=json&formatversion=2");
        for (k, v) in params {
            url.push('&');
            url.push_str(k);
            url.push('=');
            for b in v.as_bytes() {
                if b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b'.' | b'~') {
                    url.push(*b as char);
                } else {
                    url.push_str(&format!("%{:02X}", b));
                }
            }
        }
        for attempt in 0..5 {
            match fetch_bytes(&url, 16 * 1024 * 1024, UA, Duration::from_secs(60)) {
                Ok(bytes) => return Ok(serde_json::from_slice(&bytes)?),
                Err(_) if attempt < 4 => {
                    thread::sleep(Duration::from_secs(5 * (attempt as u64 + 1)))
                }
                Err(e) => return Err(e),
            }
        }
        bail!("API request failed")
    };

    let mut titles: Vec<(String, String, String)> = Vec::new();
    while (y, m) <= (ey, em) {
        let lestart = format!("{y:04}-{m:02}-15T00:00:00Z");
        let lelimit = (per_month * 3).min(500).to_string();
        let mut cont_params: Vec<(String, String)> = Vec::new();
        let mut picked = 0usize;
        while picked < per_month {
            let mut params = vec![
                ("action", "query"),
                ("list", "logevents"),
                ("letype", "create"),
                ("lenamespace", "0"),
                ("lestart", &lestart),
                ("ledir", "older"),
                ("lelimit", &lelimit),
            ];
            for (k, v) in &cont_params {
                params.push((k.as_str(), v.as_str()));
            }
            let data = get(&params)?;
            let prefix = format!("{y:04}-{m:02}");
            if let Some(events) = data["query"]["logevents"].as_array() {
                for ev in events {
                    if picked >= per_month {
                        break;
                    }
                    let ts = ev["timestamp"].as_str().unwrap_or("");
                    if !ts.starts_with(&prefix) {
                        picked = per_month;
                        break;
                    }
                    if let Some(title) = ev["title"].as_str() {
                        if ev["pageid"].is_number() {
                            titles.push((title.to_owned(), ts.to_owned(), prefix.clone()));
                            picked += 1;
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
            if let Some(cont) = data.get("continue").and_then(Value::as_object) {
                cont_params = cont
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect();
            } else {
                break;
            }
        }
        if m == 12 {
            y += 1;
            m = 1;
        } else {
            m += 1;
        }
    }

    let mut rows = Vec::new();
    for chunk in titles.chunks(20) {
        let joined: String = chunk
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect::<Vec<_>>()
            .join("|");
        let data = get(&[
            ("action", "query"),
            ("prop", "extracts|info|pageprops|categories"),
            ("exintro", "1"),
            ("explaintext", "1"),
            ("cllimit", "max"),
            ("clshow", "!hidden"),
            ("redirects", "0"),
            ("titles", &joined),
        ])?;
        let mut pages: HashMap<&str, &Value> = HashMap::new();
        if let Some(arr) = data["query"]["pages"].as_array() {
            for p in arr {
                if let Some(t) = p["title"].as_str() {
                    pages.insert(t, p);
                }
            }
        }
        for (title, created, month) in chunk {
            let Some(&p) = pages.get(title.as_str()) else {
                continue;
            };
            if p["missing"].as_bool() == Some(true)
                || p["redirect"].as_bool() == Some(true)
                || p["pageprops"].get("disambiguation").is_some()
            {
                continue;
            }
            let text = p["extract"].as_str().unwrap_or("").trim();
            if !text.is_empty() {
                let cats: Vec<String> = p["categories"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|c| c["title"].as_str())
                            .map(|t| t.split_once(':').map(|(_, r)| r).unwrap_or(t).to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                rows.push(json!({
                    "title": title,
                    "created": created,
                    "month": month,
                    "pageid": p["pageid"],
                    "lastrevid": p["lastrevid"],
                    "categories": cats,
                    "extract": text,
                }));
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    let mut body = String::new();
    for r in &rows {
        body.push_str(&serde_json::to_string(r)?);
        body.push('\n');
    }
    fs::write(out.join("articles.jsonl"), body)?;
    let meta = json!({
        "api": API,
        "user_agent": UA,
        "start": start,
        "end": end,
        "per_month": per_month,
        "titles": titles.len(),
        "articles": rows.len(),
        "license": "CC BY-SA 4.0 (Wikipedia text)",
    });
    fs::write(out.join("meta.json"), serde_json::to_string_pretty(&meta)?)?;
    Ok(meta)
}

fn reading_disagree_or_eq(
    tok: &StatelessTokenizer<&JapaneseDictionary>,
    text: &str,
) -> Result<Option<String>> {
    let a = sudachi_reading(tok, text)?;
    if let Some(b) = try_mecab_reading(text) {
        if a != b {
            return Ok(None);
        }
    }
    Ok(Some(a))
}

/// `build_eval_sets.py`: Wikinews / RealPersonaChat / JMultiWOZ / Wikipedia 新記事から評価セットを生成する。
pub fn run_build_eval_sets(
    sources: &Path,
    out: &Path,
    sudachi_dict: &Path,
    judge_per_domain: usize,
    dev_per_domain: usize,
) -> Result<Value> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let allowed = Regex::new(
        r"^[ぁ-ゖァ-ヺー一-龥々〆ヶ0-9０-９A-Za-zＡ-Ｚａ-ｚ、。！？!?・％%「」『』（）()\s]+$",
    )?;
    let kanji_kana = Regex::new(r"[一-鿿ァ-ヺ]")?;
    let typable = Regex::new(r"^[ぁ-ゖー0-9、。！？!?（）()「」『』・〜～%]+$")?;

    let (short_cands, doc_cands, wiki_cands) = collect_source_candidates(sources)?;
    let mut pools: BTreeMap<(&'static str, &'static str), Vec<Value>> = BTreeMap::new();
    let mut stats: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut consider =
        |domain: &'static str, doc: String, text: String, context: String| -> Result<()> {
            *stats.entry(format!("{domain}:seen")).or_default() += 1;
            let len = text.chars().count();
            if seen.contains(&text) || !allowed.is_match(&text) {
                *stats.entry(format!("{domain}:filtered")).or_default() += 1;
                return Ok(());
            }
            if len > 30 && !kanji_kana.is_match(&text) {
                *stats.entry(format!("{domain}:junk")).or_default() += 1;
                return Ok(());
            }
            seen.insert(text.clone());
            let Some(reading) = reading_disagree_or_eq(&tok, &text)? else {
                *stats
                    .entry(format!("{domain}:reading_disagree"))
                    .or_default() += 1;
                return Ok(());
            };
            if !typable.is_match(&nfkc(&reading)) {
                *stats.entry(format!("{domain}:untypable")).or_default() += 1;
                return Ok(());
            }
            pools
                .entry((domain, split_of(&doc)))
                .or_default()
                .push(json!({
                    "doc": doc,
                    "input": reading,
                    "context_text": context.replace('\t', " "),
                    "text": text,
                }));
            *stats.entry(format!("{domain}:accepted")).or_default() += 1;
            Ok(())
        };

    for (doc, text, ctx) in short_cands {
        consider("short", doc, text, ctx)?;
    }
    for (doc, text, ctx) in doc_cands {
        consider("doc", doc, text, ctx)?;
    }
    for (domain, doc, text, ctx) in wiki_cands {
        consider(domain, doc, text, ctx)?;
    }

    fs::create_dir_all(out)?;
    let mut sets = serde_json::Map::new();
    for ((domain, split), mut pool) in pools {
        let want = if split == "judge" {
            judge_per_domain
        } else {
            dev_per_domain
        };
        pool.sort_by_cached_key(|r| sha256_bytes(r["text"].as_str().unwrap_or("").as_bytes()));
        let mut by_doc: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for item in &pool {
            let d = item["doc"].as_str().unwrap_or("").to_owned();
            by_doc.entry(d).or_default().push(item.clone());
        }
        let mut docs: Vec<String> = by_doc.keys().cloned().collect();
        docs.sort_by_cached_key(|d| sha256_bytes(d.as_bytes()));
        let mut picked = Vec::new();
        while picked.len() < want {
            let mut progressed = false;
            for d in &docs {
                if picked.len() >= want {
                    break;
                }
                if let Some(q) = by_doc.get_mut(d) {
                    if !q.is_empty() {
                        picked.push(q.remove(0));
                        progressed = true;
                    }
                }
            }
            if !progressed {
                break;
            }
        }
        let mut items = Vec::with_capacity(picked.len());
        for (i, r) in picked.iter().enumerate() {
            let txt = r["text"].as_str().unwrap_or("");
            let toks = sudachi_tokens(&tok, txt)?;
            items.push(json!({
                "index": format!("{domain}-{split}-{i:04}"),
                "input": r["input"],
                "context_text": r["context_text"],
                "expected_output": expected_for(txt, &toks),
                "domain": domain,
                "source_doc": r["doc"],
            }));
        }
        let name = format!("asteria-{domain}-{split}.json");
        let path = out.join(&name);
        fs::write(&path, serde_json::to_string_pretty(&items)?)?;
        sets.insert(
            name,
            json!({
                "items": items.len(),
                "pool": pool.len(),
                "docs": docs.len(),
                "sha256": sha256_file(&path)?,
            }),
        );
    }
    let summary = json!({ "stats": stats, "sets": sets });
    fs::write(
        out.join("build-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    Ok(summary)
}

const DISTINCT_KANJI: &[&str] = &[
    "測計量図謀諮",
    "効利",
    "熱暑厚",
    "温暖",
    "早速",
    "収納治修",
    "勧進薦",
    "変代替換",
    "会合遭",
    "上揚挙",
    "表現著",
    "写映",
    "冒犯侵",
    "堅固硬",
    "尋訪",
    "説解溶",
    "止留泊",
    "執採捕取撮",
    "付着就",
    "空開明",
    "帰返",
    "聞効",
    "見診観看",
    "立建",
    "降下",
    "冷覚",
    "誤謝",
    "厚篤",
    "初始",
    "柔軟",
    "絞搾",
    "差指刺挿射",
    "勤努務",
    "備供",
    "伸延",
    "乗載",
    "張貼",
    "沈鎮",
    "超越",
    "成為",
    "望臨",
    "生産",
    "鳴泣",
    "斬切",
    "混交",
    "異違",
];

/// `build_context_choice.py` の異字同訓表記ゆれ判定。
pub fn same_spelling_family(a: &str, b: &str, normalized_same: bool) -> bool {
    let kanji_only = |s: &str| -> String {
        s.chars()
            .filter(|&c| ('一'..='鿿').contains(&c) || c == '々')
            .collect()
    };
    let ka = kanji_only(a);
    let kb = kanji_only(b);
    if ka == kb || ka.contains(&kb) || kb.contains(&ka) {
        return true;
    }
    if !normalized_same {
        return false;
    }
    let sa: HashSet<char> = ka.chars().collect();
    let sb: HashSet<char> = kb.chars().collect();
    let diff: HashSet<char> = sa.symmetric_difference(&sb).copied().collect();
    !DISTINCT_KANJI
        .iter()
        .any(|g| diff.iter().all(|c| g.contains(*c)))
}

/// `build_context_choice.py`: 文中選択・長文評価セット (`asteria-choice-*`, `asteria-passage-*`) を生成する。
#[allow(clippy::too_many_arguments)]
pub fn run_build_context_choice(
    sources: &Path,
    out: &Path,
    sudachi_dict: &Path,
    judge_pairs: usize,
    dev_pairs: usize,
    judge_passages: usize,
    dev_passages: usize,
    passage_len: usize,
) -> Result<Value> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let has_kanji = |s: &str| s.chars().any(|c| ('一'..='鿿').contains(&c) || c == '々');
    let (short_cands, doc_cands, wiki_cands) = collect_source_candidates(sources)?;
    let mut by_reading: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    let mut docs: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stats: BTreeMap<String, usize> = BTreeMap::new();

    let mut consider = |doc: &str, chunk: &str| -> Result<()> {
        let len = chunk.chars().count();
        if !(8..=30).contains(&len) {
            return Ok(());
        }
        *stats.entry("chunks".into()).or_default() += 1;
        let ms = tok.tokenize(chunk, Mode::C, false)?;
        for idx in 0..ms.len() {
            let m = ms.get(idx);
            let p = m.part_of_speech();
            let p0 = p.first().map(String::as_str).unwrap_or("");
            let p1 = p.get(1).map(String::as_str).unwrap_or("");
            let surf = m.surface();
            if !matches!(p0, "名詞" | "動詞" | "形容詞") || !has_kanji(&surf) {
                continue;
            }
            if p0 == "名詞" && (p1 != "普通名詞" || surf.chars().count() < 2) {
                continue;
            }
            let (reading, key) = if p0 == "名詞" {
                (
                    kata_to_hira(m.reading_form()),
                    m.normalized_form().to_owned(),
                )
            } else {
                let lemma = m.dictionary_form().to_owned();
                let r = sudachi_reading(&tok, &lemma)?;
                (r, lemma)
            };
            if !reading.is_empty() && has_kanji(&key) {
                by_reading
                    .entry(reading)
                    .or_default()
                    .entry(key)
                    .or_default()
                    .push(json!({
                        "doc": doc,
                        "chunk": chunk,
                        "surface": &*surf,
                        "pos": p0,
                    }));
            }
        }
        Ok(())
    };

    for (doc, chunk, _) in &doc_cands {
        consider(doc, chunk)?;
        docs.entry(doc.clone()).or_default().push(chunk.clone());
    }
    for (doc, chunk, _) in &short_cands {
        consider(doc, chunk)?;
    }
    for (_, doc, chunk, _) in &wiki_cands {
        consider(doc, chunk)?;
        docs.entry(doc.clone()).or_default().push(chunk.clone());
    }

    let mut pairs: Vec<(String, Vec<Value>)> = Vec::new();
    for (reading, groups) in &by_reading {
        if groups.len() < 2 {
            continue;
        }
        let mut sorted: Vec<(&String, &Vec<Value>)> = groups.iter().collect();
        sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
        let (ka, ia) = sorted[0];
        let (kb, ib) = sorted[1];
        let docs_a: HashSet<&str> = ia.iter().filter_map(|r| r["doc"].as_str()).collect();
        let docs_b: HashSet<&str> = ib.iter().filter_map(|r| r["doc"].as_str()).collect();
        if ia.len().min(ib.len()) < 3 || docs_a.len().min(docs_b.len()) < 2 {
            *stats.entry("rare".into()).or_default() += 1;
            continue;
        }
        let norm_one = |w: &str| -> String {
            let Ok(ms) = tok.tokenize(w, Mode::C, false) else {
                return w.to_owned();
            };
            if ms.len() == 1 {
                ms.get(0).normalized_form().to_owned()
            } else {
                w.to_owned()
            }
        };
        if same_spelling_family(ka, kb, norm_one(ka) == norm_one(kb)) {
            *stats.entry("spelling_family".into()).or_default() += 1;
            continue;
        }
        let mut picked: Vec<Value> = Vec::new();
        for (key, occ, rival_key, rival_occ) in [(ka, ia, kb, ib), (kb, ib, ka, ia)] {
            let mut sorted_occ = occ.clone();
            sorted_occ.sort_by_cached_key(|r| {
                let key_s = format!(
                    "{}{}",
                    r["doc"].as_str().unwrap_or(""),
                    r["chunk"].as_str().unwrap_or("")
                );
                sha256_bytes(key_s.as_bytes())
            });
            for r in sorted_occ {
                let d = r["doc"].as_str().unwrap_or("");
                let c = r["chunk"].as_str().unwrap_or("");
                if picked.iter().any(|p| p["doc"].as_str() == Some(d))
                    || c.contains(rival_key.as_str())
                {
                    continue;
                }
                let Some(inp) = reading_disagree_or_eq(&tok, c)? else {
                    *stats.entry("reading_disagree".into()).or_default() += 1;
                    continue;
                };
                let rivals: BTreeSet<String> = rival_occ
                    .iter()
                    .filter_map(|o| o["surface"].as_str().map(str::to_owned))
                    .collect();
                let mut item = r.clone();
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("key".into(), json!(key));
                    obj.insert("input".into(), json!(inp));
                    obj.insert(
                        "rival".into(),
                        json!(rivals.into_iter().take(4).collect::<Vec<_>>()),
                    );
                }
                picked.push(item);
                break;
            }
        }
        if picked.len() == 2 {
            pairs.push((reading.clone(), picked));
        }
    }
    *stats.entry("pairs".into()).or_default() = pairs.len();

    fs::create_dir_all(out)?;
    let mut sets = serde_json::Map::new();
    for (split, want) in [("judge", judge_pairs), ("dev", dev_pairs)] {
        let mut chosen: Vec<&(String, Vec<Value>)> = pairs
            .iter()
            .filter(|(r, _)| split_of(&format!("choice-{r}")) == split)
            .collect();
        chosen.sort_by_cached_key(|(r, _)| sha256_bytes(r.as_bytes()));
        let mut rows = Vec::new();
        for (n, (reading, items)) in chosen.into_iter().take(want).enumerate() {
            for (side, it) in ['a', 'b'].iter().zip(items.iter()) {
                let ch = it["chunk"].as_str().unwrap_or("");
                let toks = sudachi_tokens(&tok, ch)?;
                rows.push(json!({
                    "index": format!("choice-{split}-{n:03}{side}"),
                    "input": it["input"],
                    "context_text": "",
                    "expected_output": expected_for(ch, &toks),
                    "domain": "context_choice",
                    "pair": format!("{split}-{n:03}"),
                    "word_reading": reading,
                    "target": it["surface"],
                    "rival": it["rival"],
                    "pos": it["pos"],
                    "source_doc": it["doc"],
                }));
            }
        }
        let name = format!("asteria-choice-{split}.json");
        let p = out.join(&name);
        fs::write(&p, serde_json::to_string_pretty(&rows)?)?;
        sets.insert(
            name,
            json!({"items": rows.len(), "sha256": sha256_file(&p)?}),
        );
    }

    let mut passages: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for (doc, chunks) in docs {
        let mut run = Vec::new();
        for c in chunks {
            let l = c.chars().count();
            let inp = if (8..=40).contains(&l) {
                reading_disagree_or_eq(&tok, &c)?
            } else {
                None
            };
            let Some(inp) = inp else {
                run.clear();
                continue;
            };
            run.push((c, inp));
            if run.len() == passage_len {
                passages.push((doc.clone(), run.clone()));
                break;
            }
        }
    }
    *stats.entry("passages".into()).or_default() = passages.len();
    for (split, want) in [("judge", judge_passages), ("dev", dev_passages)] {
        let mut chosen: Vec<&(String, Vec<(String, String)>)> = passages
            .iter()
            .filter(|(d, _)| split_of(&format!("passage-{d}")) == split)
            .collect();
        chosen.sort_by_cached_key(|(d, _)| sha256_bytes(d.as_bytes()));
        let mut rows = Vec::new();
        for (n, (doc, run)) in chosen.into_iter().take(want).enumerate() {
            let mut before = String::new();
            for (k, (c, inp)) in run.iter().enumerate() {
                let tail: String = before
                    .chars()
                    .rev()
                    .take(60)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                let toks = sudachi_tokens(&tok, c)?;
                rows.push(json!({
                    "index": format!("passage-{split}-{n:03}-{k}"),
                    "input": inp,
                    "context_text": tail,
                    "expected_output": expected_for(c, &toks),
                    "domain": "passage",
                    "passage": format!("{split}-{n:03}"),
                    "source_doc": doc,
                }));
                before.push_str(c);
            }
        }
        let name = format!("asteria-passage-{split}.json");
        let p = out.join(&name);
        fs::write(&p, serde_json::to_string_pretty(&rows)?)?;
        sets.insert(
            name,
            json!({"items": rows.len(), "sha256": sha256_file(&p)?}),
        );
    }

    let summary = json!({ "stats": stats, "sets": sets });
    fs::write(
        out.join("context-choice-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    Ok(summary)
}

/// `build_context_pairs.py`: 前文依存最小対 (`asteria-ctxpair-*`) を生成する。
pub fn run_build_context_pairs(
    sources: &Path,
    out: &Path,
    sudachi_dict: &Path,
    judge_pairs: usize,
    dev_pairs: usize,
) -> Result<Value> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let has_kanji = |s: &str| s.chars().any(|c| ('一'..='鿿').contains(&c) || c == '々');
    let (short_cands, doc_cands, wiki_cands) = collect_source_candidates(sources)?;
    let _ = short_cands;
    let mut by_reading: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    let mut stats: BTreeMap<String, usize> = BTreeMap::new();

    let mut scan = |doc: &str, chunk: &str, context: &str| -> Result<()> {
        let ms = tok.tokenize(chunk, Mode::C, false)?;
        let mut pos = 0usize;
        for i in 0..ms.len() {
            let m = ms.get(i);
            let surf = m.surface().to_owned();
            let start = pos;
            pos += surf.len();
            let p = m.part_of_speech();
            if p.first().map(String::as_str) != Some("名詞")
                || p.get(1).map(String::as_str) != Some("普通名詞")
                || !has_kanji(&surf)
                || surf.chars().count() < 2
                || i + 1 >= ms.len()
                || ms.get(i + 1).part_of_speech().first().map(String::as_str) != Some("助詞")
            {
                continue;
            }
            let phrase = format!("{surf}{}", ms.get(i + 1).surface());
            *stats.entry("phrases".into()).or_default() += 1;
            let raw_before = format!("{context}{}", &chunk[..start]).replace('\t', " ");
            let before: String = raw_before
                .chars()
                .rev()
                .take(60)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            if before.chars().count() < 8 {
                continue;
            }
            by_reading
                .entry(kata_to_hira(m.reading_form()))
                .or_default()
                .entry(m.normalized_form().to_owned())
                .or_default()
                .push(json!({
                    "doc": doc,
                    "phrase": phrase,
                    "noun": surf,
                    "context": before,
                }));
        }
        Ok(())
    };

    for (doc, chunk, ctx) in &doc_cands {
        scan(doc, chunk, ctx)?;
    }
    for (_, doc, chunk, ctx) in &wiki_cands {
        scan(doc, chunk, ctx)?;
    }

    let mut pairs: Vec<(String, Vec<Value>)> = Vec::new();
    for (reading, groups) in &by_reading {
        if groups.len() < 2 {
            continue;
        }
        let mut sorted: Vec<(&String, &Vec<Value>)> = groups.iter().collect();
        sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
        let sort_occ = |occ: &[Value]| {
            let mut v = occ.to_vec();
            v.sort_by_cached_key(|r| {
                let s = format!(
                    "{}{}",
                    r["doc"].as_str().unwrap_or(""),
                    r["phrase"].as_str().unwrap_or("")
                );
                sha256_bytes(s.as_bytes())
            });
            v
        };
        let ia = sort_occ(sorted[0].1);
        let ib = sort_occ(sorted[1].1);
        let a = &ia[0];
        let Some(b) = ib.iter().find(|r| r["doc"] != a["doc"]) else {
            continue;
        };
        if a["noun"] == b["noun"] {
            continue;
        }
        let mut items = Vec::new();
        for r in [a, b] {
            let phrase = r["phrase"].as_str().unwrap_or("");
            let Some(inp) = reading_disagree_or_eq(&tok, phrase)? else {
                *stats.entry("reading_disagree".into()).or_default() += 1;
                break;
            };
            let mut it = r.clone();
            if let Some(obj) = it.as_object_mut() {
                obj.insert("input".into(), json!(inp));
            }
            items.push(it);
        }
        if items.len() == 2 {
            pairs.push((reading.clone(), items));
        }
    }

    fs::create_dir_all(out)?;
    let mut sets = serde_json::Map::new();
    for (split, want) in [("judge", judge_pairs), ("dev", dev_pairs)] {
        let mut chosen: Vec<&(String, Vec<Value>)> = pairs
            .iter()
            .filter(|(r, _)| split_of(&format!("pair-{r}")) == split)
            .collect();
        chosen.sort_by_cached_key(|(r, _)| sha256_bytes(r.as_bytes()));
        let chosen: Vec<_> = chosen.into_iter().take(want).collect();
        let mut rows = Vec::new();
        for (n, (_, items)) in chosen.iter().enumerate() {
            for (side, it) in ['a', 'b'].iter().zip(items.iter()) {
                let phrase = it["phrase"].as_str().unwrap_or("");
                let exp: Vec<String> = width_variants(phrase).into_iter().collect();
                rows.push(json!({
                    "index": format!("ctxpair-{split}-{n:03}{side}"),
                    "input": it["input"],
                    "context_text": it["context"],
                    "expected_output": exp,
                    "domain": "context_pair",
                    "pair": format!("{split}-{n:03}"),
                    "source_doc": it["doc"],
                }));
            }
        }
        let name = format!("asteria-ctxpair-{split}.json");
        let p = out.join(&name);
        fs::write(&p, serde_json::to_string_pretty(&rows)?)?;
        sets.insert(
            name,
            json!({"pairs": chosen.len(), "items": rows.len(), "sha256": sha256_file(&p)?}),
        );
    }
    let summary = json!({ "stats": stats, "candidate_pairs": pairs.len(), "sets": sets });
    fs::write(
        out.join("ctxpair-summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;
    Ok(summary)
}

/// `build_kana_preference.py`: 開発用コーパスから漢字 vs ひらがなの頻度を数えて `kana_preference.tsv` を作る。
pub fn run_build_kana_preference(
    sources: &Path,
    out: &Path,
    sudachi_dict: &Path,
    min_count: usize,
) -> Result<Value> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let kana_re = Regex::new(r"^[ぁ-ゖー]+$")?;
    let kanji_re = Regex::new(r"[一-鿿々]")?;

    let mut utts: Vec<(String, String)> = Vec::new();
    let rpc_dir = sources
        .join("realpersonachat/extracted/real-persona-chat-1.0.0/real_persona_chat/dialogues");
    if rpc_dir.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&rpc_dir)?
            .filter_map(|e| e.ok().map(|x| x.path()))
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort();
        for p in files {
            let d: Value = serde_json::from_str(&fs::read_to_string(&p)?)?;
            let did = &d["dialogue_id"];
            let doc = format!(
                "rpc-{}",
                did.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| did.to_string())
            );
            if let Some(arr) = d["utterances"].as_array() {
                for u in arr {
                    if let Some(t) = u["text"].as_str() {
                        utts.push((doc.clone(), t.to_owned()));
                    }
                }
            }
        }
    }
    let mwoz_path = sources.join("jmultiwoz/extracted/JMultiWOZ_1.0/dialogues.json");
    if mwoz_path.is_file() {
        let w: BTreeMap<String, Value> = serde_json::from_str(&fs::read_to_string(&mwoz_path)?)?;
        for (_, d) in w {
            let did = &d["dialogue_id"];
            let doc = format!(
                "mwoz-{}",
                did.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| did.to_string())
            );
            if let Some(turns) = d["turns"].as_array() {
                for t in turns {
                    if let Some(u) = t["utterance"].as_str() {
                        utts.push((doc.clone(), u.to_owned()));
                    }
                }
            }
        }
    }

    type FullKey = (String, String, String, String, String);
    type MergedKey = (String, String, String, String);
    let mut counts: BTreeMap<FullKey, BTreeMap<String, usize>> = BTreeMap::new();
    let mut used_docs = 0usize;
    let mut last_doc = String::new();
    for (doc, text) in &utts {
        if split_of(doc) != "dev" {
            continue;
        }
        if doc != &last_doc {
            used_docs += 1;
            last_doc = doc.clone();
        }
        let ms = tok.tokenize(&text.replace('\n', ""), Mode::C, false)?;
        for idx in 0..ms.len() {
            let m = ms.get(idx);
            let s = m.surface();
            if !kana_re.is_match(&s) && !kanji_re.is_match(&s) {
                continue;
            }
            let pos = m.part_of_speech();
            let k = (
                m.normalized_form().to_owned(),
                kata_to_hira(m.reading_form()),
                pos.first().cloned().unwrap_or_default(),
                pos.get(4).cloned().unwrap_or_default(),
                pos.get(5).cloned().unwrap_or_default(),
            );
            *counts
                .entry(k)
                .or_default()
                .entry(s.to_owned())
                .or_default() += 1;
        }
    }

    let mut rows: Vec<(String, String, f64, usize, String)> = Vec::new();
    let mut have: HashSet<(String, String)> = HashSet::new();
    let mut emit = |c: &BTreeMap<String, usize>, pos0: &str| {
        let total: usize = c.values().sum();
        if total < min_count {
            return;
        }
        let Some((major, &mn)) = c
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.cmp(b.0)))
        else {
            return;
        };
        for (minor, &n) in c {
            if minor == major || !kanji_re.is_match(minor) || mn + n < min_count {
                continue;
            }
            if !have.insert((minor.clone(), major.clone())) {
                continue;
            }
            let ratio = ((mn as f64 / (mn + n) as f64) * 10000.0).round() / 10000.0;
            rows.push((minor.clone(), major.clone(), ratio, mn + n, pos0.to_owned()));
        }
    };

    for (key, c) in &counts {
        emit(c, &key.2);
    }
    let mut merged: BTreeMap<MergedKey, BTreeMap<String, usize>> = BTreeMap::new();
    for ((norm, _, p0, p4, p5), c) in &counts {
        let e = merged
            .entry((norm.clone(), p0.clone(), p4.clone(), p5.clone()))
            .or_default();
        for (surf, &n) in c {
            *e.entry(surf.clone()).or_default() += n;
        }
    }
    for (key, c) in &merged {
        emit(c, &key.1);
    }
    rows.sort_by(|a, b| {
        b.2.total_cmp(&a.2)
            .then_with(|| b.3.cmp(&a.3))
            .then_with(|| a.0.cmp(&b.0))
    });

    fs::create_dir_all(out)?;
    let tsv = out.join("kana_preference.tsv");
    let mut tsv_body = String::new();
    for (a, b, r, n, p) in &rows {
        tsv_body.push_str(&format!("{a}\t{b}\t{r}\t{n}\t{p}\n"));
    }
    fs::write(&tsv, tsv_body.as_bytes())?;
    let meta = json!({
        "sources": ["RealPersonaChat 1.0.0 (CC BY-SA 4.0)", "JMultiWOZ 1.0 (CC BY-SA 4.0)"],
        "documents": "evaluation dev split only (build_eval_sets.split_of == 'dev')",
        "used_documents": used_docs,
        "tokenizer": "suiko-sudachi SplitMode::C",
        "min_count": min_count,
        "entries": rows.len(),
        "sha256": sha256_file(&tsv)?,
    });
    fs::write(
        out.join("kana_preference.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;
    Ok(meta)
}

/// `build_name_lexicon.py`: Wikipedia 新記事の固有名詞・新語辞書 (`name_lexicon.tsv`) を生成する。
pub fn run_build_name_lexicon(
    articles: &Path,
    excludes: &[&Path],
    mozc_dict: &Path,
    short_lexicon: &Path,
    out: &Path,
    sudachi_dict: &Path,
    cost_offset: i64,
) -> Result<Value> {
    const POS_IDS: &[(&str, i64)] = &[
        ("名詞,固有名詞,一般,*,*,*,*", 1920),
        ("名詞,固有名詞,人名,一般,*,*,*", 1921),
        ("名詞,固有名詞,地域,一般,*,*,*", 1924),
        ("名詞,固有名詞,組織,*,*,*,*", 1929),
    ];
    let pos_id_of = |pos: &str| {
        POS_IDS
            .iter()
            .find(|(k, _)| *k == pos)
            .map(|x| x.1)
            .unwrap_or(1920)
    };
    let pos_from_id = |id: i64| POS_IDS.iter().find(|(_, v)| *v == id).map(|x| x.0);

    let title_surface = |title: &str| title.split(" (").next().unwrap_or(title).trim().to_owned();
    let person_re = Regex::new(
        r"存命人物|\d+年生|\d+年没|の人物$|選手$|俳優$|歌手$|政治家$|作家$|漫画家$|声優$|アイドル$",
    )?;
    let place_re = Regex::new(
        r"の町・字$|の地名$|の駅$|の島$|の山$|の川$|の湖$|の峠$|の集落$|の市町村$|の自治体$|の道路$",
    )?;
    let org_re = Regex::new(
        r"の企業$|の会社$|の団体$|の学校$|の大学$|の高等学校$|の政党$|のクラブ$|のバンド$|の音楽グループ$",
    )?;
    let kana_only_re = Regex::new(r"^[ァ-ヺー・]+$")?;
    let surf_re = Regex::new(r"^[一-鿿々〆ヶぁ-ゖァ-ヺー・A-Za-z0-9０-９Ａ-Ｚａ-ｚ]+$")?;
    let kk_re = Regex::new(r"[一-鿿々ァ-ヺ]")?;
    let hira_re = Regex::new(r"^[ぁ-ゖー]{2,40}$")?;
    let strip_punct_re = Regex::new(r"[・ 　]")?;

    let mut excluded_ids: HashSet<i64> = HashSet::new();
    let mut excluded_titles: HashSet<String> = HashSet::new();
    let mut excl_meta = Vec::new();
    for &p in excludes {
        excl_meta.push(json!({"path": p.to_string_lossy(), "sha256": sha256_file(p)?}));
        for line in fs::read_to_string(p)?
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            let a: Value = serde_json::from_str(line)?;
            if let Some(pid) = a["pageid"].as_i64() {
                excluded_ids.insert(pid);
            }
            if let Some(t) = a["title"].as_str() {
                excluded_titles.insert(title_surface(t));
            }
        }
    }

    let mut known: HashSet<(String, String)> = HashSet::new();
    let mut costs: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    let mut dict_files: Vec<_> = fs::read_dir(mozc_dict)?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("dictionary") && n.ends_with(".txt"))
        })
        .collect();
    dict_files.sort();
    for p in dict_files {
        for line in fs::read_to_string(&p)?.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 5 {
                continue;
            }
            known.insert((f[0].to_owned(), f[4].to_owned()));
            if let Ok(lid) = f[1].parse::<i64>() {
                if let Some(pos) = pos_from_id(lid) {
                    if let Ok(c) = f[3].parse::<i64>() {
                        costs.entry(pos.to_owned()).or_default().push(c);
                    }
                }
            }
        }
    }
    for line in fs::read_to_string(short_lexicon)?.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() >= 5 {
            known.insert((f[0].to_owned(), f[4].to_owned()));
        }
    }

    let mut base_cost: BTreeMap<String, i64> = BTreeMap::new();
    for &(pos, _) in POS_IDS {
        let mut c = costs.remove(pos).unwrap_or_default();
        c.sort_unstable();
        let med = if c.is_empty() { 6000 } else { c[c.len() / 2] };
        base_cost.insert(pos.to_owned(), med);
    }

    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let mut audit: BTreeMap<String, usize> = BTreeMap::new();
    let mut entries: BTreeMap<(String, String), (String, i64, Value)> = BTreeMap::new();

    for line in fs::read_to_string(articles)?
        .lines()
        .filter(|l| !l.trim().is_empty())
    {
        let a: Value = serde_json::from_str(line)?;
        *audit.entry("articles".into()).or_default() += 1;
        let pid = a["pageid"].as_i64().unwrap_or(0);
        let surface = title_surface(a["title"].as_str().unwrap_or(""));
        if excluded_ids.contains(&pid) || excluded_titles.contains(&surface) {
            *audit.entry("excluded_eval_article".into()).or_default() += 1;
            continue;
        }
        let slen = surface.chars().count();
        if !(2..=20).contains(&slen) || !surf_re.is_match(&surface) {
            *audit.entry("unsupported_surface".into()).or_default() += 1;
            continue;
        }
        if !kk_re.is_match(&surface) {
            *audit.entry("no_kanji_or_katakana".into()).or_default() += 1;
            continue;
        }
        let extract = a["extract"].as_str().unwrap_or("");
        let lead = if kana_only_re.is_match(&surface) {
            Some(kata_to_hira(&surface.replace('・', "")))
        } else {
            let pat = format!(
                "^{}\\s*[（(]([ぁ-ゖァ-ヺー・ 　]+)[、，,）)]",
                regex::escape(&surface)
            );
            Regex::new(&pat)
                .ok()
                .and_then(|re| re.captures(extract))
                .and_then(|c| c.get(1))
                .map(|m| kata_to_hira(&strip_punct_re.replace_all(m.as_str(), "")))
        };
        let Some(a_reading) = lead.filter(|r| hira_re.is_match(r)) else {
            *audit.entry("no_lead_reading".into()).or_default() += 1;
            continue;
        };
        let sr = strip_punct_re
            .replace_all(&sudachi_reading(&tok, &surface)?, "")
            .into_owned();
        let ur =
            try_mecab_reading(&surface).map(|r| strip_punct_re.replace_all(&r, "").into_owned());
        if a_reading != sr && ur.as_deref() != Some(a_reading.as_str()) {
            *audit.entry("reading_disagree".into()).or_default() += 1;
            continue;
        }
        if known.contains(&(a_reading.clone(), surface.clone())) {
            *audit.entry("already_in_mozc".into()).or_default() += 1;
            continue;
        }
        let cats: Vec<&str> = a["categories"]
            .as_array()
            .map(|arr| arr.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let pos = if cats.iter().any(|c| person_re.is_match(c)) {
            "名詞,固有名詞,人名,一般,*,*,*"
        } else if cats.iter().any(|c| place_re.is_match(c)) {
            "名詞,固有名詞,地域,一般,*,*,*"
        } else if cats.iter().any(|c| org_re.is_match(c)) {
            "名詞,固有名詞,組織,*,*,*,*"
        } else {
            "名詞,固有名詞,一般,*,*,*,*"
        };
        entries.insert(
            (a_reading, surface),
            (pos.to_owned(), pid, a["lastrevid"].clone()),
        );
        *audit.entry("accepted".into()).or_default() += 1;
    }

    fs::create_dir_all(out)?;
    let mut tsv_body = String::new();
    let mut by_pos: BTreeMap<String, usize> = BTreeMap::new();
    for ((reading, surface), (pos, _, _)) in &entries {
        let lid = pos_id_of(pos);
        let cost = (base_cost[pos] + cost_offset).clamp(0, 32767);
        tsv_body.push_str(&format!("{reading}\t{lid}\t{lid}\t{cost}\t{surface}\n"));
        *by_pos.entry(pos.clone()).or_default() += 1;
    }
    let tsv = out.join("name_lexicon.tsv");
    fs::write(&tsv, tsv_body.as_bytes())?;
    let mut sources_list: Vec<(i64, Value)> = entries
        .values()
        .map(|(_, pid, rev)| (*pid, json!({"pageid": pid, "lastrevid": rev})))
        .collect();
    sources_list.sort_by_key(|x| x.0);
    let pages: Vec<Value> = sources_list.into_iter().map(|x| x.1).collect();
    let meta_path = articles
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("meta.json");
    let source_meta: Value = if meta_path.is_file() {
        serde_json::from_str(&fs::read_to_string(&meta_path)?)?
    } else {
        Value::Null
    };
    let manifest = json!({
        "source": "ja.wikipedia.org 記事の題名と冒頭の読み（2024-03以降に作成された記事）",
        "source_meta": source_meta,
        "articles_sha256": sha256_file(articles)?,
        "excluded": excl_meta,
        "license": "CC BY-SA 4.0（Wikipedia）。題名と読みの組は、出典の記事の版（pageid、lastrevid）を pages に記録する",
        "rule": {
            "reading": "記事冒頭の括弧書きの読み（カタカナの題名はその読み）が SudachiDict または UniDic の読みと一致",
            "cost": "Mozc辞書の同じ品詞のコストの中央値 + cost_offset",
            "cost_offset": cost_offset,
            "base_cost": base_cost,
        },
        "audit": audit,
        "output_entries": entries.len(),
        "by_pos": by_pos,
        "sha256": sha256_bytes(tsv_body.as_bytes()),
        "pages": pages,
    });
    fs::write(
        out.join("name_lexicon.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(manifest)
}

/// `import_sonnet_choice.py`: Sonnet 生成文脈選択 JSON を調整用評価セットへ整形する。
pub fn run_import_sonnet_choice(src: &Path, out: &Path, sudachi_dict: &Path) -> Result<Value> {
    let dict = load_sudachi_dict(sudachi_dict)?;
    let tok = StatelessTokenizer::new(&dict);
    let mut stats: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    let mut files: Vec<_> = fs::read_dir(src)?
        .filter_map(|e| e.ok().map(|x| x.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    files.sort();
    for p in files {
        let domain = p.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown");
        let arr: Vec<Value> = serde_json::from_str(&fs::read_to_string(&p)?)?;
        for (n, it) in arr.iter().enumerate() {
            *stats.entry("in".into()).or_default() += 1;
            let s = it["sentence"].as_str().unwrap_or("").trim();
            let reading = it["reading"].as_str().unwrap_or("").trim();
            let target = it["target"].as_str().unwrap_or("");
            let rivals: Vec<&str> = it
                .get("rival")
                .or_else(|| it.get("rivals"))
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .filter(|r| !r.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            if s.is_empty()
                || reading.is_empty()
                || !s.contains(target)
                || rivals.is_empty()
                || rivals.contains(&target)
            {
                *stats.entry("bad_fields".into()).or_default() += 1;
                continue;
            }
            if !seen.insert(s.to_owned()) {
                *stats.entry("duplicate".into()).or_default() += 1;
                continue;
            }
            let Some(checked) = reading_disagree_or_eq(&tok, s)? else {
                *stats.entry("reading_mismatch".into()).or_default() += 1;
                continue;
            };
            if checked != reading {
                *stats.entry("reading_mismatch".into()).or_default() += 1;
                continue;
            }
            let toks = sudachi_tokens(&tok, s)?;
            let clue = it["clue"].as_str().unwrap_or("");
            rows.push(json!({
                "index": format!("sonnet-{domain}-{n:03}"),
                "input": reading,
                "context_text": it["context"].as_str().unwrap_or(""),
                "expected_output": expected_for(s, &toks),
                "domain": format!("sonnet_{domain}"),
                "target": target,
                "rival": rivals,
                "clue": clue,
                "reason": it["reason"].as_str().unwrap_or(""),
                "pair": format!("sonnet-{domain}-{n:03}"),
            }));
            *stats.entry("kept".into()).or_default() += 1;
            *stats.entry(format!("clue_{clue}")).or_default() += 1;
        }
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(out, serde_json::to_string_pretty(&rows)?)?;
    Ok(json!({
        "stats": stats,
        "out": out.to_string_lossy(),
        "sha256": sha256_file(out)?,
    }))
}

/// `make_atok_sets.py`: 打ち間違い・同音語・固有名詞の各 40 問調整用セットを書き出す。
pub fn run_make_atok_sets(out_dir: &Path) -> Result<()> {
    let typo: &[(&str, &[&str])] = &[
        ("ありがとうごさいます", &["ありがとうございます"]),
        (
            "よろしくおねがします",
            &["よろしくお願いします", "宜しくお願いします"],
        ),
        ("おせわになっておりまます", &["お世話になっております"]),
        ("ちょとまってください", &["ちょっと待ってください"]),
        ("しりょをおくります", &["資料を送ります"]),
        (
            "かいぎのじかんをへんこうしまいた",
            &["会議の時間を変更しました"],
        ),
        ("きょうはいいてんきでうね", &["今日はいい天気ですね"]),
        (
            "あしたのよていおかくにんします",
            &["明日の予定を確認します"],
        ),
        (
            "よろしくおねがいいたしまっす",
            &["よろしくお願いいたします", "よろしくお願い致します"],
        ),
        (
            "おつかれさまでいた",
            &["お疲れ様でした", "お疲れさまでした"],
        ),
        ("すみまえん", &["すみません"]),
        (
            "いつもおせあになっております",
            &["いつもお世話になっております"],
        ),
        ("けんとしておきます", &["検討しておきます"]),
        ("ごかくいんおねがいします", &["ご確認お願いします"]),
        (
            "らいしゅうのかよびはどうですか",
            &["来週の火曜日はどうですか"],
        ),
        ("しょうしょうおまちくだしあ", &["少々お待ちください"]),
        (
            "たいへんもうしわけござまいせん",
            &["大変申し訳ございません"],
        ),
        (
            "なにとぞよろしくおねがいもうしあげまう",
            &[
                "何卒よろしくお願い申し上げます",
                "なにとぞよろしくお願い申し上げます",
                "何卒宜しくお願い申し上げます",
            ],
        ),
        ("こんいちは", &["こんにちは"]),
        ("ありあとう", &["ありがとう"]),
        ("おはよございます", &["おはようございます"]),
        ("でんしゃがおくれていまう", &["電車が遅れています"]),
        ("いまからでがけます", &["今から出かけます"]),
        ("かくいんしました", &["確認しました"]),
        ("しゅくだいがおあった", &["宿題が終わった"]),
        ("けいやくしょをそうふしまう", &["契約書を送付します"]),
        ("らいねんのけいかう", &["来年の計画"]),
        ("びょういんにいきまう", &["病院に行きます"]),
        ("しんかんせでかえります", &["新幹線で帰ります"]),
        ("りょうしゅしょをください", &["領収書をください"]),
        ("よやくおへんこうしたい", &["予約を変更したい"]),
        (
            "きをつけてかえてね",
            &["気をつけて帰ってね", "気を付けて帰ってね"],
        ),
        ("おめでとうござます", &["おめでとうございます"]),
        ("せつめいしてくだあい", &["説明してください"]),
        (
            "ぜひさんかしたいでう",
            &["ぜひ参加したいです", "是非参加したいです"],
        ),
        ("こんどのしゅまつ", &["今度の週末"]),
        ("れんあくおまちしています", &["連絡お待ちしています"]),
        ("わかりまいた", &["わかりました", "分かりました"]),
        ("だいじょうぶでう", &["大丈夫です"]),
        (
            "ちゅもんしたしょうひんがとどいた",
            &["注文した商品が届いた"],
        ),
    ];
    let homophone: &[(&str, &[&str])] = &[
        (
            "しょうひんのひんしつをほしょうします",
            &["商品の品質を保証します"],
        ),
        (
            "こくみんのせいかつをほしょうする",
            &["国民の生活を保障する"],
        ),
        ("じこのそんがいをほしょうする", &["事故の損害を補償する"]),
        ("せきにんをついきゅうする", &["責任を追及する"]),
        ("りえきをついきゅうする", &["利益を追求する"]),
        (
            "がくもんのしんりをついきゅうする",
            &["学問の真理を追究する"],
        ),
        ("たいじゅうをはかる", &["体重を量る", "体重を計る"]),
        ("じかんをはかる", &["時間を計る"]),
        ("きょりをはかる", &["距離を測る"]),
        ("こっきょうをこえる", &["国境を越える"]),
        ("ていいんをこえる", &["定員を超える"]),
        ("きかいにいじょうがみつかった", &["機械に異常が見つかった"]),
        ("いがいなけっか", &["意外な結果"]),
        (
            "かんけいしゃいがいたちいりきんし",
            &["関係者以外立ち入り禁止", "関係者以外立入禁止"],
        ),
        (
            "どうぶつのせいたいをちょうさする",
            &["動物の生態を調査する"],
        ),
        ("せいたいもしゃがとくいだ", &["声帯模写が得意だ"]),
        ("ぶんしょうのこうせいをかんがえる", &["文章の構成を考える"]),
        ("こうせいなさいばん", &["公正な裁判"]),
        ("げんこうのこうせいをする", &["原稿の校正をする"]),
        ("いしがつよい", &["意志が強い"]),
        ("いしのしんさつをうける", &["医師の診察を受ける"]),
        ("じぶんのうでにじしんがある", &["自分の腕に自信がある"]),
        ("おおきなじしんがおきた", &["大きな地震が起きた"]),
        ("しょうがいのともだち", &["生涯の友達", "生涯の友だち"]),
        ("しょうがいぶつをとりのぞく", &["障害物を取り除く"]),
        ("かいとうようしにかきこむ", &["解答用紙に書き込む"]),
        ("しつもんへのかいとう", &["質問への回答"]),
        ("たいしょうしゃをしぼる", &["対象者を絞る"]),
        ("さゆうたいしょうのずけい", &["左右対称の図形"]),
        (
            "ふたつのあんをたいしょうする",
            &["二つの案を対照する", "２つの案を対照する"],
        ),
        ("きかんしゃがはしる", &["機関車が走る"]),
        (
            "ていしゅつのきかんはらいしゅうまで",
            &["提出の期間は来週まで"],
        ),
        ("ちきゅうにきかんする", &["地球に帰還する"]),
        ("しゅうしょくかつどうをはじめる", &["就職活動を始める"]),
        ("めいしをしゅうしょくする", &["名詞を修飾する"]),
        ("しんぶんきしゃにはなしをきく", &["新聞記者に話を聞く"]),
        ("せいひんのしようをかくにんする", &["製品の仕様を確認する"]),
        ("しようきょかをえる", &["使用許可を得る"]),
        ("かていのじじょう", &["家庭の事情"]),
        (
            "じじょうさようでよごれがおちる",
            &["自浄作用で汚れが落ちる"],
        ),
    ];
    let names: &[(&str, &[&str])] = &[
        ("おおたにしょうへい", &["大谷翔平"]),
        ("はにゅうゆづる", &["羽生結弦"]),
        ("ふじいそうた", &["藤井聡太"]),
        ("よねづけんし", &["米津玄師"]),
        ("みやざきはやお", &["宮崎駿"]),
        ("しんかいまこと", &["新海誠"]),
        ("むらかみはるき", &["村上春樹"]),
        ("なつめそうせき", &["夏目漱石"]),
        ("あくたがわりゅうのすけ", &["芥川龍之介"]),
        ("とくがわいえやす", &["徳川家康"]),
        ("さかもとりょうま", &["坂本龍馬", "坂本竜馬"]),
        ("おだのぶなが", &["織田信長"]),
        ("ほっかいどうさっぽろし", &["北海道札幌市"]),
        ("きょうとふうじし", &["京都府宇治市"]),
        ("かごしまけん", &["鹿児島県"]),
        ("おきなわけんなはし", &["沖縄県那覇市"]),
        ("とどろきけいこく", &["等々力渓谷"]),
        ("おちゃのみず", &["御茶ノ水", "お茶の水"]),
        ("あきはばら", &["秋葉原"]),
        ("せいせいえーあい", &["生成AI"]),
        ("おしかつ", &["推し活"]),
        ("きめつのやいば", &["鬼滅の刃"]),
        ("じゅじゅつかいせん", &["呪術廻戦"]),
        ("そうそうのふりーれん", &["葬送のフリーレン"]),
        ("りょうしこんぴゅーたー", &["量子コンピューター"]),
        ("はんどうたい", &["半導体"]),
        ("せんじょうこうすいたい", &["線状降水帯"]),
        ("しんがたころなういるす", &["新型コロナウイルス"]),
        ("はたらきかたかいかく", &["働き方改革"]),
        ("ぶっかだか", &["物価高"]),
        ("えんやす", &["円安"]),
        ("やみばいと", &["闇バイト"]),
        ("おおさか・かんさいばんぱく", &["大阪・関西万博"]),
        ("のとはんとうじしん", &["能登半島地震"]),
        ("ねっちゅうしょう", &["熱中症"]),
        ("もうしょび", &["猛暑日"]),
        ("さむらいじゃぱん", &["侍ジャパン"]),
        ("しょうぎのはっかん", &["将棋の八冠"]),
        ("よしのや", &["吉野家"]),
        ("むじるしりょうひん", &["無印良品"]),
    ];

    fs::create_dir_all(out_dir)?;
    for (name, prefix, rows) in [
        ("asteria-typo.json", "t", typo),
        ("asteria-homophone.json", "h", homophone),
        ("asteria-names.json", "n", names),
    ] {
        let data: Vec<Value> = rows
            .iter()
            .enumerate()
            .map(|(i, (r, e))| {
                json!({
                    "index": format!("{prefix}{:02}", i + 1),
                    "context_text": "",
                    "input": r,
                    "expected_output": e,
                })
            })
            .collect();
        fs::write(
            out_dir.join(name),
            format!("{}\n", serde_json::to_string_pretty(&data)?),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn width_and_kana_variants_match_expected() {
        let w = width_variants("10%");
        assert!(w.contains("10%"));
        assert!(w.contains("１０％"));
        let toks = vec!["出来る".into(), "事".into()];
        let kv = kana_variants(&toks, 16);
        assert!(kv.contains("出来る事"));
        assert!(kv.contains("できること"));
    }

    #[test]
    fn gen_notation_table_formats_cpp_entries() {
        let dir = TempDir::new().unwrap();
        let tsv = dir.path().join("pref.tsv");
        let meta = dir.path().join("pref.json");
        let out = dir.path().join("out.inc");
        fs::write(
            &tsv,
            "良い\tいい\t0.8500\t100\t形容詞\n事\tこと\t0.6000\t50\t名詞\n",
        )
        .unwrap();
        fs::write(
            &meta,
            json!({
                "sources": ["RPC"],
                "documents": "dev",
                "tokenizer": "Sudachi",
                "sha256": sha256_file(&tsv).unwrap(),
            })
            .to_string(),
        )
        .unwrap();
        let count = gen_notation_table(&tsv, &meta, &out).unwrap();
        assert_eq!(count, 1);
        let text = fs::read_to_string(&out).unwrap();
        assert!(text.contains(r#"{"良い", "いい", 0.8500f},  // 100"#));
    }
}
