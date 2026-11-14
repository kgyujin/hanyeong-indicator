//! 작성자: Git 이력 참조
//! 작성목적: 메뉴바(macOS)·알림 영역(Windows) 아이콘과 설정 메뉴
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | 표시 켜기/끄기, 크기, 불투명도, 위치, 전환 강조, 종료 메뉴 추가
//!
//! 별도 설정 창(WebView)을 두지 않고 이 메뉴가 설정 화면 역할을 한다.
//! 메뉴 클릭은 tray-icon의 전역 채널로 들어오므로, 앱 루프가 주기적으로 `poll`을 호출해 꺼낸다.
//! 체크 항목은 클릭하면 muda가 스스로 체크를 뒤집는다. 여러 개 중 하나를 고르는 항목은
//! 처리 후 `sync`로 설정 값과 체크 상태를 다시 맞춘다.

use tray_icon::menu::{
    CheckMenuItem, IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::core::placement::BadgePosition;
use crate::core::settings::{BadgeSize, Settings};

/// 메뉴 처리 결과 중 앱이 따로 해야 할 일
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    /// 설정이 바뀜(저장 필요)
    SettingsChanged,
    Quit,
}

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
    quit: MenuItem,
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
    pub fn new(settings: &Settings, mode_label: &str) -> Result<Self, String> {
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
        let menu = Menu::with_items(&[
            &show,
            &separator_top,
            &size_menu,
            &opacity_menu,
            &position_menu,
            &emphasize,
            &separator_bottom,
            &quit,
        ])
        .map_err(|error| error.to_string())?;

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_title(mode_label)
            .with_tooltip("한영 표시기")
            .build()
            .map_err(|error| error.to_string())?;

        let tray = Self {
            icon,
            show,
            sizes,
            opacities,
            positions,
            emphasize,
            quit,
        };
        tray.sync(settings);
        Ok(tray)
    }

    /// 메뉴바에 현재 입력 모드 글자를 표시한다(macOS). Windows는 아이콘 그림으로 바꿀 예정(5단계).
    pub fn set_mode_label(&self, label: &str) {
        self.icon.set_title(Some(label));
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
