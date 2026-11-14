//! 작성자: Git 이력 참조
//! 작성목적: Windows 앱 본체. 입력 모드 폴링, 캐럿 추적, 배지 표시, 알림 영역 메뉴를 하나의 루프로 묶는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 5·6단계 Windows 앱 | 메시지 루프, 100ms 스레드 타이머, Win32 캐럿 → UIA → 마우스 순 위치 결정
//! - 2026-11-14 | 7단계 자동 실행 | 메뉴로 로그인 시 자동 실행 켜기/끄기, 시작할 때 등록 경로 갱신
//! - 2026-11-14 | 입력 시작 시 표시 | 표시 방식(입력 시작 시/항상) 적용, 입력란 판정과 입력란 영역 기준점
//!
//! 실행 구조(모두 한 스레드):
//! - Win32 메시지 루프(GetMessageW). tray-icon 메뉴와 배지 창 그리기도 이 루프로 처리된다.
//! - 100ms 스레드 타이머(SetTimer + TIMERPROC): 메뉴 처리, 입력 모드 폴링, 캐럿 추적, 배지 이동
//!   Windows에는 입력 모드 변경 알림이 없어 폴링으로 감지한다(전환 반응은 최대 약 100ms 지연).
//! - UI Automation 객체는 이 스레드(STA)에서 만들고 이 스레드에서만 쓴다.
//!
//! 검증 상태: macOS에서 교차 컴파일 검사만 했다. Windows 실기 실행은 미검증.

use std::cell::RefCell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, SetTimer, TranslateMessage,
};

use super::caret::{self, UiaCaret};
use super::ime;
use super::overlay::{BadgeOverlay, dpi_for_point, monitor_work_areas};
use super::render::render_badge_rgba;
use crate::autostart;
use crate::core::placement::{Anchor, estimated_caret_in_field, place_badge};
use crate::core::settings::{DisplayMode, Settings, default_settings_path};
use crate::core::state::InputMode;
use crate::core::state::ScreenRect;
use crate::core::style::{EMPHASIS_DURATION, badge_look, badge_rgb};
use crate::core::tracker::ModeTracker;
use crate::core::visibility::{CaretMark, InputContext, VisibilityTracker};
use crate::tray::{Tray, TrayCommand};

/// 입력 모드 폴링·캐럿 추적 주기(ms)
const TICK_INTERVAL_MS: u32 = 100;
/// 알림 영역 아이콘 이미지 한 변(px). Windows가 DPI에 맞춰 축소한다.
const TRAY_ICON_SIZE: i32 = 32;

struct App {
    overlay: BadgeOverlay,
    uia: Option<UiaCaret>,
    tray: Option<Tray>,
    tray_label: String,
    tracker: ModeTracker,
    settings: Settings,
    settings_path: Option<PathBuf>,
    debug: bool,
    last_debug_line: String,
    visibility: VisibilityTracker,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn tray_icon_image(mode: &InputMode) -> (Vec<u8>, u32) {
    (
        render_badge_rgba(mode.badge_label(), badge_rgb(mode), TRAY_ICON_SIZE),
        TRAY_ICON_SIZE as u32,
    )
}

impl App {
    fn emphasis_duration(&self) -> Duration {
        if self.settings.emphasize_on_change {
            EMPHASIS_DURATION
        } else {
            Duration::ZERO
        }
    }

    /// 메뉴 클릭을 처리한다. 종료를 골랐으면 true.
    fn handle_menu(&mut self) -> bool {
        let Some(tray) = self.tray.as_ref() else {
            return false;
        };
        match tray.poll(&mut self.settings) {
            Some(TrayCommand::Quit) => true,
            Some(TrayCommand::SettingsChanged) => {
                self.apply_launch_at_login();
                self.save_settings();
                false
            }
            // macOS 전용 항목이라 Windows 메뉴에는 없다.
            Some(TrayCommand::OpenAccessibilitySettings) | None => false,
        }
    }

    /// 자동 실행 등록 상태를 설정에 맞춘다. 실패하면 설정과 메뉴 체크를 되돌린다.
    fn apply_launch_at_login(&mut self) {
        if let Err(error) = autostart::set_enabled(self.settings.launch_at_login) {
            eprintln!("자동 실행 설정을 바꾸지 못함: {error}");
            self.settings.launch_at_login = autostart::is_enabled();
            if let Some(tray) = &self.tray {
                tray.sync(&self.settings);
            }
        }
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path
            && let Err(error) = self.settings.save(path)
        {
            eprintln!("설정을 저장하지 못함 ({}): {error}", path.display());
        }
    }

    fn tick(&mut self) {
        let now = Instant::now();
        let snapshot = ime::snapshot();
        // 전경 창이 없으면(잠금 화면 전환 중 등) 이전 모드를 유지한다.
        if let Some(snapshot) = &snapshot {
            let duration = self.emphasis_duration();
            self.tracker.observe(snapshot.mode.clone(), now, duration);
        }
        let mode = self
            .tracker
            .current()
            .cloned()
            .unwrap_or(InputMode::Unknown);

        if self.tray_label != mode.badge_label() {
            self.tray_label = mode.badge_label().to_string();
            if let Some(tray) = &self.tray {
                tray.set_mode_label(&self.tray_label);
                let (rgba, size) = tray_icon_image(&mode);
                tray.set_mode_icon(rgba, size);
            }
        }

        if !self.settings.enabled {
            self.overlay.hide();
            return;
        }

        // 입력란 판정과 기준점: Win32 캐럿 → UIA 입력란(캐럿 → 입력란 영역) → 마우스
        let (context, caret_rect) = self.focused_input(snapshot.as_ref());
        let visible = match self.settings.display {
            DisplayMode::Always => true,
            DisplayMode::OnInputStart => {
                self.visibility
                    .update(now, context, self.tracker.last_change())
            }
        };
        if !visible {
            self.overlay.hide();
            self.debug_log(format!("[debug] mode={mode} context={context:?} 숨김"));
            return;
        }
        let anchor = caret_rect.map(Anchor::Caret).unwrap_or_else(|| {
            let (x, y) = caret::mouse_location();
            Anchor::Mouse { x, y }
        });
        let (anchor_x, anchor_y) = match anchor {
            Anchor::Caret(rect) => (rect.x, rect.y),
            Anchor::Mouse { x, y } => (x, y),
        };

        let emphasized = self.tracker.is_emphasized(now);
        let look = badge_look(&mode, &self.settings, emphasized);
        self.overlay
            .apply_look(&look, dpi_for_point(anchor_x, anchor_y));
        let (width, height) = self.overlay.size();
        let monitors = monitor_work_areas();
        let (x, y) = place_badge(anchor, self.settings.position, width, height, &monitors);
        self.overlay.move_to(x, y);

        if self.debug {
            let raw = snapshot.as_ref().map_or("전경 창 없음".to_string(), |s| {
                format!(
                    "class={} lang=0x{:04X} open={:?} conversion={:?}",
                    s.focus_class, s.lang_id, s.open_status, s.conversion_mode
                )
            });
            self.debug_log(format!(
                "[debug] mode={mode} {raw} context={context:?} anchor={anchor:?} badge=({x:.0},{y:.0} {width:.0}x{height:.0})"
            ));
        }
    }

    /// 포커스가 입력란인지와 배지 기준 사각형(캐럿 또는 입력란 영역에서 추정한 캐럿)을 구한다.
    fn focused_input(
        &self,
        snapshot: Option<&ime::ImeSnapshot>,
    ) -> (InputContext, Option<ScreenRect>) {
        // Win32 시스템 캐럿이 있으면 텍스트 입력 중인 전통 Win32 컨트롤이다.
        if let Some(snapshot) = snapshot
            && let Some(rect) = caret::win32_caret(&snapshot.focus)
        {
            let key = snapshot.focus.focus.0 as usize as u64;
            let context = InputContext::Field {
                key,
                caret: Some(CaretMark::Point(rect.x, rect.y)),
            };
            return (context, Some(rect));
        }
        let Some(uia) = &self.uia else {
            return (InputContext::Unknown, None);
        };
        match uia.focused_input() {
            Ok(Some(input)) => {
                let caret = input.caret.ok();
                let context = InputContext::Field {
                    key: input.key,
                    caret: caret.map(|rect| CaretMark::Point(rect.x, rect.y)),
                };
                let rect = caret.or_else(|| input.field_rect.map(estimated_caret_in_field));
                (context, rect)
            }
            Ok(None) => (InputContext::NotField, None),
            Err(_) => (InputContext::Unknown, None),
        }
    }

    /// HANYEONG_DEBUG=1 일 때, 직전과 다른 줄만 출력한다.
    fn debug_log(&mut self, line: String) {
        if self.debug && line != self.last_debug_line {
            eprintln!("{line}");
            self.last_debug_line = line;
        }
    }
}

unsafe extern "system" fn on_timer(_window: HWND, _message: u32, _id: usize, _time: u32) {
    let mut should_quit = false;
    APP.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        let Some(app) = borrowed.as_mut() else {
            return;
        };
        if app.handle_menu() {
            should_quit = true;
            return;
        }
        app.tick();
    });
    if should_quit {
        unsafe { PostQuitMessage(0) };
    }
}

/// 앱을 실행한다. 종료할 때까지 돌아오지 않는다.
pub fn run() {
    caret::init_dpi_awareness();

    let settings_path = default_settings_path();
    let settings = match &settings_path {
        Some(path) => {
            let (settings, warning) = Settings::load(path);
            if let Some(warning) = warning {
                eprintln!("{warning}");
            }
            settings
        }
        None => Settings::default(),
    };

    // 자동 실행이 켜져 있으면 지금 실행 파일 경로로 다시 등록한다(앱을 옮기거나 업데이트한 경우 대비).
    if settings.launch_at_login
        && let Err(error) = autostart::set_enabled(true)
    {
        eprintln!("자동 실행 등록을 갱신하지 못함: {error}");
    }

    let overlay = match BadgeOverlay::new() {
        Ok(overlay) => overlay,
        Err(error) => {
            eprintln!("배지 창을 만들지 못함: {error}");
            std::process::exit(1);
        }
    };
    let uia = match UiaCaret::new() {
        Ok(uia) => Some(uia),
        Err(error) => {
            eprintln!("UI Automation을 초기화하지 못함(Win32 캐럿과 마우스 위치만 사용): {error}");
            None
        }
    };

    let mut tracker = ModeTracker::default();
    let initial_mode = ime::snapshot().map_or(InputMode::Unknown, |s| s.mode);
    tracker.observe(initial_mode.clone(), Instant::now(), Duration::ZERO);
    let tray_label = initial_mode.badge_label().to_string();
    let tray = match Tray::new(&settings, &tray_label, Some(tray_icon_image(&initial_mode))) {
        Ok(tray) => Some(tray),
        Err(error) => {
            eprintln!("알림 영역 아이콘을 만들지 못함: {error}");
            None
        }
    };

    APP.with(|cell| {
        *cell.borrow_mut() = Some(App {
            overlay,
            uia,
            tray,
            tray_label,
            tracker,
            settings,
            settings_path,
            debug: std::env::var_os("HANYEONG_DEBUG").is_some_and(|value| value == "1"),
            last_debug_line: String::new(),
            visibility: VisibilityTracker::default(),
        });
    });

    unsafe {
        SetTimer(None, 0, TICK_INTERVAL_MS, Some(on_timer));
        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}
