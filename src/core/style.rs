//! 작성자: Git 이력 참조
//! 작성목적: 배지 색과 크기 규칙. OS별 오버레이가 같은 모양을 그리도록 한곳에 둔다.
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | 모드별 색, 배지 치수, 강조 배율·시간 추가

use std::time::Duration;

use super::state::InputMode;

/// 모드가 바뀐 뒤 크게 보여주는 시간
pub const EMPHASIS_DURATION: Duration = Duration::from_millis(800);
/// 강조 중 글자 크기 배율
pub const EMPHASIS_SCALE: f64 = 1.8;

/// 배지 배경색(sRGB). 글자는 흰색이므로 흰 글자와 대비가 충분한 진한 색을 쓴다.
pub fn badge_rgb(mode: &InputMode) -> (u8, u8, u8) {
    match mode {
        InputMode::Hangul => (0xEA, 0x58, 0x0C),      // 주황
        InputMode::Latin => (0x47, 0x55, 0x69),       // 회청색
        InputMode::Japanese(_) => (0xBE, 0x18, 0x5D), // 자홍
        InputMode::Chinese(_) => (0xB9, 0x1C, 0x1C),  // 빨강
        InputMode::Other(_) => (0x03, 0x69, 0xA1),    // 파랑
        InputMode::Unknown => (0x52, 0x52, 0x5B),     // 회색
    }
}

/// 배지 치수(px). 글자 크기와 글자 폭에서 계산한다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeMetrics {
    pub width: f64,
    pub height: f64,
    pub corner_radius: f64,
}

/// `text_width`: 실제 글꼴로 잰 라벨 폭.
/// 한 글자 라벨은 정사각형에 가깝게, 두 글자 이상(예: "RU")은 좌우 여백을 두고 늘린다.
pub fn badge_metrics(font_size: f64, text_width: f64) -> BadgeMetrics {
    let height = (font_size * 1.55).round();
    let width = (text_width + font_size * 0.5).max(height).round();
    BadgeMetrics {
        width,
        height,
        corner_radius: (height * 0.28).round(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_character_badge_is_square() {
        let metrics = badge_metrics(13.0, 13.0);
        assert_eq!(metrics.width, metrics.height);
    }

    #[test]
    fn wide_label_grows_horizontally() {
        let narrow = badge_metrics(13.0, 13.0);
        let wide = badge_metrics(13.0, 30.0);
        assert!(wide.width > narrow.width);
        assert_eq!(wide.height, narrow.height);
    }

    #[test]
    fn each_mode_family_has_distinct_color() {
        use crate::core::state::{ChineseScript, JapaneseKana};
        let colors = [
            badge_rgb(&InputMode::Hangul),
            badge_rgb(&InputMode::Latin),
            badge_rgb(&InputMode::Japanese(JapaneseKana::Hiragana)),
            badge_rgb(&InputMode::Chinese(ChineseScript::Simplified)),
            badge_rgb(&InputMode::Other("RU".into())),
        ];
        for (i, a) in colors.iter().enumerate() {
            for b in &colors[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }
}
