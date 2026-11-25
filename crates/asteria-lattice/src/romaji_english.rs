//! ローマ字のまま打った英語・固有名詞候補 (docs/adr/0080, `asteria_romaji_english.{h,cc}` 移植)。

use std::collections::{HashMap, HashSet};

use asteria_dict::EnglishLexicon;

/// 製品名・サービス名・技術用語の一覧。日本語の語として読める語は含めない。
pub const BUILTIN_NAMES: &[&str] = &[
    "GitHub",
    "GitLab",
    "Git",
    "Linux",
    "Ubuntu",
    "Debian",
    "Fedora",
    "Python",
    "JavaScript",
    "TypeScript",
    "Java",
    "Rust",
    "Golang",
    "Kotlin",
    "Swift",
    "Ruby",
    "Perl",
    "PHP",
    "Node",
    "React",
    "Vue",
    "Docker",
    "Kubernetes",
    "Nginx",
    "Apache",
    "MySQL",
    "PostgreSQL",
    "SQLite",
    "Redis",
    "Slack",
    "Zoom",
    "Discord",
    "Teams",
    "Notion",
    "Figma",
    "Trello",
    "Jira",
    "Confluence",
    "YouTube",
    "Twitter",
    "Instagram",
    "Facebook",
    "TikTok",
    "Twitch",
    "Reddit",
    "Netflix",
    "Spotify",
    "Amazon",
    "Google",
    "Gmail",
    "Chrome",
    "Firefox",
    "Safari",
    "Edge",
    "Apple",
    "iPhone",
    "iPad",
    "iPod",
    "Mac",
    "MacBook",
    "macOS",
    "iOS",
    "Android",
    "Windows",
    "Microsoft",
    "Excel",
    "PowerPoint",
    "Outlook",
    "OneDrive",
    "Dropbox",
    "Steam",
    "Xbox",
    "PlayStation",
    "Intel",
    "AMD",
    "NVIDIA",
    "Qualcomm",
    "Samsung",
    "OpenAI",
    "ChatGPT",
    "Claude",
    "Gemini",
    "Copilot",
    "Bluetooth",
    "USB",
    "HDMI",
    "PDF",
    "HTML",
    "CSS",
    "JSON",
    "YAML",
    "XML",
    "API",
    "SQL",
    "URL",
    "HTTP",
    "HTTPS",
    "DNS",
    "VPN",
    "CPU",
    "GPU",
    "SSD",
    "RAM",
    "AWS",
    "Azure",
    "Vercel",
    "Netlify",
    "Cloudflare",
    "Unity",
    "Unreal",
    "Blender",
    "Photoshop",
    "Illustrator",
    "Premiere",
    "Canva",
    "Zapier",
    "Stripe",
    "PayPal",
    "Uber",
    "Airbnb",
    "WordPress",
    "Shopify",
    "LinkedIn",
    "WhatsApp",
    "Telegram",
    "Signal",
    "Tesla",
    "Wikipedia",
    "Markdown",
    "LaTeX",
    "Vim",
    "Emacs",
    "VSCode",
    "Xcode",
    "npm",
    "pip",
    "Homebrew",
    "Ollama",
    "Mozc",
    "Asteria",
];

#[derive(Debug, Clone)]
pub struct RomajiEnglishLookup {
    names_lower: HashSet<String>,
    builtin_map: HashMap<String, &'static str>,
    lexicon: Option<EnglishLexicon>,
}

impl Default for RomajiEnglishLookup {
    fn default() -> Self {
        let mut names_lower = HashSet::with_capacity(BUILTIN_NAMES.len());
        let mut builtin_map = HashMap::with_capacity(BUILTIN_NAMES.len());
        for &name in BUILTIN_NAMES {
            let lower = name.to_ascii_lowercase();
            names_lower.insert(lower.clone());
            builtin_map.insert(lower, name);
        }
        Self {
            names_lower,
            builtin_map,
            lexicon: None,
        }
    }
}

impl RomajiEnglishLookup {
    pub fn with_lexicon(mut self, lexicon: EnglishLexicon) -> Self {
        self.lexicon = Some(lexicon);
        self
    }

    /// 小文字英字列に対応する英語表記を返す。戻り値の `bool` は製品名・技術固有名詞テーブル由来かを示す。
    pub fn find(&self, raw_lower_keys: &str) -> Option<(String, bool)> {
        if raw_lower_keys.is_empty() || !raw_lower_keys.bytes().all(|b| b.is_ascii_alphabetic()) {
            return None;
        }
        let lower = raw_lower_keys.to_ascii_lowercase();
        let is_name = self.names_lower.contains(&lower);
        if let Some(&canonical) = self.builtin_map.get(&lower) {
            return Some((canonical.to_owned(), is_name));
        }
        self.lexicon
            .as_ref()
            .and_then(|lex| lex.canonical_english(&lower))
            .map(|word| (word.to_owned(), false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_builtin_brand_names_and_lexicon_words() {
        let lex = EnglishLexicon::parse_tsv(
            "インテル\tIntel\nアプリケーション\tapplication\nパス\tpath\n",
        );
        let lookup = RomajiEnglishLookup::default().with_lexicon(lex);
        assert_eq!(lookup.find("github"), Some(("GitHub".to_owned(), true)));
        assert_eq!(lookup.find("asteria"), Some(("Asteria".to_owned(), true)));
        assert_eq!(
            lookup.find("application"),
            Some(("application".to_owned(), false))
        );
        assert_eq!(
            lookup.find("path"),
            None,
            "5文字未満の一般英単語は変換しない"
        );
        assert_eq!(lookup.find("123"), None);
    }
}
