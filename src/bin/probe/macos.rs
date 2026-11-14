//! macOS 프로브 실행부.
//!
//! 메인 스레드에서 CFRunLoop를 돌린다. TIS API가 메인 스레드 전용이고, 분산 알림도 런루프로 전달되기 때문이다.
//! - 분산 알림 콜백: 입력 소스 변경을 즉시 출력([notify])
//! - 런루프 타이머: 입력 모드 폴링([poll])과 캐럿 조회([caret]/[fallback])
//! - --self-test: 타이머가 상태 기계를 돌려 입력 소스를 바꾸고 감지 지연을 잰다

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::time::{Duration, Instant};

use core_foundation::date::CFDate;
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::runloop::{
    CFRunLoop, CFRunLoopTimer, CFRunLoopTimerRef, kCFRunLoopDefaultMode,
};
use core_foundation::string::CFStringRef;

use hanyeong_indicator::core::state::ScreenRect;
use hanyeong_indicator::platform::macos::caret::{self, CaretError};
use hanyeong_indicator::platform::macos::ime::{self, InputSourceInfo};

use crate::{Options, log};

/// self-test에서 입력 소스 하나를 유지하는 시간. 알림이 늦게 와도 잡을 수 있게 넉넉히 둔다.
const SELF_TEST_STEP: Duration = Duration::from_millis(700);
/// self-test 중 타이머 주기. 지연 측정 해상도를 정한다.
const SELF_TEST_TICK: Duration = Duration::from_millis(5);

#[derive(Debug, Clone, PartialEq)]
enum CaretReport {
    Caret(ScreenRect),
    Fallback(CaretError),
}

struct StepResult {
    target: String,
    notify_ms: Option<f64>,
    visible_ms: Option<f64>,
}

struct SelfTest {
    targets: Vec<String>,
    index: usize,
    step_started: Option<Instant>,
    notify_ms: Option<f64>,
    visible_ms: Option<f64>,
    results: Vec<StepResult>,
}

struct State {
    start: Instant,
    options: Options,
    last_source: Option<InputSourceInfo>,
    last_caret: Option<CaretReport>,
    last_pid: Option<i32>,
    electron_fixed: HashSet<i32>,
    self_test: Option<SelfTest>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

fn with_state<R>(action: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|cell| cell.borrow_mut().as_mut().map(action))
}

fn describe(info: &Option<InputSourceInfo>) -> String {
    match info {
        Some(info) => format!(
            "mode={} id={} lang={} name={}",
            info.mode(),
            info.id,
            info.primary_language.as_deref().unwrap_or("-"),
            info.localized_name.as_deref().unwrap_or("-"),
        ),
        None => "mode=? (입력 소스 조회 실패)".into(),
    }
}

/// 캐럿 이동이 이 값(px)보다 작으면 출력하지 않는다.
fn moved_enough(previous: &ScreenRect, current: &ScreenRect) -> bool {
    (previous.x - current.x).abs() >= 1.0
        || (previous.y - current.y).abs() >= 1.0
        || (previous.height - current.height).abs() >= 1.0
}

extern "C" fn on_input_source_changed(
    _center: *const c_void,
    _observer: *const c_void,
    _name: CFStringRef,
    _object: *const c_void,
    _user_info: CFDictionaryRef,
) {
    let current = ime::current_input_source();
    with_state(|state| {
        if let Some(test) = state.self_test.as_mut()
            && let (Some(started), None) = (test.step_started, test.notify_ms)
        {
            test.notify_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        }
        log(state.start, format!("[notify] {}", describe(&current)));
        state.last_source = current;
    });
}

fn poll_mode(state: &mut State) {
    let current = ime::current_input_source();
    let changed =
        current.as_ref().map(|info| &info.id) != state.last_source.as_ref().map(|info| &info.id);
    if changed {
        log(state.start, format!("[poll] {}", describe(&current)));
        state.last_source = current;
    }
}

fn poll_focus_app(state: &mut State) {
    let pid = caret::focused_app_pid();
    if pid == state.last_pid {
        return;
    }
    state.last_pid = pid;
    let Some(pid) = pid else {
        log(state.start, "[app] 포커스된 앱 없음");
        return;
    };
    log(state.start, format!("[app] pid={pid}"));
    if state.options.electron_fix && state.electron_fixed.insert(pid) {
        let error = caret::enable_manual_accessibility(pid);
        log(
            state.start,
            format!(
                "[app] AXManualAccessibility=true 설정 결과 AXError={error} (0=성공, -25205=지원 안 함)"
            ),
        );
    }
}

fn poll_caret(state: &mut State) {
    let report = match caret::focused_caret_rect() {
        Ok(rect) => CaretReport::Caret(rect),
        Err(error) => CaretReport::Fallback(error),
    };
    let should_print = match (&state.last_caret, &report) {
        (Some(CaretReport::Caret(previous)), CaretReport::Caret(current)) => {
            moved_enough(previous, current)
        }
        (previous, current) => previous.as_ref() != Some(current),
    };
    if should_print {
        match &report {
            CaretReport::Caret(rect) => log(state.start, format!("[caret] ax {rect}")),
            CaretReport::Fallback(error) => {
                let (x, y) = caret::mouse_location();
                log(
                    state.start,
                    format!("[fallback] {error:?} → 마우스 ({x:.0},{y:.0})"),
                );
            }
        }
    }
    state.last_caret = Some(report);
}

/// self-test 한 틱. 끝나면 true.
fn self_test_tick(state: &mut State) -> bool {
    let start = state.start;
    let Some(test) = state.self_test.as_mut() else {
        return true;
    };

    match test.step_started {
        None => {
            let target = test.targets[test.index].clone();
            log(start, format!("[self-test] 선택 → {target}"));
            test.notify_ms = None;
            test.visible_ms = None;
            test.step_started = Some(Instant::now());
            if let Err(message) = ime::select_input_source(&target) {
                log(start, format!("[self-test] {message}"));
            }
        }
        Some(started) => {
            if test.visible_ms.is_none() {
                let current = ime::current_input_source();
                if current.as_ref().map(|info| info.id.as_str())
                    == Some(test.targets[test.index].as_str())
                {
                    test.visible_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
                }
            }
            if started.elapsed() >= SELF_TEST_STEP {
                test.results.push(StepResult {
                    target: test.targets[test.index].clone(),
                    notify_ms: test.notify_ms,
                    visible_ms: test.visible_ms,
                });
                test.index += 1;
                test.step_started = None;
                if test.index == test.targets.len() {
                    print_self_test_summary(start, &test.results);
                    return true;
                }
            }
        }
    }
    false
}

fn print_self_test_summary(start: Instant, results: &[StepResult]) {
    let format_ms =
        |value: Option<f64>| value.map_or("없음".to_string(), |ms| format!("{ms:.1} ms"));
    log(start, "[self-test] 결과 (알림 도착 / TIS 조회에 반영까지)");
    for result in results {
        log(
            start,
            format!(
                "[self-test]   {:<45} notify={:<10} visible={}",
                result.target,
                format_ms(result.notify_ms),
                format_ms(result.visible_ms)
            ),
        );
    }
}

extern "C" fn on_timer(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    let should_stop = with_state(|state| {
        if state.self_test.is_some() {
            return self_test_tick(state);
        }
        poll_mode(state);
        poll_focus_app(state);
        poll_caret(state);
        state
            .options
            .duration
            .is_some_and(|limit| state.start.elapsed() >= limit)
    })
    .unwrap_or(true);
    if should_stop {
        CFRunLoop::get_current().stop();
    }
}

fn build_self_test(start: Instant) -> Option<SelfTest> {
    let sources = ime::enabled_input_sources();
    log(start, "[self-test] 켜 둔 입력 소스:");
    for source in &sources {
        log(
            start,
            format!("[self-test]   {}", describe(&Some(source.clone()))),
        );
    }
    let original = ime::current_input_source()?.id;
    let korean = sources
        .iter()
        .find(|s| s.id.starts_with("com.apple.inputmethod.Korean."))?;
    let latin = sources
        .iter()
        .find(|s| s.id.starts_with("com.apple.keylayout."))?;
    let mut targets = vec![
        korean.id.clone(),
        latin.id.clone(),
        korean.id.clone(),
        latin.id.clone(),
    ];
    if let Some(other) = sources
        .iter()
        .find(|s| s.id.starts_with("com.apple.inputmethod.") && !s.id.contains("Korean"))
    {
        targets.push(other.id.clone());
    }
    // 마지막 단계는 원래 입력 소스로 복구
    targets.push(original);
    Some(SelfTest {
        targets,
        index: 0,
        step_started: None,
        notify_ms: None,
        visible_ms: None,
        results: Vec::new(),
    })
}

pub fn run(options: Options) {
    let start = Instant::now();
    let trusted = if options.prompt {
        caret::prompt_for_trust()
    } else {
        caret::is_trusted()
    };
    log(
        start,
        format!(
            "[init] 손쉬운 사용 권한: {}",
            if trusted {
                "있음"
            } else {
                "없음(캐럿 대신 마우스 위치 사용)"
            }
        ),
    );
    let current = ime::current_input_source();
    log(start, format!("[init] {}", describe(&current)));

    let self_test = if options.self_test {
        let test = build_self_test(start);
        if test.is_none() {
            log(
                start,
                "[self-test] 한국어 입력 소스와 ABC 같은 키보드 레이아웃이 모두 켜져 있어야 합니다.",
            );
            return;
        }
        test
    } else {
        None
    };
    let interval = if self_test.is_some() {
        SELF_TEST_TICK
    } else {
        options.interval
    };

    STATE.with(|cell| {
        *cell.borrow_mut() = Some(State {
            start,
            options,
            last_source: current,
            last_caret: None,
            last_pid: None,
            electron_fixed: HashSet::new(),
            self_test,
        });
    });

    ime::subscribe_input_source_changed(on_input_source_changed);

    let interval_seconds = interval.as_secs_f64();
    let timer = CFRunLoopTimer::new(
        CFDate::now().abs_time() + interval_seconds,
        interval_seconds,
        0,
        0,
        on_timer,
        std::ptr::null_mut(),
    );
    CFRunLoop::get_current().add_timer(&timer, unsafe { kCFRunLoopDefaultMode });
    log(
        start,
        "[init] 감시 시작. 한/영을 바꾸거나 다른 입력란을 클릭해 보세요. 종료: Ctrl+C",
    );
    CFRunLoop::run_current();
    log(start, "[exit] 종료");
}
