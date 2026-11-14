//! 작성자: Git 이력 참조
//! 작성목적: 메뉴바(macOS)·알림 영역(Windows) 아이콘과 설정 메뉴
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | 표시 켜기/끄기, 크기, 불투명도, 위치, 전환 강조, 종료 메뉴 추가
//! - 2026-11-14 | 4단계 macOS 권한 안내 | 손쉬운 사용 권한 상태·설정 열기 항목 추가(macOS 전용)
//! - 2026-11-14 | 5단계 Windows 오버레이 | 모드 글자 아이콘 이미지 지원(Windows는 제목 글자를 표시하지 못함)
//!
//! 별도 설정 창(WebView)을 두지 않고 이 메뉴가 설정 화면 역할을 한다.
//! 메뉴 클릭은 tray-icon의 전역 채널로 들어오므로, 앱 루프가 주기적으로 `poll`을 호출해 꺼낸다.
//! 체크 항목은 클릭하면 muda가 스스로 체크를 뒤집는다. 여러 개 중 하나를 고르는 항목은
//! 처리 후 `sync`로 설정 값과 체크 상태를 다시 맞춘다.

use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::core::placement::BadgePosition;
use crate::core::settings::{BadgeSize, Settings};

/// 메뉴 처리 결과 중 앱이 따로 해야 할 일
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// 설정이 바뀜(저장 필요)
    SettingsChanged,
    /// 시스템 설정의 손쉬운 사용 화면 열기(macOS)
    OpenAccessibilitySettings,
    Quit,
}

#[cfg(target_os = "macos")]
const PERMISSION_GRANTED_TEXT: &str = "캐럿 추적: 손쉬운 사용 권한 있음";
#[cfg(target_os = "macos")]
const PERMISSION_NEEDED_TEXT: &str = "손쉬운 사용 권한 허용… (캐럿 옆 표시에 필요)";

const SIZE_CHOICES: [(BadgeSize, &str); 3] = [
    (BadgeSize::Small, "작게"),
    (BadgeSize::Medium, "보통"),
    (BadgeSize::Large, "크게"),
];
const OPACITY_CHOICES: [(f64, &str); 4] =
    [(0.5, "50%"), (0.7, "70%"), (0.85, "85%"), (1.0, "100%")];
const POSITION_CHOICES: [(BadgePosition, &str); 2] = [
    (BadgePosition::Below, "캐럿 아래"),
    (BadgePosition::Right, "캐럿 오른쪽"),
];

pub struct Tray {
    icon: TrayIcon,
    show: CheckMenuItem,
    sizes: Vec<(BadgeSize, CheckMenuItem)>,
    opacities: Vec<(f64, CheckMenuItem)>,
    positions: Vec<(BadgePosition, CheckMenuItem)>,
    emphasize: CheckMenuItem,
    /// 손쉬운 사용 권한 상태 겸 설정 열기 항목. 권한이 있으면 비활성화한다.
    #[cfg(target_os = "macos")]
    permission: MenuItem,
    quit: MenuItem,
}

fn tooltip(mode_label: &str) -> String {
    format!("한영 표시기: {mode_label}")
}

fn choice_items<T: Copy>(choices: &[(T, &str)]) -> Vec<(T, CheckMenuItem)> {
    choices
        .iter()
        .map(|(value, text)| (*value, CheckMenuItem::new(*text, true, false, None)))
        .collect()
}

fn submenu<T>(title: &str, items: &[(T, CheckMenuItem)]) -> Result<Submenu, String> {
    let refs: Vec<&dyn IsMenuItem> = items
        .iter()
        .map(|(_, item)| item as &dyn IsMenuItem)
        .collect();
    Submenu::with_items(title, true, &refs).map_err(|error| error.to_string())
}

fn find<T: Copy>(items: &[(T, CheckMenuItem)], id: &MenuId) -> Option<T> {
    items
        .iter()
        .find(|(_, item)| item.id() == id)
        .map(|(value, _)| *value)
}

impl Tray {
    /// 아이콘과 메뉴를 만든다. macOS에서는 NSApplication 이벤트 루프가 돌기 시작한 뒤 호출해야 한다.
    /// `icon`: 모드 글자 이미지(RGBA, 정사각형 한 변 길이). Windows는 아이콘 없이 만들 수 없어 필수다.
    pub fn new(
        settings: &Settings,
        mode_label: &str,
        icon: Option<(Vec<u8>, u32)>,
    ) -> Result<Self, String> {
        let show = CheckMenuItem::new("입력 상태 배지 표시", true, settings.enabled, None);
        let sizes = choice_items(&SIZE_CHOICES);
        let opacities = choice_items(&OPACITY_CHOICES);
        let positions = choice_items(&POSITION_CHOICES);
        let emphasize = CheckMenuItem::new(
            "전환할 때 크게 강조",
            true,
            settings.emphasize_on_change,
            None,
        );
        let quit = MenuItem::new("한영 표시기 종료", true, None);

        let size_menu = submenu("크기", &sizes)?;
        let opacity_menu = submenu("불투명도", &opacities)?;
        let position_menu = submenu("위치", &positions)?;
        let separator_top = PredefinedMenuItem::separator();
        let separator_bottom = PredefinedMenuItem::separator();
        let mut items: Vec<&dyn IsMenuItem> = Vec::new();
        #[cfg(target_os = "macos")]
        let permission = MenuItem::new(PERMISSION_NEEDED_TEXT, true, None);
        #[cfg(target_os = "macos")]
        let separator_permission = PredefinedMenuItem::separator();
        #[cfg(target_os = "macos")]
        items.extend([
            &permission as &dyn IsMenuItem,
            &separator_permission as &dyn IsMenuItem,
        ]);
        items.extend([
            &show as &dyn IsMenuItem,
            &separator_top,
            &size_menu,
            &opacity_menu,
            &position_menu,
            &emphasize,
            &separator_bottom,
            &quit,
        ]);
        let menu = Menu::with_items(&items).map_err(|error| error.to_string())?;

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_title(mode_label)
            .with_tooltip(tooltip(mode_label));
        if let Some((rgba, size)) = icon {
            builder = builder
                .with_icon(Icon::from_rgba(rgba, size, size).map_err(|error| error.to_string())?);
        }
        let icon = builder.build().map_err(|error| error.to_string())?;

        let tray = Self {
            icon,
            show,
            sizes,
            opacities,
            positions,
            emphasize,
            #[cfg(target_os = "macos")]
            permission,
            quit,
        };
        tray.sync(settings);
        Ok(tray)
    }

    /// 메뉴바에 현재 입력 모드 글자를 표시한다(macOS). Windows는 제목을 무시하므로 툴팁만 바뀐다.
    pub fn set_mode_label(&self, label: &str) {
        self.icon.set_title(Some(label));
        let _ = self.icon.set_tooltip(Some(tooltip(label)));
    }

    /// 아이콘 그림을 바꾼다(Windows 알림 영역용). 실패하면 이전 그림을 유지한다.
    pub fn set_mode_icon(&self, rgba: Vec<u8>, size: u32) {
        if let Ok(icon) = Icon::from_rgba(rgba, size, size) {
            let _ = self.icon.set_icon(Some(icon));
        }
    }

    /// 손쉬운 사용 권한 상태를 메뉴에 반영한다(macOS).
    #[cfg(target_os = "macos")]
    pub fn set_accessibility_granted(&self, granted: bool) {
        self.permission.set_text(if granted {
            PERMISSION_GRANTED_TEXT
        } else {
            PERMISSION_NEEDED_TEXT
        });
        self.permission.set_enabled(!granted);
    }

    /// 체크 상태를 설정 값에 맞춘다.
    pub fn sync(&self, settings: &Settings) {
        self.show.set_checked(settings.enabled);
        self.emphasize.set_checked(settings.emphasize_on_change);
        for (value, item) in &self.sizes {
            item.set_checked(*value == settings.size);
        }
        for (value, item) in &self.opacities {
            item.set_checked((*value - settings.opacity).abs() < 0.001);
        }
        for (value, item) in &self.positions {
            item.set_checked(*value == settings.position);
        }
    }

    /// 쌓인 메뉴 클릭을 모두 처리한다. 종료가 있으면 Quit, 설정이 바뀌었으면 SettingsChanged.
    pub fn poll(&self, settings: &mut Settings) -> Option<TrayCommand> {
        let mut result = None;
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id();
            if id == self.quit.id() {
                return Some(TrayCommand::Quit);
            }
            #[cfg(target_os = "macos")]
            if id == self.permission.id() {
                return Some(TrayCommand::OpenAccessibilitySettings);
            }
            if id == self.show.id() {
                settings.enabled = !settings.enabled;
            } else if id == self.emphasize.id() {
                settings.emphasize_on_change = !settings.emphasize_on_change;
            } else if let Some(size) = find(&self.sizes, id) {
                settings.size = size;
            } else if let Some(opacity) = find(&self.opacities, id) {
                settings.opacity = opacity;
            } else if let Some(position) = find(&self.positions, id) {
                settings.position = position;
            } else {
                continue;
            }
            result = Some(TrayCommand::SettingsChanged);
        }
        if result.is_some() {
            self.sync(settings);
        }
        result
    }
}
