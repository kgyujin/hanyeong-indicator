//! Windows 프로브 실행부.
//!
//! 단일 스레드에서 주기적으로 폴링한다. UIA 객체는 이 스레드(STA)에서 만들고 이 스레드에서만 쓴다.
//! 입력 모드는 원시 값(열림 상태, 변환 모드, LANGID)도 함께 출력해
//! Windows 11 새 한국어 IME에서 어느 값이 바뀌는지 확인할 수 있게 한다.

use std::time::Instant;

use hanyeong_indicator::core::state::{CaretSource, ScreenRect};
use hanyeong_indicator::platform::windows::caret::{self, UiaCaret};
use hanyeong_indicator::platform::windows::ime::{self, ImeSnapshot};

use crate::{Options, log};

#[derive(Debug, Clone, PartialEq)]
enum CaretReport {
    Caret(CaretSource, ScreenRect),
    Fallback(String),
}

fn describe(snapshot: &ImeSnapshot) -> String {
    let open = snapshot
        .open_status
        .map_or("실패".to_string(), |open| open.to_string());
    let conversion = snapshot
        .conversion_mode
        .map_or("실패".to_string(), |mode| format!("0x{mode:04X}"));
    format!(
        "mode={} lang=0x{:04X} open={open} conversion={conversion}",
        snapshot.mode, snapshot.lang_id
    )
}

fn moved_enough(previous: &ScreenRect, current: &ScreenRect) -> bool {
    (previous.x - current.x).abs() >= 1.0
        || (previous.y - current.y).abs() >= 1.0
        || (previous.height - current.height).abs() >= 1.0
}

pub fn run(options: Options) {
    let start = Instant::now();
    caret::init_dpi_awareness();
    let uia = match UiaCaret::new() {
        Ok(uia) => Some(uia),
        Err(error) => {
            log(start, format!("[init] UI Automation 초기화 실패: {error}"));
            None
        }
    };
    log(
        start,
        "[init] 감시 시작. 한/영 키를 누르거나 다른 입력란을 클릭해 보세요. 종료: Ctrl+C",
    );

    let mut last_mode: Option<String> = None;
    let mut last_app: Option<String> = None;
    let mut last_caret: Option<CaretReport> = None;

    loop {
        if options
            .duration
            .is_some_and(|limit| start.elapsed() >= limit)
        {
            break;
        }

        if let Some(snapshot) = ime::snapshot() {
            if last_app.as_deref() != Some(snapshot.focus_class.as_str()) {
                log(start, format!("[app] focus class={}", snapshot.focus_class));
                last_app = Some(snapshot.focus_class.clone());
            }
            let description = describe(&snapshot);
            if last_mode.as_deref() != Some(description.as_str()) {
                log(start, format!("[poll] {description}"));
                last_mode = Some(description);
            }

            let report = if let Some(rect) = caret::win32_caret(&snapshot.focus) {
                CaretReport::Caret(CaretSource::Win32Caret, rect)
            } else {
                match uia.as_ref().map(|uia| uia.caret_rect()) {
                    Some(Ok(rect)) => CaretReport::Caret(CaretSource::UiAutomation, rect),
                    Some(Err(reason)) => CaretReport::Fallback(reason),
                    None => CaretReport::Fallback("UIA 없음".into()),
                }
            };
            let should_print = match (&last_caret, &report) {
                (Some(CaretReport::Caret(a, previous)), CaretReport::Caret(b, current)) => {
                    a != b || moved_enough(previous, current)
                }
                (previous, current) => previous.as_ref() != Some(current),
            };
            if should_print {
                match &report {
                    CaretReport::Caret(source, rect) => {
                        log(start, format!("[caret] {source:?} {rect}"));
                    }
                    CaretReport::Fallback(reason) => {
                        let (x, y) = caret::mouse_location();
                        log(
                            start,
                            format!("[fallback] {reason} → 마우스 ({x:.0},{y:.0})"),
                        );
                    }
                }
            }
            last_caret = Some(report);
        }

        std::thread::sleep(options.interval);
    }
    log(start, "[exit] 종료");
}
