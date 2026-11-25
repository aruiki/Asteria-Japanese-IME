//! 読み整合検証 (`MatchesReading`, ADR 0075) と数詞・助数詞バリエーション生成。

use std::collections::HashMap;

use asteria_dict::Dictionary;

use crate::{to_hiragana, MAX_SPAN};

fn to_katakana_char(c: char) -> char {
    match c {
        '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
        _ => c,
    }
}

fn to_hiragana_char(c: char) -> char {
    match c {
        '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

fn is_kanji(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c) || c == '々'
}

fn unvoice_katakana(c: char) -> Option<char> {
    Some(match c {
        'ガ' => 'カ',
        'ギ' => 'キ',
        'グ' => 'ク',
        'ゲ' => 'ケ',
        'ゴ' => 'コ',
        'ザ' => 'サ',
        'ジ' => 'シ',
        'ズ' => 'ス',
        'ゼ' => 'セ',
        'ゾ' => 'ソ',
        'ダ' => 'タ',
        'ヂ' => 'チ',
        'ヅ' => 'ツ',
        'デ' => 'テ',
        'ド' => 'ト',
        'バ' | 'パ' => 'ハ',
        'ビ' | 'ピ' => 'ヒ',
        'ブ' | 'プ' => 'フ',
        'ベ' | 'ペ' => 'ヘ',
        'ボ' | 'ポ' => 'ホ',
        _ => return None,
    })
}

fn is_alnum_or_unit(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || ('０'..='９').contains(&c)
        || ('Ａ'..='Ｚ').contains(&c)
        || ('ａ'..='ｚ').contains(&c)
        || matches!(
            c,
            '%' | '％'
                | '℃'
                | '°'
                | '㎡'
                | '㎥'
                | '㎠'
                | '㎞'
                | '㎝'
                | '㎜'
                | '㎏'
                | 'ℓ'
                | '-'
                | '/'
                | ','
                | '.'
        )
}

fn same_kana_or_punct(a: char, b: char) -> bool {
    let ha = to_hiragana_char(a);
    let hb = to_hiragana_char(b);
    ha == hb || (matches!(a, 'ヶ' | 'ケ') && matches!(b, 'ヶ' | 'ケ' | 'か' | 'カ'))
}

/// 候補文字列 `text` が入力読み `reading` の妥当な変換か辞書で検証する。
///
/// `genmai` 等の生成候補が別読みの類義語へすり替わる (`しょうひん` → `製品`)・
/// 長文を途中で打ち切る・語尾モーラを落とす現象を排除するために使う。
pub fn matches_reading(dict: &Dictionary, reading: &str, text: &str) -> bool {
    let r_chars: Vec<char> = reading.chars().map(to_katakana_char).collect();
    let t_chars: Vec<char> = text.chars().collect();
    if r_chars.is_empty() || t_chars.is_empty() {
        return r_chars.is_empty() && t_chars.is_empty();
    }

    let r_kata: String = r_chars.iter().collect();
    let mut r_byte_off = Vec::with_capacity(r_chars.len() + 1);
    let mut off = 0usize;
    r_byte_off.push(0);
    for c in &r_chars {
        off += c.len_utf8();
        r_byte_off.push(off);
    }

    let mut t_byte_off = Vec::with_capacity(t_chars.len() + 1);
    let mut toff = 0usize;
    t_byte_off.push(0);
    for c in &t_chars {
        toff += c.len_utf8();
        t_byte_off.push(toff);
    }

    let t_len = t_chars.len();
    let r_len = r_chars.len();
    let mut seen = vec![false; (t_len + 1) * (r_len + 1)];
    seen[0] = true;
    let mut stack = vec![(0usize, 0usize)];
    let mut cache: HashMap<usize, Vec<(usize, String)>> = HashMap::new();

    let mut push = |i: usize, j: usize, stack: &mut Vec<(usize, usize)>| {
        if i <= t_len && j <= r_len {
            let slot = i * (r_len + 1) + j;
            if !seen[slot] {
                seen[slot] = true;
                stack.push((i, j));
            }
        }
    };

    while let Some((i, j)) = stack.pop() {
        if i == t_len && j == r_len {
            return true;
        }
        if i < t_len && j < r_len && same_kana_or_punct(t_chars[i], r_chars[j]) {
            push(i + 1, j + 1, &mut stack);
        }
        if i < t_len && is_alnum_or_unit(t_chars[i]) {
            for k in 1..=10 {
                if j + k <= r_len {
                    push(i + 1, j + k, &mut stack);
                }
            }
        }
        // 長音・中黒や全角半角スペースは表記側のみに入る場合がある。
        if i < t_len
            && matches!(t_chars[i], '・' | ' ')
            && !matches!(r_chars.get(j), Some('・' | ' '))
        {
            push(i + 1, j, &mut stack);
        }
        if i < t_len && j < r_len {
            let entries = cache.entry(j).or_insert_with(|| {
                let r_end = (j + MAX_SPAN).min(r_len);
                let slice = &r_kata[r_byte_off[j]..r_byte_off[r_end]];
                let mut list = Vec::new();
                for m in dict.prefix_search(slice) {
                    let rc = m.reading_chars();
                    for e in m.entries() {
                        list.push((rc, e.surface.into_owned()));
                    }
                }
                // 複合語中の連濁（夜駆け: ヨガケ←ヨ+カケ、支援試合: シエンジアイ←シエン+シアイ）を許容。
                if j > 0 {
                    if let Some(unvoiced) = unvoice_katakana(r_chars[j]) {
                        let tail = &r_kata[r_byte_off[j + 1]..r_byte_off[r_end]];
                        let mut alt = String::with_capacity(unvoiced.len_utf8() + tail.len());
                        alt.push(unvoiced);
                        alt.push_str(tail);
                        for m in dict.prefix_search(&alt) {
                            let rc = m.reading_chars();
                            for e in m.entries() {
                                if e.surface.chars().next().is_some_and(is_kanji) {
                                    list.push((rc, e.surface.into_owned()));
                                }
                            }
                        }
                    }
                }
                list
            });
            let t_rest = &text[t_byte_off[i]..];
            for (rc, surf) in entries {
                if !surf.is_empty() && t_rest.starts_with(surf.as_str()) {
                    push(i + surf.chars().count(), j + *rc, &mut stack);
                }
            }
            // 「一〜九」を含む数詞は漢数字/算用数字が混在する読み（みっかい→3回/三回）を許容する。
            if is_kanji(t_chars[i])
                && matches!(
                    t_chars[i],
                    '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十'
                )
            {
                for k in 1..=4 {
                    if j + k <= r_len {
                        push(i + 1, j + k, &mut stack);
                    }
                }
            }
        }
    }
    let _ = to_hiragana;
    false
}

/// 漢数字＋助数詞 (`三回` 等) や数詞＋`パーセント` を算用数字・記号へそろえた候補を作る。
pub fn counter_arabic_variants(text: &str) -> Vec<String> {
    const DIGITS: &[(char, char)] = &[
        ('一', '1'),
        ('二', '2'),
        ('三', '3'),
        ('四', '4'),
        ('五', '5'),
        ('六', '6'),
        ('七', '7'),
        ('八', '8'),
        ('九', '9'),
    ];
    const COUNTERS: &[char] = &[
        '回', '日', '年', '月', '時', '分', '個', '本', '枚', '件', '人', '番',
    ];
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();

    if chars.len() >= 2 {
        let mut replaced = String::with_capacity(text.len());
        let mut changed = false;
        for idx in 0..chars.len() {
            let c = chars[idx];
            if idx + 1 < chars.len()
                && COUNTERS.contains(&chars[idx + 1])
                && (idx == 0
                    || !matches!(
                        chars[idx - 1],
                        '一' | '二'
                            | '三'
                            | '四'
                            | '五'
                            | '六'
                            | '七'
                            | '八'
                            | '九'
                            | '十'
                            | '百'
                            | '千'
                            | '万'
                    ))
            {
                if let Some((_, d)) = DIGITS.iter().find(|&&(k, _)| k == c) {
                    replaced.push(*d);
                    changed = true;
                    continue;
                }
            }
            replaced.push(c);
        }
        if changed && replaced != text {
            out.push(replaced);
        }
    }

    if text.contains("パーセント") {
        let mut pct = String::with_capacity(text.len());
        let mut rest = text;
        let mut changed = false;
        while let Some(at) = rest.find("パーセント") {
            let before = &rest[..at];
            pct.push_str(before);
            if before
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_digit() || ('０'..='９').contains(&c))
            {
                pct.push('%');
                changed = true;
            } else {
                pct.push_str("パーセント");
            }
            rest = &rest[at + "パーセント".len()..];
        }
        pct.push_str(rest);
        if changed && pct != text && !out.contains(&pct) {
            out.push(pct);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use asteria_dict::DictBuilder;

    #[test]
    fn matches_reading_accepts_valid_and_rejects_wrong_reading() {
        let mut b = DictBuilder::new();
        b.add("ショウヒン", "商品", 1, 1, 1000, 0).unwrap();
        b.add("セイヒン", "製品", 1, 1, 1000, 0).unwrap();
        b.add("ノ", "の", 2, 2, 500, 0).unwrap();
        b.add("ヒンシツ", "品質", 1, 1, 1200, 0).unwrap();
        b.add("カギ", "鍵", 1, 1, 1000, 0).unwrap();
        b.add("カギリ", "限", 1, 1, 1000, 0).unwrap();
        b.add("ジ", "時", 1, 1, 1000, 0).unwrap();
        b.add("ヨ", "夜", 1, 1, 1000, 0).unwrap();
        b.add("カケ", "駆け", 1, 1, 1000, 0).unwrap();
        b.add("シエン", "支援", 1, 1, 1000, 0).unwrap();
        b.add("シアイ", "試合", 1, 1, 1000, 0).unwrap();
        b.set_connection(3, 3, vec![0; 9]).unwrap();
        let dict = Dictionary::from_bytes(b.build().unwrap()).unwrap();

        assert!(matches_reading(&dict, "ショウヒンノヒンシツ", "商品の品質"));
        assert!(!matches_reading(
            &dict,
            "ショウヒンノヒンシツ",
            "製品の品質"
        ));
        assert!(matches_reading(&dict, "カギ", "鍵"));
        assert!(!matches_reading(&dict, "カギ", "限"));
        assert!(matches_reading(&dict, "ジュウジ", "10時"));
        assert!(!matches_reading(&dict, "ジュウジ", "時"));
        assert!(matches_reading(&dict, "ヨガケ", "夜駆け"));
        assert!(matches_reading(&dict, "シエンジアイ", "支援試合"));
        assert!(!matches_reading(&dict, "ガギ", "鍵"));
    }

    #[test]
    fn counter_variants_normalize_mixed_counters() {
        assert_eq!(
            counter_arabic_variants("1日三回薬を飲む"),
            vec!["1日3回薬を飲む"]
        );
        assert_eq!(counter_arabic_variants("20パーセントオフ"), vec!["20%オフ"]);
    }
}
