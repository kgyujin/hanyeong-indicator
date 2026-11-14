//! 작성자: Git 이력 참조
//! 작성목적: 표시할 입력 모드와 화면 좌표 타입 정의
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | InputMode, ScreenRect 추가
//! - 2026-11-14 | 다국어 표시 | 일본어(あ/ア/ｱ), 중국어(简/繁) 입력 모드 추가
//!
//! 좌표 규칙: 모든 좌표는 "주 모니터 좌상단 원점, y는 아래로 증가, 단위는 논리 픽셀"이다.
//! macOS AX API와 CGEvent가 이 규칙을 따르고, Windows는 Per-Monitor DPI 인식 상태의 화면 좌표를 그대로 쓴다.
//! macOS Cocoa 창 좌표(좌하단 원점)로의 변환은 오버레이 구현에서 한다.

use std::fmt;

/// 일본어 입력기의 가나 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JapaneseKana {
    Hiragana,
    Katakana,
    /// 반각 가타카나
    HalfWidthKatakana,
}

/// 중국어 입력기의 문자 체계
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChineseScript {
    /// 간체 (병음, 오필 등)
    Simplified,
    /// 번체 (주음, 창힐, 번체 병음 등)
    Traditional,
}

/// 현재 키보드 입력 모드.
/// CJK 입력기 안의 영문 모드(일본어 英数, 중국어 英文 등)는 Latin으로 분류한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// 한글 입력 중
    Hangul,
    /// 영문(라틴 문자) 입력 중
    Latin,
    /// 일본어 가나 입력 중
    Japanese(JapaneseKana),
    /// 중국어 입력 중
    Chinese(ChineseScript),
    /// 위 언어 외 입력 소스. 값은 화면에 보여줄 짧은 라벨(예: 러시아어 "RU")
    Other(String),
    /// 상태를 읽지 못함(예: Windows 관리자 권한 창)
    Unknown,
}

impl InputMode {
    /// 배지에 표시할 글자
    pub fn badge_label(&self) -> &str {
        match self {
            InputMode::Hangul => "한",
            InputMode::Latin => "A",
            InputMode::Japanese(JapaneseKana::Hiragana) => "あ",
            InputMode::Japanese(JapaneseKana::Katakana) => "ア",
            InputMode::Japanese(JapaneseKana::HalfWidthKatakana) => "ｱ",
            InputMode::Chinese(ChineseScript::Simplified) => "简",
            InputMode::Chinese(ChineseScript::Traditional) => "繁",
            InputMode::Other(label) => label,
            InputMode::Unknown => "?",
        }
    }
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.badge_label())
    }
}

/// 화면 좌표 사각형(위 좌표 규칙 참고)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl fmt::Display for ScreenRect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({:.0},{:.0} {:.0}x{:.0})",
            self.x, self.y, self.width, self.height
        )
    }
}

/// 캐럿 위치를 어디서 얻었는지(진단 출력과 폴백 판단용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretSource {
    /// macOS 손쉬운 사용 API
    Accessibility,
    /// Windows GetGUIThreadInfo (Win32 시스템 캐럿)
    Win32Caret,
    /// Windows UI Automation TextPattern2
    UiAutomation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_label_matches_mode() {
        assert_eq!(InputMode::Hangul.badge_label(), "한");
        assert_eq!(InputMode::Latin.badge_label(), "A");
        assert_eq!(
            InputMode::Japanese(JapaneseKana::Hiragana).badge_label(),
            "あ"
        );
        assert_eq!(
            InputMode::Japanese(JapaneseKana::Katakana).badge_label(),
            "ア"
        );
        assert_eq!(
            InputMode::Japanese(JapaneseKana::HalfWidthKatakana).badge_label(),
            "ｱ"
        );
        assert_eq!(
            InputMode::Chinese(ChineseScript::Simplified).badge_label(),
            "简"
        );
        assert_eq!(
            InputMode::Chinese(ChineseScript::Traditional).badge_label(),
            "繁"
        );
        assert_eq!(InputMode::Other("RU".into()).badge_label(), "RU");
        assert_eq!(InputMode::Unknown.badge_label(), "?");
    }
}
