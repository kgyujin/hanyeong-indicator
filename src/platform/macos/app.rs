//! 작성자: Git 이력 참조
//! 작성목적: macOS 앱 본체. 입력 모드 감지, 캐럿 추적, 배지 표시, 메뉴바 메뉴를 하나의 루프로 묶는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | NSApplication 루프, 100ms 타이머, 입력 소스 알림 연결
//! - 2026-11-14 | 4단계 macOS 권한 안내 | 최초 1회 권한 안내창, 권한 상태 메뉴 반영, 설정 화면 열기
//! - 2026-11-14 | 입력 시작 시 표시 | 표시 방식(입력 시작 시/항상) 적용, 입력란 영역 기준점 폴백
//! - 2026-11-14 | 7단계 자동 실행 | 메뉴로 로그인 시 자동 실행 켜기/끄기, 시작할 때 등록 경로 갱신
//! - 2026-11-14 | 인식률·반응 속도 개선 | 앞 앱 AXObserver 연결(포커스·선택 변경 즉시 갱신), Electron 접근성 자동 활성화, 캐럿 좌표 기반 타이핑 판정
//!
//! 실행 구조(모두 메인 스레드):
//! - NSApplication.run(): Dock 아이콘 없는 Accessory 앱으로 실행
//! - 입력 소스 변경 분산 알림: 즉시 모드를 다시 읽고 배지를 갱신(전환 반응 속도 담당)
//! - 앞 앱의 AX 알림(포커스 이동, 텍스트 선택 변경): 입력란 진입·타이핑을 즉시 반영(observer 모듈)
//! - 100ms 반복 타이머: 메뉴 클릭 처리, 앞 앱 변경 확인, 모드 폴링(알림 누락 대비), 캐럿 추적과 배지 이동
//!   타이머는 common 모드에 등록해 메뉴가 열려 있는 동안에도 돈다.
//!
//! 손쉬운 사용 권한:
//! - 처음 실행할 때 한 번만 시스템 안내창을 띄운다(거부한 사용자를 반복해서 귀찮게 하지 않기 위해).
//! - 권한은 매 틱 다시 확인한다. 사용자가 시스템 설정에서 허용하면 재시작 없이 캐럿 추적으로 바뀐다.
//! - 권한이 없으면 배지는 마우스 옆에 뜨고, 메뉴 맨 위 항목으로 설정 화면을 열 수 있다.
//!
//! 주의: 캐럿 조회(AX)는 응답 없는 앱에 대해 요청당 최대 50ms를 기다린다.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use core_foundation::base::CFTypeRef;
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
use super::ffi::AXUIElementRef;
use super::ime;
use super::observer::AppObserver;
use super::overlay::{BadgeOverlay, visible_screen_frames};
use crate::autostart;
use crate::core::placement::{Anchor, estimated_caret_in_field, place_badge};
use crate::core::settings::{DisplayMode, Settings, default_settings_path};
use crate::core::state::InputMode;
use crate::core::style::{EMPHASIS_DURATION, badge_look};
use crate::core::tracker::ModeTracker;
use crate::core::visibility::{CaretMark, InputContext, VisibilityTracker};
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
    visibility: VisibilityTracker,
    /// 마지막으로 메뉴에 반영한 손쉬운 사용 권한 상태
    accessibility_granted: Option<bool>,
    /// 앞 앱의 포커스·선택 변경 알림 관찰자
    observer: Option<AppObserver>,
    /// AXManualAccessibility를 이미 보낸 앱
    accessibility_requested: HashSet<i32>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// 앱 상태로 작업한다. 이미 다른 콜백이 쓰는 중이면(재진입) 건너뛴다.
/// 다음 알림이나 100ms 타이머가 곧 다시 갱신하므로 한 번 건너뛰어도 문제없다.
fn with_app(action: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Ok(mut borrowed) = cell.try_borrow_mut()
            && let Some(app) = borrowed.as_mut()
        {
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
                self.apply_launch_at_login();
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

    /// 앞 앱이 바뀌었으면 AX 알림 관찰자를 새 앱에 다시 연결한다.
    /// 처음 보는 앱에는 AXManualAccessibility를 보내 Electron 앱이 접근성 트리(입력란·캐럿 정보)를 만들게 한다.
    /// Electron이 아닌 앱은 이 값을 지원하지 않는다는 오류(-25205)를 돌려줄 뿐 영향이 없다.
    fn follow_frontmost_app(&mut self) {
        if self.accessibility_granted != Some(true) {
            self.observer = None;
            return;
        }
        let Some(pid) = NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .map(|app| app.processIdentifier())
        else {
            return;
        };
        if self
            .observer
            .as_ref()
            .is_some_and(|observer| observer.pid() == pid)
        {
            return;
        }
        if self.accessibility_requested.insert(pid) {
            let result = caret::enable_manual_accessibility(pid);
            self.debug_log(format!(
                "[debug] pid={pid} AXManualAccessibility 결과={result}"
            ));
        }
        // 이전 관찰자를 먼저 해제한 뒤 새로 연결한다.
        self.observer = None;
        match AppObserver::attach(pid, on_ax_notification) {
            Ok(observer) => self.observer = Some(observer),
            Err(error) => self.debug_log(format!(
                "[debug] pid={pid} AX 알림 등록 실패 AXError={error}"
            )),
        }
    }

    /// 배지를 갱신하고, 디버그 모드에서는 오래 걸린 갱신을 기록한다.
    fn render_timed(&mut self, reason: &str) {
        let started = Instant::now();
        self.render();
        let elapsed = started.elapsed();
        if elapsed >= Duration::from_millis(15) {
            self.debug_log(format!("[debug] 갱신({reason}) {}ms", elapsed.as_millis()));
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

        let input = caret::focused_input();
        let context = match &input {
            Ok(input) => InputContext::Field {
                key: input.key,
                // 글자 위치가 있으면 글자 수로, 없으면 캐럿 화면 좌표로 타이핑을 판정한다.
                caret: input.caret_index.map(CaretMark::Index).or_else(|| {
                    input
                        .caret
                        .ok()
                        .map(|rect| CaretMark::Point(rect.x, rect.y))
                }),
            },
            // 권한이 없으면 입력란인지 알 수 없다.
            Err(caret::CaretError::NotTrusted) => InputContext::Unknown,
            Err(_) => InputContext::NotField,
        };
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

        let emphasized = self.tracker.is_emphasized(now);
        self.overlay
            .apply_look(&badge_look(&mode, &self.settings, emphasized));

        // 기준점: 캐럿 → 입력란 영역 → 마우스
        let anchor = match &input {
            Ok(input) => match (input.caret, input.field_rect) {
                (Ok(rect), _) => Some(Anchor::Caret(rect)),
                (Err(_), Some(field)) => Some(Anchor::Caret(estimated_caret_in_field(field))),
                (Err(_), None) => None,
            },
            Err(_) => None,
        }
        .unwrap_or_else(|| {
            let (x, y) = caret::mouse_location();
            Anchor::Mouse { x, y }
        });
        let (width, height) = self.overlay.size();
        let monitors = visible_screen_frames(self.mtm);
        let (x, y) = place_badge(anchor, self.settings.position, width, height, &monitors);
        self.overlay.move_to(x, y);

        self.debug_log(format!(
            "[debug] mode={mode} context={context:?} anchor={anchor:?} badge=({x:.0},{y:.0} {width:.0}x{height:.0}) monitors={monitors:?}"
        ));
    }

    /// HANYEONG_DEBUG=1 일 때, 직전과 다른 줄만 출력한다.
    fn debug_log(&mut self, line: String) {
        if self.debug && line != self.last_debug_line {
            eprintln!("{line}");
            self.last_debug_line = line;
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

extern "C" fn on_ax_notification(
    _observer: CFTypeRef,
    _element: AXUIElementRef,
    _notification: CFStringRef,
    _refcon: *mut c_void,
) {
    with_app(|app| app.render_timed("AX 알림"));
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
        app.follow_frontmost_app();
        app.observe_mode();
        app.render_timed("타이머");
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

    // 자동 실행이 켜져 있으면 지금 실행 파일 경로로 다시 등록한다(앱을 옮기거나 업데이트한 경우 대비).
    if settings.launch_at_login
        && let Err(error) = autostart::set_enabled(true)
    {
        eprintln!("자동 실행 등록을 갱신하지 못함: {error}");
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
        visibility: VisibilityTracker::default(),
        observer: None,
        accessibility_requested: HashSet::new(),
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
