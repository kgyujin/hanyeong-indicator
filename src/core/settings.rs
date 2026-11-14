//! 작성자: Git 이력 참조
//! 작성목적: 사용자 설정 정의와 TOML 파일 로드·저장
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 2단계 core 모듈 | 설정 구조체, 로드(누락·손상 시 기본값), 원자적 저장 추가
//! - 2026-11-14 | 4단계 macOS 권한 안내 | 손쉬운 사용 권한 안내창 표시 여부 저장
//!
//! 파일 위치: OS 표준 설정 폴더의 `config.toml`
//!   macOS   ~/Library/Application Support/dev.hanyeong.hanyeong-indicator/config.toml
//!   Windows %APPDATA%\hanyeong\hanyeong-indicator\config\config.toml
//! 설정 화면은 메뉴바/트레이 메뉴이고, 이 파일은 그 상태를 기억하는 용도다.
//! 사용자가 파일을 직접 고쳐 값이 잘못돼도 앱이 멈추지 않도록, 읽을 수 없는 항목은 기본값으로 대체한다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::placement::BadgePosition;

/// 배지 크기 단계
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgeSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl BadgeSize {
    /// 평상시 배지 글자 크기(pt). 배지 크기는 글자 크기에서 정한다.
    pub fn font_size(self) -> f64 {
        match self {
            BadgeSize::Small => 11.0,
            BadgeSize::Medium => 13.0,
            BadgeSize::Large => 16.0,
        }
    }
}

/// 투명도 허용 범위. 0에 가까우면 배지가 안 보여 켜져 있는지 알 수 없다.
pub const MIN_OPACITY: f64 = 0.3;
pub const MAX_OPACITY: f64 = 1.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 배지 표시 여부
    pub enabled: bool,
    pub size: BadgeSize,
    /// 평상시 불투명도(0.3~1.0). 전환 강조 중에는 1.0
    pub opacity: f64,
    pub position: BadgePosition,
    /// 입력 모드가 바뀐 순간 배지를 잠깐 크게 강조
    pub emphasize_on_change: bool,
    pub launch_at_login: bool,
    /// macOS 손쉬운 사용 권한 안내창을 이미 띄웠는지.
    /// 거부한 사용자에게 실행할 때마다 안내창을 띄우지 않도록 처음 한 번만 띄운다.
    pub accessibility_prompt_shown: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            size: BadgeSize::default(),
            opacity: 0.85,
            position: BadgePosition::default(),
            emphasize_on_change: true,
            launch_at_login: false,
            accessibility_prompt_shown: false,
        }
    }
}

impl Settings {
    /// 범위를 벗어난 값을 허용 범위로 고친다.
    pub fn normalized(mut self) -> Self {
        self.opacity = if self.opacity.is_finite() {
            self.opacity.clamp(MIN_OPACITY, MAX_OPACITY)
        } else {
            Settings::default().opacity
        };
        self
    }

    /// TOML 문자열을 읽는다. 문법 오류면 기본값과 오류 설명을 돌려준다.
    /// 일부 키만 잘못된 형식이어도 파일 전체가 오류가 되므로, 이때는 기본값으로 시작하고 사용자가 메뉴로 다시 고르게 한다.
    pub fn from_toml(text: &str) -> (Settings, Option<String>) {
        match toml::from_str::<Settings>(text) {
            Ok(settings) => (settings.normalized(), None),
            Err(error) => (Settings::default(), Some(error.to_string())),
        }
    }

    pub fn to_toml(&self) -> String {
        // Settings는 직렬화가 실패할 수 없는 단순 값만 가진다.
        toml::to_string_pretty(self).expect("Settings는 항상 TOML로 직렬화된다")
    }

    /// 파일에서 읽는다. 파일이 없으면 기본값(경고 없음), 읽기·문법 오류면 기본값과 경고.
    pub fn load(path: &Path) -> (Settings, Option<String>) {
        match fs::read_to_string(path) {
            Ok(text) => {
                let (settings, error) = Settings::from_toml(&text);
                (
                    settings,
                    error.map(|e| format!("설정 파일 형식 오류, 기본값 사용: {e}")),
                )
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Settings::default(), None),
            Err(error) => (
                Settings::default(),
                Some(format!("설정 파일을 읽지 못함, 기본값 사용: {error}")),
            ),
        }
    }

    /// 임시 파일에 쓴 뒤 이름을 바꿔, 저장 도중 앱이 꺼져도 설정 파일이 반쯤 쓰인 상태로 남지 않게 한다.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("toml.tmp");
        fs::write(&temporary, self.to_toml())?;
        fs::rename(&temporary, path)
    }
}

/// OS 표준 설정 파일 경로. 홈 폴더를 알 수 없으면 None.
pub fn default_settings_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "hanyeong", "hanyeong-indicator")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "hanyeong-settings-test-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_file_gives_defaults_without_warning() {
        let dir = temp_dir("missing");
        let (settings, warning) = Settings::load(&dir.join("config.toml"));
        assert_eq!(settings, Settings::default());
        assert!(warning.is_none());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("nested").join("config.toml");
        let settings = Settings {
            enabled: false,
            size: BadgeSize::Large,
            opacity: 0.5,
            position: BadgePosition::Right,
            emphasize_on_change: false,
            launch_at_login: true,
            accessibility_prompt_shown: true,
        };
        settings.save(&path).unwrap();
        let (loaded, warning) = Settings::load(&path);
        assert_eq!(loaded, settings);
        assert!(warning.is_none());
        assert!(!path.with_extension("toml.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn partial_file_fills_missing_keys_with_defaults() {
        let (settings, warning) = Settings::from_toml("size = \"small\"\n");
        assert!(warning.is_none());
        assert_eq!(settings.size, BadgeSize::Small);
        assert_eq!(settings.opacity, Settings::default().opacity);
        assert!(settings.enabled);
    }

    #[test]
    fn corrupted_file_gives_defaults_with_warning() {
        let (settings, warning) = Settings::from_toml("size = [this is not toml");
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
    }

    #[test]
    fn unknown_enum_value_gives_defaults_with_warning() {
        let (settings, warning) = Settings::from_toml("position = \"diagonal\"\n");
        assert_eq!(settings, Settings::default());
        assert!(warning.is_some());
    }

    #[test]
    fn opacity_is_clamped() {
        assert_eq!(Settings::from_toml("opacity = 0.0").0.opacity, MIN_OPACITY);
        assert_eq!(Settings::from_toml("opacity = 3.5").0.opacity, MAX_OPACITY);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let (settings, warning) = Settings::from_toml("future_option = true\nsize = \"large\"\n");
        assert!(warning.is_none());
        assert_eq!(settings.size, BadgeSize::Large);
    }
}
