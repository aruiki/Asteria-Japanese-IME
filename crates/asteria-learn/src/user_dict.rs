//! ユーザー辞書 (TSV 読み書き・登録・検索)。

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDictEntry {
    pub reading: String,
    pub surface: String,
    pub pos: String,
    pub comment: String,
}

#[derive(Debug, Clone, Default)]
pub struct UserDictionary {
    entries: Vec<UserDictEntry>,
}

fn to_katakana(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{3041}'..='\u{3096}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
            _ => c,
        })
        .collect()
}

impl UserDictionary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, reading: &str, surface: &str, pos: &str, comment: &str) {
        let reading = to_katakana(reading.trim());
        let surface = surface.trim().to_owned();
        if reading.is_empty() || surface.is_empty() {
            return;
        }
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.reading == reading && e.surface == surface)
        {
            existing.pos = pos.trim().to_owned();
            existing.comment = comment.trim().to_owned();
            return;
        }
        self.entries.push(UserDictEntry {
            reading,
            surface,
            pos: if pos.trim().is_empty() {
                "名詞".to_owned()
            } else {
                pos.trim().to_owned()
            },
            comment: comment.trim().to_owned(),
        });
    }

    pub fn import_tsv(&mut self, text: &str) -> usize {
        let mut added = 0;
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            let cols: Vec<&str> = line.split('\t').collect();
            match cols.as_slice() {
                [r, s] => {
                    self.add(r, s, "名詞", "");
                    added += 1;
                }
                [r, s, pos] => {
                    self.add(r, s, pos, "");
                    added += 1;
                }
                [r, s, pos, comment, ..] => {
                    self.add(r, s, pos, comment);
                    added += 1;
                }
                _ => {}
            }
        }
        added
    }

    pub fn export_tsv(&self) -> String {
        let mut out = String::new();
        for e in &self.entries {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\n",
                e.reading, e.surface, e.pos, e.comment
            ));
        }
        out
    }

    pub fn load_file(&mut self, path: &Path) -> std::io::Result<usize> {
        let text = fs::read_to_string(path)?;
        Ok(self.import_tsv(&text))
    }

    pub fn save_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, self.export_tsv())
    }

    pub fn lookup(&self, reading: &str) -> Vec<&UserDictEntry> {
        let key = to_katakana(reading);
        self.entries.iter().filter(|e| e.reading == key).collect()
    }

    pub fn prefix_lookup(&self, prefix_reading: &str) -> Vec<&UserDictEntry> {
        let key = to_katakana(prefix_reading);
        if key.is_empty() {
            return Vec::new();
        }
        self.entries
            .iter()
            .filter(|e| e.reading.starts_with(&key))
            .collect()
    }

    pub fn entries(&self) -> &[UserDictEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tsv_import_export_and_prefix_lookup() {
        let mut dict = UserDictionary::new();
        let n = dict.import_tsv("# header\nあすてりあ\tAsteria\t固有名詞\tIME名\nてすと\tテスト\n");
        assert_eq!(n, 2);
        assert_eq!(dict.lookup("アステリア")[0].surface, "Asteria");
        assert_eq!(dict.prefix_lookup("アステ").len(), 1);
        let exported = dict.export_tsv();
        assert!(exported.contains("アステリア\tAsteria\t固有名詞\tIME名"));
    }
}
