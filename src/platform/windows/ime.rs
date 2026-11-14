//! 작성자: Git 이력 참조
//! 작성목적: 포커스 창의 키보드 레이아웃과 IME 열림 상태·변환 모드를 읽는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | WM_IME_CONTROL 기반 조회 추가
//!
//! 방식: 포커스 창의 기본 IME 창(ImmGetDefaultIMEWnd)에 WM_IME_CONTROL 메시지를 보내
//! IMC_GETOPENSTATUS, IMC_GETCONVERSIONMODE 값을 받는다. 다른 프로세스의 IME 상태를
//! 훅 없이 읽는 방법이다. Windows 11 새 한국어 IME에서의 동작은 1단계 실측 대상이다.
//! 응답하지 않는 창 때문에 멈추지 않도록 SendMessageTimeoutW에 짧은 제한 시간을 둔다.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::Ime::ImmGetDefaultIMEWnd;
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows::Win32::UI::WindowsAndMessaging::{SMTO_ABORTIFHUNG, SendMessageTimeoutW};

use super::focus::{self, FocusInfo};
use crate::core::classify::classify_windows;
use crate::core::state::InputMode;

const WM_IME_CONTROL: u32 = 0x0283;
const IMC_GETCONVERSIONMODE: usize = 0x0001;
const IMC_GETOPENSTATUS: usize = 0x0005;
/// IME 창 응답 대기 시간(ms). 폴링 주기(120ms)보다 짧아야 한다.
const IME_QUERY_TIMEOUT_MS: u32 = 50;

/// 한 번의 조회 결과(원시 값 포함, 진단용)
#[derive(Debug, Clone)]
pub struct ImeSnapshot {
    pub focus: FocusInfo,
    pub focus_class: String,
    /// HKL 하위 16비트(LANGID). 한국어 0x0412
    pub lang_id: u16,
    pub open_status: Option<bool>,
    pub conversion_mode: Option<u32>,
    pub mode: InputMode,
}

fn query_ime(ime_window: HWND, command: usize) -> Option<usize> {
    if ime_window.is_invalid() {
        return None;
    }
    let mut answer: usize = 0;
    let result = unsafe {
        SendMessageTimeoutW(
            ime_window,
            WM_IME_CONTROL,
            WPARAM(command),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            IME_QUERY_TIMEOUT_MS,
            Some(&mut answer),
        )
    };
    // 반환값 0은 시간 초과 또는 실패(UIPI 차단 포함)
    (result.0 != 0).then_some(answer)
}

/// 현재 포커스 창의 입력 상태를 읽는다. 전경 창이 없으면 None.
pub fn snapshot() -> Option<ImeSnapshot> {
    let focus = focus::foreground_focus()?;
    let layout = unsafe { GetKeyboardLayout(focus.thread_id) };
    let lang_id = (layout.0 as usize & 0xffff) as u16;

    let ime_window = unsafe { ImmGetDefaultIMEWnd(focus.focus) };
    let open_status = query_ime(ime_window, IMC_GETOPENSTATUS).map(|value| value != 0);
    let conversion_mode = query_ime(ime_window, IMC_GETCONVERSIONMODE).map(|value| value as u32);

    Some(ImeSnapshot {
        focus,
        focus_class: focus::class_name(focus.focus),
        lang_id,
        open_status,
        conversion_mode,
        mode: classify_windows(lang_id, open_status, conversion_mode),
    })
}
