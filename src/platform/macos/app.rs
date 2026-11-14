//! 작성자: Git 이력 참조
//! 작성목적: macOS 앱 본체. 입력 모드 감지, 캐럿 추적, 배지 표시, 메뉴바 메뉴를 하나의 루프로 묶는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | NSApplication 루프, 100ms 타이머, 입력 소스 알림 연결
//! - 2026-11-14 | 4단계 macOS 권한 안내 | 최초 1회 권한 안내창, 권한 상태 메뉴 반영, 설정 화면 열기
//!
//! 실행 구조(모두 메인 스레드):
//! - NSApplication.run(): Dock 아이콘 없는 Accessory 앱으로 실행
//! - 입력 소스 변경 분산 알림: 즉시 모드를 다시 읽고 배지를 갱신(전환 반응 속도 담당)
//! - 100ms 반복 타이머: 메뉴 클릭 처리, 모드 폴링(알림 누락 대비), 캐럿 추적과 배지 이동
//!   타이머는 common 모드에 등록해 메뉴가 열려 있는 동안에도 돈다.
//!
//! 손쉬운 사용 권한:
//! - 처음 실행할 때 한 번만 시스템 안내창을 띄운다(거부한 사용자를 반복해서 귀찮게 하지 않기 위해).
//! - 권한은 매 틱 다시 확인한다. 사용자가 시스템 설정에서 허용하면 재시작 없이 캐럿 추적으로 바뀐다.
//! - 권한이 없으면 배지는 마우스 옆에 뜨고, 메뉴 맨 위 항목으로 설정 화면을 열 수 있다.
//!
//! 주의: 캐럿 조회(AX)는 응답 없는 앱에 대해 요청당 최대 50ms를 기다린다.

use std::cell::RefCell;
use std::ffi::c_void;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use core_foundation::date::CFDate;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopTimer, CFRunLoopTimerRef, kCFRunLoopCommonModes,
};
use core_foundation::string::CFStringRef;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSWorkspace};
use objc2_foundation::{NSString, NSURL};

use super::caret;
use super::ime;
use super::overlay::{BadgeOverlay, visible_screen_frames};
use crate::core::placement::{Anchor, place_badge};
use crate::core::settings::{Settings, default_settings_path};
use crate::core::state::InputMode;
use crate::core::style::{EMPHASIS_DURATION, badge_look};
use crate::core::tracker::ModeTracker;
use crate::tray::{Tray, TrayCommand};

/// 캐럿 추적·메뉴 처리 주기
const TICK_INTERVAL: Duration = Duration::from_millis(100);
/// 시스템 설정 > 개인정보 보호 및 보안 > 손쉬운 사용 화면
const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

struct App {
    mtm: MainThreadMarker,
    overlay: BadgeOverlay,
    tray: Option<Tray>,
    tray_error_reported: bool,
    tray_label: String,
    tracker: ModeTracker,
    settings: Settings,
    settings_path: Option<PathBuf>,
    /// 환경 변수 HANYEONG_DEBUG=1 이면 배지 기준점과 좌표가 바뀔 때마다 stderr에 출력
    debug: bool,
    last_debug_line: String,
    /// 마지막으로 메뉴에 반영한 손쉬운 사용 권한 상태
    accessibility_granted: Option<bool>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with_app(action: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            action(app);
        }
    });
}

impl App {
    fn emphasis_duration(&self) -> Duration {
        if self.settings.emphasize_on_change {
            EMPHASIS_DURATION
        } else {
            Duration::ZERO
        }
    }

    fn observe_mode(&mut self) {
        let duration = self.emphasis_duration();
        self.tracker
            .observe(ime::current_mode(), Instant::now(), duration);
    }

    fn ensure_tray(&mut self) {
        if self.tray.is_some() || self.tray_error_reported {
            return;
        }
        match Tray::new(&self.settings, &self.tray_label, None) {
            Ok(tray) => {
                if self.debug {
                    eprintln!("[debug] 메뉴바 아이콘 생성: {}", self.tray_label);
                }
                self.tray = Some(tray);
            }
            Err(error) => {
                eprintln!("메뉴바 아이콘을 만들지 못함: {error}");
                self.tray_error_reported = true;
            }
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
                self.save_settings();
                false
            }
            Some(TrayCommand::OpenAccessibilitySettings) => {
                open_accessibility_settings();
                false
            }
            None => false,
        }
    }

    /// 권한 상태가 바뀌었으면 메뉴에 반영한다. 메뉴가 아직 없으면 다음 틱에 다시 시도한다.
    fn refresh_accessibility(&mut self) {
        let granted = caret::is_trusted();
        if self.accessibility_granted == Some(granted) {
            return;
        }
        let Some(tray) = &self.tray else {
            return;
        };
        tray.set_accessibility_granted(granted);
        if self.debug {
            eprintln!(
                "[debug] 손쉬운 사용 권한: {}",
                if granted { "있음" } else { "없음" }
            );
        }
        self.accessibility_granted = Some(granted);
    }

    fn save_settings(&self) {
        if let Some(path) = &self.settings_path
            && let Err(error) = self.settings.save(path)
        {
            eprintln!("설정을 저장하지 못함 ({}): {error}", path.display());
        }
    }

    fn render(&mut self) {
        let now = Instant::now();
        let mode = self
            .tracker
            .current()
            .cloned()
            .unwrap_or(InputMode::Unknown);

        if self.tray_label != mode.badge_label() {
            self.tray_label = mode.badge_label().to_string();
            if let Some(tray) = &self.tray {
                tray.set_mode_label(&self.tray_label);
            }
        }

        if !self.settings.enabled {
            self.overlay.hide();
            return;
        }

        let emphasized = self.tracker.is_emphasized(now);
        self.overlay
            .apply_look(&badge_look(&mode, &self.settings, emphasized));

        let anchor = match caret::focused_caret_rect() {
            Ok(rect) => Anchor::Caret(rect),
            Err(_) => {
                let (x, y) = caret::mouse_location();
                Anchor::Mouse { x, y }
            }
        };
        let (width, height) = self.overlay.size();
        let monitors = visible_screen_frames(self.mtm);
        let (x, y) = place_badge(anchor, self.settings.position, width, height, &monitors);
        self.overlay.move_to(x, y);

        if self.debug {
            let line = format!(
                "[debug] mode={mode} anchor={anchor:?} badge=({x:.0},{y:.0} {width:.0}x{height:.0}) monitors={monitors:?}"
            );
            if line != self.last_debug_line {
                eprintln!("{line}");
                self.last_debug_line = line;
            }
        }
    }
}

extern "C" fn on_input_source_changed(
    _center: *const c_void,
    _observer: *const c_void,
    _name: CFStringRef,
    _object: *const c_void,
    _user_info: CFDictionaryRef,
) {
    with_app(|app| {
        app.observe_mode();
        app.render();
    });
}

extern "C" fn on_tick(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    let mut should_quit = false;
    with_app(|app| {
        app.ensure_tray();
        if app.handle_menu() {
            should_quit = true;
            return;
        }
        app.refresh_accessibility();
        app.observe_mode();
        app.render();
    });
    if should_quit {
        // terminate는 앱 상태를 빌린 채로 부르지 않는다(종료 중 콜백 재진입 방지).
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).terminate(None);
        }
    }
}

fn open_accessibility_settings() {
    let opened = NSURL::URLWithString(&NSString::from_str(ACCESSIBILITY_SETTINGS_URL))
        .is_some_and(|url| NSWorkspace::sharedWorkspace().openURL(&url));
    if !opened {
        eprintln!("시스템 설정의 손쉬운 사용 화면을 열지 못함");
    }
}

/// 앱을 실행한다. 메인 스레드에서 호출해야 하며 종료할 때까지 돌아오지 않는다.
pub fn run() {
    let mtm = MainThreadMarker::new().expect("한영 표시기는 메인 스레드에서 시작해야 합니다");
    let application = NSApplication::sharedApplication(mtm);
    application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let settings_path = default_settings_path();
    let mut settings = match &settings_path {
        Some(path) => {
            let (settings, warning) = Settings::load(path);
            if let Some(warning) = warning {
                eprintln!("{warning}");
            }
            settings
        }
        None => Settings::default(),
    };

    let mut prompted = false;
    if !caret::is_trusted() && !settings.accessibility_prompt_shown {
        caret::prompt_for_trust();
        settings.accessibility_prompt_shown = true;
        prompted = true;
    }

    let mut app = App {
        mtm,
        overlay: BadgeOverlay::new(mtm),
        tray: None,
        tray_error_reported: false,
        tray_label: String::new(),
        tracker: ModeTracker::default(),
        settings,
        settings_path,
        debug: std::env::var_os("HANYEONG_DEBUG").is_some_and(|value| value == "1"),
        last_debug_line: String::new(),
        accessibility_granted: None,
    };
    if prompted {
        app.save_settings();
    }
    app.observe_mode();
    app.tray_label = app
        .tracker
        .current()
        .map_or("?".into(), |mode| mode.badge_label().to_string());
    APP.with(|cell| *cell.borrow_mut() = Some(app));

    ime::subscribe_input_source_changed(on_input_source_changed);

    let interval = TICK_INTERVAL.as_secs_f64();
    let timer = CFRunLoopTimer::new(
        CFDate::now().abs_time() + interval,
        interval,
        0,
        0,
        on_tick,
        std::ptr::null_mut(),
    );
    CFRunLoop::get_current().add_timer(&timer, unsafe { kCFRunLoopCommonModes });

    application.run();
}
