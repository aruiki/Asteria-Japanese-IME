//! ニュアンス・同音異義語の意味比較リライター (`asteria_nuance_rewriter.{h,cc}` 移植)。

#[derive(Debug, Clone, Copy)]
struct Choice {
    word: &'static str,
    meaning: &'static str,
}

const GROUPS: &[&[Choice]] = &[
    &[
        Choice {
            word: "特徴",
            meaning: "目立つ性質",
        },
        Choice {
            word: "特長",
            meaning: "良いところ・長所",
        },
    ],
    &[
        Choice {
            word: "意志",
            meaning: "こうしたいという思い",
        },
        Choice {
            word: "意思",
            meaning: "考え・意向",
        },
    ],
    &[
        Choice {
            word: "制作",
            meaning: "作品を作る",
        },
        Choice {
            word: "製作",
            meaning: "物を作る",
        },
    ],
    &[
        Choice {
            word: "保証",
            meaning: "確かだと請け合う",
        },
        Choice {
            word: "保障",
            meaning: "権利や安全を守る",
        },
        Choice {
            word: "補償",
            meaning: "損失を補う",
        },
    ],
    &[
        Choice {
            word: "回答",
            meaning: "質問への返事",
        },
        Choice {
            word: "解答",
            meaning: "問題の答え",
        },
    ],
    &[
        Choice {
            word: "対象",
            meaning: "相手・目的となるもの",
        },
        Choice {
            word: "対照",
            meaning: "比べ合わせる",
        },
        Choice {
            word: "対称",
            meaning: "釣り合った配置",
        },
    ],
    &[
        Choice {
            word: "関心",
            meaning: "興味を持つこと",
        },
        Choice {
            word: "感心",
            meaning: "優れたものと認めること",
        },
    ],
];

const MAX_ALTERNATIVES: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NuanceAlternative {
    pub text: String,
    pub description: String,
}

/// 第 1 候補文に含まれる重要同音異義語に対して、文全体の意味比較付き別候補を生成する。
pub fn generate_nuance_alternatives(
    original: &str,
    existing_candidates: &[String],
) -> Vec<NuanceAlternative> {
    if original.is_empty() || original.len() > 1024 {
        return Vec::new();
    }
    let mut out = Vec::new();
    'outer: for group in GROUPS {
        for from in *group {
            for (pos, _) in original.match_indices(from.word) {
                for to in *group {
                    if from.word == to.word {
                        continue;
                    }
                    let mut text = original.to_owned();
                    text.replace_range(pos..pos + from.word.len(), to.word);
                    if existing_candidates.iter().any(|c| c == &text)
                        || out.iter().any(|alt: &NuanceAlternative| alt.text == text)
                    {
                        continue;
                    }
                    out.push(NuanceAlternative {
                        text,
                        description: format!("{} → {}（{}）", from.word, to.word, to.meaning),
                    });
                    if out.len() >= MAX_ALTERNATIVES {
                        break 'outer;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_nuance_alternatives_with_descriptions() {
        let existing = vec!["回答を送る".to_string()];
        let alts = generate_nuance_alternatives("回答を送る", &existing);
        assert_eq!(alts.len(), 1);
        assert_eq!(alts[0].text, "解答を送る");
        assert!(alts[0].description.contains("問題の答え"));
    }
}
