//! 작성자: Git 이력 참조
//! 작성목적: 전경 창과 키보드 포커스 창, Win32 캐럿 정보를 한 번에 읽는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | GetGUIThreadInfo 기반 포커스 조회 추가

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    GUITHREADINFO, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
};

/// 전경 스레드의 포커스 정보
#[derive(Debug, Clone, Copy)]
pub struct FocusInfo {
    pub foreground: HWND,
    /// 키보드 포커스를 가진 창. 정보를 못 얻으면 foreground와 같다.
    pub focus: HWND,
    pub thread_id: u32,
    /// 시스템 캐럿을 가진 창. 없으면 null.
    pub caret_window: HWND,
    /// caret_window 클라이언트 좌표 기준 캐럿 사각형
    pub caret_rect: RECT,
}

/// 전경 창의 포커스 정보를 읽는다. 전경 창이 없으면(잠금 화면 등) None.
pub fn foreground_focus() -> Option<FocusInfo> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, None) };
    let mut info = GUITHREADINFO {
        cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
        ..Default::default()
    };
    let has_info = unsafe { GetGUIThreadInfo(thread_id, &mut info) }.is_ok();
    let focus = if has_info && !info.hwndFocus.is_invalid() {
        info.hwndFocus
    } else {
        foreground
    };
    Some(FocusInfo {
        foreground,
        focus,
        thread_id,
        caret_window: if has_info {
            info.hwndCaret
        } else {
            HWND::default()
        },
        caret_rect: if has_info {
            info.rcCaret
        } else {
            RECT::default()
        },
    })
}

/// 창 클래스 이름(진단 출력용). 예: `Notepad`, `Chrome_WidgetWin_1`
pub fn class_name(window: HWND) -> String {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(window, &mut buffer) };
    String::from_utf16_lossy(&buffer[..length.max(0) as usize])
}
