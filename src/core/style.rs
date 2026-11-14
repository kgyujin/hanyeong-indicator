//! 작성자: Git 이력 참조
//! 작성목적: 배지 색과 크기 규칙. OS별 오버레이가 같은 모양을 그리도록 한곳에 둔다.
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | 모드별 색, 배지 치수, 강조 배율·시간 추가
//! - 2026-11-14 | 5단계 Windows 오버레이 | OS 공통 배지 모양(BadgeLook) 계산을 macOS 오버레이에서 옮겨 옴
//! - 2026-11-14 | 사용자 피드백 반영 | 시스템 기본 모양(BadgeFill::System) 추가, 언어별 색은 선택 사항으로

use std::time::Duration;

use super::settings::{BadgeStyle, Settings};
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

/// 배지 배경을 칠하는 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeFill {
    /// OS 기본 모양. 밝은 모드에서는 밝은 회색 바탕에 검은 글자, 어두운 모드에서는 반대.
    /// macOS 기본 입력 소스 표시기와 비슷하게 보이도록 한다.
    System,
    /// 언어별 색 바탕에 흰 글자
    Colored((u8, u8, u8)),
}

/// 배지 하나의 겉모습. OS별 오버레이가 이 값대로 그린다.
#[derive(Debug, Clone, PartialEq)]
pub struct BadgeLook {
    pub label: String,
    pub fill: BadgeFill,
    /// 글자 크기(논리 pt). Windows는 모니터 DPI 배율을 곱해 픽셀로 바꾼다.
    pub font_size: f64,
    /// 창 전체 불투명도(0~1)
    pub opacity: f64,
}

/// 현재 모드·설정·강조 여부로 배지 모양을 정한다.
/// 강조 중에는 글자를 키우고 완전히 불투명하게 해 전환을 눈에 띄게 한다.
pub fn badge_look(mode: &InputMode, settings: &Settings, emphasized: bool) -> BadgeLook {
    let scale = if emphasized { EMPHASIS_SCALE } else { 1.0 };
    BadgeLook {
        label: mode.badge_label().to_string(),
        fill: match settings.style {
            BadgeStyle::System => BadgeFill::System,
            BadgeStyle::Colored => BadgeFill::Colored(badge_rgb(mode)),
        },
        font_size: settings.size.font_size() * scale,
        opacity: if emphasized { 1.0 } else { settings.opacity },
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

/// 둥근 사각형 안에서 픽셀 (x, y)가 덮이는 비율(0~1). 모서리 경계는 0.5px 폭으로 부드럽게 한다.
/// GDI는 글자·채우기에 알파를 쓰지 않으므로, Windows 알림 영역 아이콘의 투명 모서리를 이 값으로 만든다.
pub fn rounded_rect_coverage(x: f64, y: f64, width: f64, height: f64, radius: f64) -> f64 {
    // 픽셀 중심 기준
    let px = x + 0.5;
    let py = y + 0.5;
    let nearest_x = px.clamp(radius, width - radius);
    let nearest_y = py.clamp(radius, height - radius);
    let distance = ((px - nearest_x).powi(2) + (py - nearest_y).powi(2)).sqrt();
    (radius + 0.5 - distance).clamp(0.0, 1.0)
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
    fn rounded_rect_coverage_inside_corner_and_outside() {
        // 32x32, 반지름 8
        assert_eq!(rounded_rect_coverage(16.0, 16.0, 32.0, 32.0, 8.0), 1.0);
        assert_eq!(rounded_rect_coverage(16.0, 0.0, 32.0, 32.0, 8.0), 1.0);
        assert_eq!(rounded_rect_coverage(0.0, 0.0, 32.0, 32.0, 8.0), 0.0);
        let edge = rounded_rect_coverage(2.0, 2.0, 32.0, 32.0, 8.0);
        assert!(edge > 0.0 && edge < 1.0, "모서리 경계는 부분 투명: {edge}");
    }

    #[test]
    fn style_setting_selects_fill() {
        let system = badge_look(&InputMode::Hangul, &Settings::default(), false);
        assert_eq!(system.fill, BadgeFill::System);
        let colored_settings = Settings {
            style: BadgeStyle::Colored,
            ..Settings::default()
        };
        let colored = badge_look(&InputMode::Hangul, &colored_settings, false);
        assert_eq!(
            colored.fill,
            BadgeFill::Colored(badge_rgb(&InputMode::Hangul))
        );
    }

    #[test]
    fn emphasized_look_is_larger_and_opaque() {
        let settings = Settings {
            opacity: 0.5,
            ..Settings::default()
        };
        let normal = badge_look(&InputMode::Hangul, &settings, false);
        let emphasized = badge_look(&InputMode::Hangul, &settings, true);
        assert_eq!(normal.label, "한");
        assert_eq!(normal.opacity, 0.5);
        assert_eq!(emphasized.opacity, 1.0);
        assert_eq!(emphasized.font_size, normal.font_size * EMPHASIS_SCALE);
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
