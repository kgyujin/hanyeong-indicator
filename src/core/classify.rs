//! 작성자: Git 이력 참조
//! 작성목적: OS가 돌려주는 원시 입력기 정보를 InputMode로 분류하는 순수 함수 모음
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | macOS 입력 소스 ID, Windows IME 변환 모드 분류 추가
//! - 2026-11-14 | 다국어 표시 | 일본어(히라가나/가타카나/반각), 중국어(간체/번체) 분류 추가.
//!   Windows에서 IME가 닫혀 있으면 변환 모드 조회 실패와 관계없이 영문으로 분류
//!
//! OS API를 호출하지 않으므로 어느 플랫폼에서든 단위 테스트할 수 있다.

use super::state::{ChineseScript, InputMode, JapaneseKana};

/// 영문 배지("A")로 보여줄 라틴 문자권 언어 코드.
/// 이 목록과 한국어·일본어·중국어에 없는 언어는 언어 코드를 라벨로 쓴다(예: 러시아어 → "RU").
const LATIN_SCRIPT_LANGUAGES: &[&str] = &[
    "en", "de", "fr", "es", "it", "pt", "nl", "sv", "da", "nb", "nn", "no", "fi", "is", "pl", "cs",
    "sk", "hu", "ro", "hr", "sl", "tr", "ca", "et", "lv", "lt", "id", "ms", "vi",
];

/// 서드파티 한글 입력기 "구름"의 입력 소스 ID 접두사.
/// 구름은 한글 모드 ID가 `.han2`, `.han3final`처럼 `han`으로 시작하고, 영문 모드는 `.system`, `.qwerty` 등이다.
const GUREUM_PREFIX: &str = "org.youknowone.inputmethod.Gureum.";

/// macOS 입력 소스를 분류한다.
///
/// - `source_id`: `kTISPropertyInputSourceID` 값 (예: `com.apple.inputmethod.Korean.2SetKorean`)
/// - `primary_language`: `kTISPropertyInputSourceLanguages`의 첫 항목 (예: `ko`, `zh-Hans`)
///
/// 한계: 중국어 병음 입력기 안에서 Shift로 바꾸는 영문 모드는 입력 소스 ID가 바뀌지 않아
/// TIS로 구분할 수 없다. Caps Lock으로 ABC 입력 소스로 바꾸는 경우는 구분된다.
pub fn classify_macos_source(source_id: &str, primary_language: Option<&str>) -> InputMode {
    if let Some(mode) = source_id.strip_prefix(GUREUM_PREFIX) {
        return if mode.starts_with("han") {
            InputMode::Hangul
        } else {
            InputMode::Latin
        };
    }

    // CJK 입력기 안의 로마자 모드는 영문으로 본다.
    // 예: 일본어 `...Japanese.Roman`(英字), `...Japanese.FullWidthRoman`(全角英字)
    if source_id.ends_with("Roman") {
        return InputMode::Latin;
    }

    match primary_language.map(language_base) {
        Some("ko") => InputMode::Hangul,
        Some("ja") => InputMode::Japanese(macos_japanese_kana(source_id)),
        Some("zh" | "yue") => InputMode::Chinese(macos_chinese_script(
            source_id,
            primary_language.unwrap_or_default(),
        )),
        Some(lang) if LATIN_SCRIPT_LANGUAGES.contains(&lang) => InputMode::Latin,
        Some(lang) if !lang.is_empty() => InputMode::Other(lang.to_ascii_uppercase()),
        // 언어 정보가 없는 키보드 레이아웃은 대부분 사용자 지정 라틴 레이아웃이다.
        _ if source_id.starts_with("com.apple.keylayout.") => InputMode::Latin,
        _ => InputMode::Unknown,
    }
}

/// 일본어 입력 모드 ID 끝부분으로 가나 종류를 정한다.
/// 예: `com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese.Katakana`
fn macos_japanese_kana(source_id: &str) -> JapaneseKana {
    if source_id.ends_with(".HalfWidthKana") {
        JapaneseKana::HalfWidthKatakana
    } else if source_id.ends_with(".Katakana") {
        JapaneseKana::Katakana
    } else {
        JapaneseKana::Hiragana
    }
}

/// 언어 태그(`zh-Hans`/`zh-Hant`/`yue-Hant`)로 간체·번체를 정한다.
/// 태그에 문자 체계가 없으면 Apple 번체 입력기 묶음(`.TCIM.`) 여부로 정한다.
fn macos_chinese_script(source_id: &str, language_tag: &str) -> ChineseScript {
    if language_tag.contains("Hant") {
        ChineseScript::Traditional
    } else if language_tag.contains("Hans") {
        ChineseScript::Simplified
    } else if source_id.contains(".TCIM.") {
        ChineseScript::Traditional
    } else {
        ChineseScript::Simplified
    }
}

/// `zh-Hans`, `en_US` 같은 언어 태그에서 기본 언어 코드만 꺼낸다.
fn language_base(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

/// Windows LANGID의 기본 언어 번호(`PRIMARYLANGID`, 하위 10비트)
const LANG_CHINESE: u16 = 0x04;
const LANG_ENGLISH: u16 = 0x09;
const LANG_JAPANESE: u16 = 0x11;
const LANG_KOREAN: u16 = 0x12;
/// 간체를 쓰는 중국어 LANGID: 중국(0x0804), 싱가포르(0x1004).
/// 나머지(대만 0x0404, 홍콩 0x0C04, 마카오 0x1404)는 번체로 본다.
const SIMPLIFIED_CHINESE_LANG_IDS: &[u16] = &[0x0804, 0x1004];

/// IME 변환 모드 비트 (imm.h)
/// NATIVE: 한국어=한글, 일본어=가나, 중국어=중문 입력
const IME_CMODE_NATIVE: u32 = 0x0001;
const IME_CMODE_KATAKANA: u32 = 0x0002;
const IME_CMODE_FULLSHAPE: u32 = 0x0008;

/// Windows 입력 상태를 분류한다.
///
/// - `lang_id`: `GetKeyboardLayout`이 돌려준 HKL의 하위 16비트(LANGID)
/// - `open_status`: `IMC_GETOPENSTATUS` 결과. 조회 실패 시 `None`
/// - `conversion_mode`: `IMC_GETCONVERSIONMODE` 결과. 조회 실패 시 `None`
///
/// 공통 규칙(한·일·중 IME):
/// 1. 열림 상태가 닫힘(false)이면 직접 입력이므로 영문
/// 2. 변환 모드를 못 읽으면 Unknown
/// 3. 변환 모드에 NATIVE 비트가 있으면 해당 언어, 없으면 영문
///
/// 구 IME는 변환 모드가, Windows 11 새 IME는 열림 상태가 바뀐다는 보고가 있어 두 값을 함께 본다.
/// Windows 실측 결과에 따라 이 규칙을 조정한다.
pub fn classify_windows(
    lang_id: u16,
    open_status: Option<bool>,
    conversion_mode: Option<u32>,
) -> InputMode {
    match lang_id & 0x03ff {
        LANG_KOREAN => classify_ime(open_status, conversion_mode, |_| InputMode::Hangul),
        LANG_JAPANESE => classify_ime(open_status, conversion_mode, |mode| {
            InputMode::Japanese(windows_japanese_kana(mode))
        }),
        LANG_CHINESE => {
            let script = if SIMPLIFIED_CHINESE_LANG_IDS.contains(&lang_id) {
                ChineseScript::Simplified
            } else {
                ChineseScript::Traditional
            };
            classify_ime(open_status, conversion_mode, |_| InputMode::Chinese(script))
        }
        LANG_ENGLISH => InputMode::Latin,
        _ => InputMode::Other(format!("{lang_id:04X}")),
    }
}

fn classify_ime(
    open_status: Option<bool>,
    conversion_mode: Option<u32>,
    native: impl FnOnce(u32) -> InputMode,
) -> InputMode {
    match (open_status, conversion_mode) {
        (Some(false), _) => InputMode::Latin,
        (_, None) => InputMode::Unknown,
        (_, Some(mode)) if mode & IME_CMODE_NATIVE != 0 => native(mode),
        (_, Some(_)) => InputMode::Latin,
    }
}

/// 일본어 IME 변환 모드로 가나 종류를 정한다.
/// 예: 히라가나 0x19, 전각 가타카나 0x1B, 반각 가타카나 0x13
fn windows_japanese_kana(conversion_mode: u32) -> JapaneseKana {
    if conversion_mode & IME_CMODE_KATAKANA == 0 {
        JapaneseKana::Hiragana
    } else if conversion_mode & IME_CMODE_FULLSHAPE != 0 {
        JapaneseKana::Katakana
    } else {
        JapaneseKana::HalfWidthKatakana
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_korean_ime_is_hangul() {
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.Korean.2SetKorean", Some("ko")),
            InputMode::Hangul
        );
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.Korean.3SetKorean", Some("ko")),
            InputMode::Hangul
        );
    }

    #[test]
    fn macos_abc_layout_is_latin() {
        assert_eq!(
            classify_macos_source("com.apple.keylayout.ABC", Some("en")),
            InputMode::Latin
        );
        assert_eq!(
            classify_macos_source("com.apple.keylayout.German", Some("de")),
            InputMode::Latin
        );
    }

    #[test]
    fn macos_layout_without_language_is_latin() {
        assert_eq!(
            classify_macos_source("com.apple.keylayout.Custom", None),
            InputMode::Latin
        );
    }

    #[test]
    fn macos_other_language_uses_language_label() {
        assert_eq!(
            classify_macos_source("com.apple.keylayout.Russian", Some("ru")),
            InputMode::Other("RU".into())
        );
    }

    const KOTOERI: &str = "com.apple.inputmethod.Kotoeri.RomajiTyping";

    #[test]
    fn macos_japanese_kana_modes() {
        assert_eq!(
            classify_macos_source(&format!("{KOTOERI}.Japanese"), Some("ja")),
            InputMode::Japanese(JapaneseKana::Hiragana)
        );
        assert_eq!(
            classify_macos_source(&format!("{KOTOERI}.Japanese.Katakana"), Some("ja")),
            InputMode::Japanese(JapaneseKana::Katakana)
        );
        assert_eq!(
            classify_macos_source(&format!("{KOTOERI}.Japanese.HalfWidthKana"), Some("ja")),
            InputMode::Japanese(JapaneseKana::HalfWidthKatakana)
        );
    }

    #[test]
    fn macos_japanese_roman_modes_are_latin() {
        assert_eq!(
            classify_macos_source(&format!("{KOTOERI}.Roman"), Some("ja")),
            InputMode::Latin
        );
        assert_eq!(
            classify_macos_source(&format!("{KOTOERI}.Japanese.FullWidthRoman"), Some("ja")),
            InputMode::Latin
        );
    }

    #[test]
    fn macos_chinese_script_from_language_tag() {
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.SCIM.ITABC", Some("zh-Hans")),
            InputMode::Chinese(ChineseScript::Simplified)
        );
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.TCIM.Zhuyin", Some("zh-Hant")),
            InputMode::Chinese(ChineseScript::Traditional)
        );
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.TYIM.Cangjie", Some("yue-Hant")),
            InputMode::Chinese(ChineseScript::Traditional)
        );
    }

    #[test]
    fn macos_chinese_script_without_script_subtag() {
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.TCIM.Cangjie", Some("zh")),
            InputMode::Chinese(ChineseScript::Traditional)
        );
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.SCIM.WBX", Some("zh")),
            InputMode::Chinese(ChineseScript::Simplified)
        );
    }

    #[test]
    fn macos_cjk_roman_mode_is_latin() {
        assert_eq!(
            classify_macos_source("com.apple.inputmethod.Japanese.Roman", Some("ja")),
            InputMode::Latin
        );
    }

    #[test]
    fn macos_gureum_modes() {
        assert_eq!(
            classify_macos_source("org.youknowone.inputmethod.Gureum.han2", Some("ko")),
            InputMode::Hangul
        );
        assert_eq!(
            classify_macos_source("org.youknowone.inputmethod.Gureum.han3final", None),
            InputMode::Hangul
        );
        assert_eq!(
            classify_macos_source("org.youknowone.inputmethod.Gureum.system", Some("ko")),
            InputMode::Latin
        );
    }

    #[test]
    fn macos_unknown_source_without_language() {
        assert_eq!(
            classify_macos_source("com.example.inputmethod.Mystery", None),
            InputMode::Unknown
        );
    }

    const KO_KR: u16 = 0x0412;
    const EN_US: u16 = 0x0409;
    const JA_JP: u16 = 0x0411;

    #[test]
    fn windows_korean_native_bit_is_hangul() {
        assert_eq!(
            classify_windows(KO_KR, Some(true), Some(0x0001)),
            InputMode::Hangul
        );
        // 다른 비트(예: IME_CMODE_FULLSHAPE 0x8)가 함께 켜져 있어도 NATIVE 비트만 본다.
        assert_eq!(
            classify_windows(KO_KR, Some(true), Some(0x0009)),
            InputMode::Hangul
        );
    }

    #[test]
    fn windows_korean_without_native_bit_is_latin() {
        assert_eq!(
            classify_windows(KO_KR, Some(true), Some(0x0000)),
            InputMode::Latin
        );
    }

    #[test]
    fn windows_korean_closed_ime_is_latin() {
        assert_eq!(
            classify_windows(KO_KR, Some(false), Some(0x0001)),
            InputMode::Latin
        );
    }

    #[test]
    fn windows_open_status_unknown_falls_back_to_conversion_mode() {
        assert_eq!(
            classify_windows(KO_KR, None, Some(0x0001)),
            InputMode::Hangul
        );
        assert_eq!(
            classify_windows(KO_KR, None, Some(0x0000)),
            InputMode::Latin
        );
    }

    #[test]
    fn windows_query_failure_is_unknown() {
        assert_eq!(classify_windows(KO_KR, None, None), InputMode::Unknown);
        assert_eq!(
            classify_windows(KO_KR, Some(true), None),
            InputMode::Unknown
        );
    }

    #[test]
    fn windows_closed_ime_is_latin_even_without_conversion_mode() {
        assert_eq!(classify_windows(KO_KR, Some(false), None), InputMode::Latin);
        assert_eq!(classify_windows(JA_JP, Some(false), None), InputMode::Latin);
    }

    #[test]
    fn windows_non_cjk_layouts() {
        assert_eq!(classify_windows(EN_US, None, None), InputMode::Latin);
        assert_eq!(
            classify_windows(RU_RU, Some(true), Some(0x0001)),
            InputMode::Other("0419".into())
        );
    }

    const RU_RU: u16 = 0x0419;
    const ZH_CN: u16 = 0x0804;
    const ZH_SG: u16 = 0x1004;
    const ZH_TW: u16 = 0x0404;
    const ZH_HK: u16 = 0x0C04;

    #[test]
    fn windows_japanese_kana_modes() {
        assert_eq!(
            classify_windows(JA_JP, Some(true), Some(0x19)),
            InputMode::Japanese(JapaneseKana::Hiragana)
        );
        assert_eq!(
            classify_windows(JA_JP, Some(true), Some(0x1B)),
            InputMode::Japanese(JapaneseKana::Katakana)
        );
        assert_eq!(
            classify_windows(JA_JP, Some(true), Some(0x13)),
            InputMode::Japanese(JapaneseKana::HalfWidthKatakana)
        );
    }

    #[test]
    fn windows_japanese_alphanumeric_modes_are_latin() {
        // 전각 영숫자(0x18), 반각 영숫자(0x10): NATIVE 비트 없음
        assert_eq!(
            classify_windows(JA_JP, Some(true), Some(0x18)),
            InputMode::Latin
        );
        assert_eq!(
            classify_windows(JA_JP, Some(true), Some(0x10)),
            InputMode::Latin
        );
    }

    #[test]
    fn windows_chinese_script_by_region() {
        for lang in [ZH_CN, ZH_SG] {
            assert_eq!(
                classify_windows(lang, Some(true), Some(0x0001)),
                InputMode::Chinese(ChineseScript::Simplified)
            );
        }
        for lang in [ZH_TW, ZH_HK] {
            assert_eq!(
                classify_windows(lang, Some(true), Some(0x0001)),
                InputMode::Chinese(ChineseScript::Traditional)
            );
        }
    }

    #[test]
    fn windows_chinese_english_mode_is_latin() {
        assert_eq!(
            classify_windows(ZH_CN, Some(true), Some(0x0000)),
            InputMode::Latin
        );
        assert_eq!(
            classify_windows(ZH_TW, Some(false), Some(0x0001)),
            InputMode::Latin
        );
    }
}
