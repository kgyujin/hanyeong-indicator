//! 작성자: Git 이력 참조
//! 작성목적: Windows 캐럿 위치 조회 (Win32 시스템 캐럿 → UI Automation → 마우스 순서)
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | GetGUIThreadInfo, UIA TextPattern2 캐럿 조회 추가
//! - 2026-11-14 | 입력 시작 시 표시 | UIA 입력란 판정(focused_input), 입력란 식별값·영역 조회 추가
//!
//! 좌표: 프로세스를 Per-Monitor DPI v2로 설정한 뒤의 물리 화면 픽셀.
//! Win32 시스템 캐럿은 메모장 같은 전통 Win32 앱에서만 나온다. Chrome·Edge·Office는 UIA를 쓴다.
//! UIA 객체는 COM 아파트 스레드(STA)에서 만들고 같은 스레드에서만 쓴다.

use std::ffi::c_void;

use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, SAFEARRAY,
};
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation8, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern2,
    IUIAutomationTextRange, IUIAutomationValuePattern, TextPatternRangeEndpoint_Start,
    TextUnit_Character, UIA_DocumentControlTypeId, UIA_EditControlTypeId, UIA_TextPattern2Id,
    UIA_ValuePatternId,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
use windows::core::BOOL;

use super::focus::FocusInfo;
use crate::core::state::ScreenRect;

/// 프로세스를 Per-Monitor DPI v2로 설정한다. 창을 만들기 전에 한 번 호출한다.
pub fn init_dpi_awareness() {
    // 이미 매니페스트로 설정됐으면 실패하므로 결과를 무시한다.
    let _ = unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
}

/// 마우스 커서 위치(물리 픽셀)
pub fn mouse_location() -> (f64, f64) {
    let mut point = POINT::default();
    let _ = unsafe { GetCursorPos(&mut point) };
    (point.x as f64, point.y as f64)
}

/// Win32 시스템 캐럿 위치. 캐럿을 쓰지 않는 앱이면 None.
pub fn win32_caret(focus: &FocusInfo) -> Option<ScreenRect> {
    if focus.caret_window.is_invalid() {
        return None;
    }
    let rect = focus.caret_rect;
    let mut top_left = POINT {
        x: rect.left,
        y: rect.top,
    };
    if !unsafe { ClientToScreen(focus.caret_window, &mut top_left) }.as_bool() {
        return None;
    }
    let height = (rect.bottom - rect.top) as f64;
    if height <= 0.0 {
        return None;
    }
    Some(ScreenRect {
        x: top_left.x as f64,
        y: top_left.y as f64,
        width: ((rect.right - rect.left) as f64).max(1.0),
        height,
    })
}

/// UIA로 찾은 포커스된 입력란
#[derive(Debug, Clone)]
pub struct UiaInput {
    /// 입력란 식별값(UIA RuntimeId 해시). 같은 요소면 같은 값
    pub key: u64,
    /// 입력란 전체 화면 영역(캐럿 좌표가 없을 때 기준점)
    pub field_rect: Option<ScreenRect>,
    pub caret: Result<ScreenRect, String>,
}

/// 입력란 판정.
/// - Edit: 값 패턴이 읽기 전용이라고 밝히지 않는 한 입력란
/// - Document: 값 패턴이 있고 쓰기 가능할 때만 입력란(웹 페이지 본문 같은 읽기 전용 문서 제외)
fn is_text_input(element: &IUIAutomationElement) -> bool {
    unsafe {
        let Ok(control_type) = element.CurrentControlType() else {
            return false;
        };
        let read_only = element
            .GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId)
            .and_then(|pattern| pattern.CurrentIsReadOnly())
            .map(|value| value.as_bool())
            .ok();
        if control_type == UIA_EditControlTypeId {
            read_only != Some(true)
        } else if control_type == UIA_DocumentControlTypeId {
            read_only == Some(false)
        } else {
            false
        }
    }
}

fn runtime_id_key(element: &IUIAutomationElement) -> u64 {
    use std::hash::{Hash, Hasher};
    let ids: Vec<i32> = unsafe {
        element
            .GetRuntimeId()
            .ok()
            .and_then(|array| take_safearray::<i32>(array))
            .unwrap_or_default()
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    ids.hash(&mut hasher);
    hasher.finish()
}

/// UI Automation 캐럿 조회기. 생성한 스레드에서만 사용한다.
pub struct UiaCaret {
    automation: IUIAutomation,
}

impl UiaCaret {
    /// 현재 스레드를 STA로 초기화하고 UIA 객체를 만든다.
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            // 이미 초기화된 스레드면 S_FALSE를 돌려주므로 결과를 확인하지 않는다.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
            Ok(Self { automation })
        }
    }

    fn focused_element(&self) -> Result<IUIAutomationElement, String> {
        unsafe { self.automation.GetFocusedElement() }
            .map_err(|error| format!("GetFocusedElement: {error}"))
    }

    /// 포커스된 요소가 입력란이면 Some, 입력란이 아니면 None. UIA 호출이 실패하면 Err.
    pub fn focused_input(&self) -> Result<Option<UiaInput>, String> {
        let element = self.focused_element()?;
        if !is_text_input(&element) {
            return Ok(None);
        }
        let field_rect = unsafe { element.CurrentBoundingRectangle() }
            .ok()
            .filter(|rect| rect.right > rect.left && rect.bottom > rect.top)
            .map(|rect| ScreenRect {
                x: f64::from(rect.left),
                y: f64::from(rect.top),
                width: f64::from(rect.right - rect.left),
                height: f64::from(rect.bottom - rect.top),
            });
        Ok(Some(UiaInput {
            key: runtime_id_key(&element),
            field_rect,
            caret: caret_rect_of(&element),
        }))
    }

    /// 포커스된 요소의 캐럿 사각형. 실패 이유를 문자열로 돌려준다(진단용).
    pub fn caret_rect(&self) -> Result<ScreenRect, String> {
        caret_rect_of(&self.focused_element()?)
    }
}

/// 요소의 캐럿 사각형. TextPattern2가 없거나 좌표가 없으면 이유를 돌려준다.
fn caret_rect_of(element: &IUIAutomationElement) -> Result<ScreenRect, String> {
    unsafe {
        let pattern: IUIAutomationTextPattern2 = element
            .GetCurrentPatternAs(UIA_TextPattern2Id)
            .map_err(|error| format!("TextPattern2 없음: {error}"))?;
        let mut is_active = BOOL(0);
        let caret = pattern
            .GetCaretRange(&mut is_active)
            .map_err(|error| format!("GetCaretRange: {error}"))?;

        // 1) 길이 0 범위 그대로 (Chrome은 폭 0 사각형을 준다)
        if let Some(rect) = first_rect(&caret)? {
            return Ok(ScreenRect {
                width: rect.width.max(1.0),
                ..rect
            });
        }
        // 2) 캐럿 뒤 한 글자로 확장 → 왼쪽 끝
        let next = caret.Clone().map_err(|error| format!("Clone: {error}"))?;
        if next.ExpandToEnclosingUnit(TextUnit_Character).is_ok()
            && let Some(rect) = first_rect(&next)?
        {
            return Ok(ScreenRect { width: 1.0, ..rect });
        }
        // 3) 캐럿 앞 한 글자 → 오른쪽 끝 (문서 끝에 캐럿이 있을 때)
        let previous = caret.Clone().map_err(|error| format!("Clone: {error}"))?;
        if previous
            .MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1)
            .is_ok_and(|moved| moved != 0)
            && let Some(rect) = first_rect(&previous)?
        {
            return Ok(ScreenRect {
                x: rect.x + rect.width,
                width: 1.0,
                ..rect
            });
        }
        Err("캐럿 범위의 화면 좌표 없음".into())
    }
}

/// 텍스트 범위의 첫 번째 화면 사각형. 배열은 [left, top, width, height, ...] double 값이다.
unsafe fn first_rect(range: &IUIAutomationTextRange) -> Result<Option<ScreenRect>, String> {
    let array = unsafe { range.GetBoundingRectangles() }
        .map_err(|error| format!("GetBoundingRectangles: {error}"))?;
    let Some(values) = (unsafe { take_safearray::<f64>(array) }) else {
        return Ok(None);
    };
    let [x, y, width, height] = values
        .get(..4)
        .and_then(|v| v.try_into().ok())
        .unwrap_or([0.0; 4]);
    Ok((height > 0.0).then_some(ScreenRect {
        x,
        y,
        width,
        height,
    }))
}

/// 1차원 SAFEARRAY의 원소를 복사한다. `T`는 배열 원소 형식과 같아야 한다(호출자 책임).
unsafe fn read_safearray<T: Copy>(array: *mut SAFEARRAY) -> Option<Vec<T>> {
    unsafe {
        let lower = SafeArrayGetLBound(array, 1).ok()?;
        let upper = SafeArrayGetUBound(array, 1).ok()?;
        let count = (upper - lower + 1).max(0) as usize;
        let mut data: *mut c_void = std::ptr::null_mut();
        SafeArrayAccessData(array, &mut data).ok()?;
        let values = std::slice::from_raw_parts(data as *const T, count).to_vec();
        let _ = SafeArrayUnaccessData(array);
        Some(values)
    }
}

/// SAFEARRAY를 읽고 해제한다.
unsafe fn take_safearray<T: Copy>(array: *mut SAFEARRAY) -> Option<Vec<T>> {
    if array.is_null() {
        return None;
    }
    unsafe {
        let values = read_safearray(array);
        let _ = SafeArrayDestroy(array);
        values
    }
}
