//! 候補の窓の色(今の C++ の窓と同じ値。renderer/renderer_style.textproto と ApplyKotoriPalette)。

/// 0〜255 の RGB。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Direct2D の色(0.0〜1.0)。
    pub fn to_f32(self) -> [f32; 3] {
        [
            self.0 as f32 / 255.0,
            self.1 as f32 / 255.0,
            self.2 as f32 / 255.0,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub foreground: Rgb,
    /// 番号・説明・下の帯の文字(候補より控えめ)。
    pub secondary: Rgb,
    pub focused_background: Rgb,
    pub focused_border: Rgb,
    pub border: Rgb,
}

/// 明るい配色(Mozc の既定の窓)。
pub const LIGHT: Palette = Palette {
    background: Rgb(255, 255, 255),
    foreground: Rgb(0, 0, 0),
    secondary: Rgb(136, 136, 136),
    focused_background: Rgb(209, 234, 255),
    focused_border: Rgb(127, 172, 221),
    border: Rgb(150, 150, 150),
};

/// 暗い配色(Asteria の ApplyKotoriPalette)。
pub const DARK: Palette = Palette {
    background: Rgb(32, 32, 32),
    foreground: Rgb(245, 245, 245),
    secondary: Rgb(170, 170, 170),
    focused_background: Rgb(54, 65, 80),
    focused_border: Rgb(54, 65, 80),
    border: Rgb(80, 80, 80),
};

pub fn for_theme(dark: bool) -> &'static Palette {
    if dark {
        &DARK
    } else {
        &LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secondary_is_quieter_than_foreground() {
        // 明るい配色では背景に近く、暗い配色では背景から遠すぎない(主従を保つ)。
        let lum = |c: Rgb| c.0 as u32 + c.1 as u32 + c.2 as u32;
        assert!(lum(LIGHT.secondary) > lum(LIGHT.foreground));
        assert!(lum(DARK.secondary) < lum(DARK.foreground));
    }

    #[test]
    fn converts_to_unit_range() {
        assert_eq!(Rgb(255, 0, 51).to_f32(), [1.0, 0.0, 0.2]);
    }
}
