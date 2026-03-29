use crate::scene::{ContrastProfile, RenderConfig, RenderMode, DEFAULT_CHARSET};

const BRAILLE_RAMP: &str = "⠀⠂⠆⠖⠶⠷⠿⡿⣿";
const ADAPTIVE_ASCII_LOW: [char; 9] = [' ', '.', ':', '=', '+', '*', '#', '%', '@'];
const ADAPTIVE_ASCII_NORMAL: [char; 10] = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
const ADAPTIVE_ASCII_HIGH: [char; 11] = [' ', ' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

pub(super) fn glyph_coverage(glyph: char) -> f32 {
    if glyph == ' ' {
        return 0.0;
    }
    let code = glyph as u32;
    if (0x2800..=0x28ff).contains(&code) {
        let mask = (code - 0x2800) as u8;
        return (mask.count_ones() as f32 / 8.0).clamp(0.20, 1.0);
    }
    match glyph {
        '.' | '\'' | '`' => 0.35,
        ':' | ';' => 0.45,
        '-' | '_' => 0.55,
        '=' | '+' => 0.70,
        '*' | 'x' | 'X' => 0.80,
        '#' => 0.90,
        '%' => 0.95,
        '@' => 1.0,
        _ => 0.82,
    }
}

#[derive(Debug, Clone)]
pub struct GlyphRamp {
    chars: Vec<char>,
}

impl GlyphRamp {
    pub fn from_config(config: &RenderConfig) -> Self {
        let source = if config.mode == RenderMode::Braille {
            BRAILLE_RAMP
        } else if config.charset.is_empty() {
            " "
        } else {
            config.charset.as_str()
        };
        let mut chars: Vec<char> = source.chars().collect();
        if chars.is_empty() {
            chars.push(' ');
        }
        Self { chars }
    }

    pub fn chars(&self) -> &[char] {
        &self.chars
    }
}

pub(super) fn glyph_for_intensity(intensity: f32, charset: &[char]) -> char {
    if charset.is_empty() {
        return ' ';
    }
    let last = charset.len().saturating_sub(1);
    let index = ((intensity * (last as f32)).round() as usize).min(last);
    charset[index]
}

pub(super) fn glyph_intensity(glyph: char, charset: &[char]) -> f32 {
    if charset.is_empty() {
        return 0.0;
    }
    if let Some(index) = charset.iter().position(|ch| *ch == glyph) {
        let denom = charset.len().saturating_sub(1).max(1) as f32;
        return (index as f32 / denom).clamp(0.0, 1.0);
    }
    if glyph == ' ' {
        0.0
    } else {
        1.0
    }
}

pub(super) fn select_charset<'a>(
    config: &RenderConfig,
    fallback: &'a [char],
    cells: usize,
) -> &'a [char] {
    if config.mode != RenderMode::Ascii {
        return fallback;
    }
    if config.charset != DEFAULT_CHARSET {
        return fallback;
    }
    if config.contrast_profile == ContrastProfile::Fixed {
        return fallback;
    }
    if cells < 6_000 {
        &ADAPTIVE_ASCII_HIGH
    } else if cells < 12_000 {
        &ADAPTIVE_ASCII_NORMAL
    } else {
        &ADAPTIVE_ASCII_LOW
    }
}
