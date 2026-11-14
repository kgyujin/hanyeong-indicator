//! 작성자: Git 이력 참조
//! 작성목적: macOS 손쉬운 사용(AX) API로 포커스된 텍스트 입력란의 캐럿 위치를 읽는다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | AX 캐럿 조회, 권한 확인, 마우스 위치, Electron 접근성 활성화 추가
//! - 2026-11-14 | 4단계 진단 개선 | 좌표 실패 시 AX 오류 코드가 -1로 덮이던 문제 수정, 빈 좌표(EmptyBounds) 구분
//!
//! 권한: 캐럿 조회는 "손쉬운 사용" 권한이 필요하다. 마우스 위치(CGEvent)는 권한이 필요 없다.
//! 좌표: AX와 CGEvent 모두 주 모니터 좌상단 원점(core::state 좌표 규칙과 같음).
//! 응답 지연: 응답하지 않는 앱 때문에 멈추지 않도록 AX 메시지 타임아웃을 짧게 건다.

use std::ffi::c_void;

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;

use super::ffi::{self, CFRange, CGRect};
use crate::core::state::ScreenRect;

/// AX 요청 하나의 최대 대기 시간(초). 폴링 주기(100ms)보다 짧게 둔다.
const AX_MESSAGING_TIMEOUT_SECONDS: f32 = 0.05;

/// 캐럿 위치를 얻지 못한 이유(진단 출력용)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretError {
    /// 손쉬운 사용 권한 없음
    NotTrusted,
    /// 포커스된 UI 요소 없음 (AXError 코드)
    NoFocusedElement(i32),
    /// 텍스트 입력란이 아니거나 선택 범위를 제공하지 않음
    NoSelectedRange(i32),
    /// 범위의 화면 좌표 요청이 실패함 (마지막 AXError 코드, AXValue 형식 불일치는 -1)
    NoBounds(i32),
    /// 좌표 요청은 성공했지만 크기 0 또는 원점(0,0)인 쓸 수 없는 사각형을 돌려줌
    EmptyBounds,
}

/// AXValue에서 기대한 형식의 값을 꺼내지 못했을 때 쓰는 오류 코드(AX 오류 코드와 겹치지 않음)
const VALUE_TYPE_MISMATCH: i32 = -1;

/// 이 프로세스가 손쉬운 사용 권한을 받았는지 확인한다(안내창을 띄우지 않음).
pub fn is_trusted() -> bool {
    unsafe { ffi::AXIsProcessTrusted() != 0 }
}

/// 권한이 없으면 시스템 설정 안내창을 띄운다. 현재 권한 상태를 돌려준다.
pub fn prompt_for_trust() -> bool {
    let key = unsafe { CFString::wrap_under_get_rule(ffi::kAXTrustedCheckOptionPrompt) };
    let options =
        CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    unsafe { ffi::AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) != 0 }
}

/// 마우스 커서 위치. 권한 없이 동작한다.
pub fn mouse_location() -> (f64, f64) {
    // SAFETY: CGEventCreate(NULL)은 현재 상태 이벤트를 만든다. Create rule이라 CFType이 해제한다.
    let raw = unsafe { ffi::CGEventCreate(std::ptr::null()) };
    if raw.is_null() {
        return (0.0, 0.0);
    }
    let event = unsafe { CFType::wrap_under_create_rule(raw) };
    let point = unsafe { ffi::CGEventGetLocation(event.as_CFTypeRef()) };
    (point.x, point.y)
}

fn system_wide() -> CFType {
    // SAFETY: Create rule
    let element = unsafe { CFType::wrap_under_create_rule(ffi::AXUIElementCreateSystemWide()) };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(element.as_CFTypeRef(), AX_MESSAGING_TIMEOUT_SECONDS)
    };
    element
}

fn copy_attribute(element: &CFType, name: &'static str) -> Result<CFType, i32> {
    let attribute = CFString::from_static_string(name);
    let mut value: CFTypeRef = std::ptr::null();
    // SAFETY: Copy 함수. 성공 시 value를 소유한다.
    let error = unsafe {
        ffi::AXUIElementCopyAttributeValue(
            element.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            &mut value,
        )
    };
    if error != ffi::kAXErrorSuccess || value.is_null() {
        return Err(error);
    }
    Ok(unsafe { CFType::wrap_under_create_rule(value) })
}

fn bounds_for_range(element: &CFType, range: CFRange) -> Result<CGRect, i32> {
    // SAFETY: AXValueCreate는 Create rule
    let range_value = unsafe {
        CFType::wrap_under_create_rule(ffi::AXValueCreate(
            ffi::kAXValueCFRangeType,
            &range as *const CFRange as *const c_void,
        ))
    };
    let attribute = CFString::from_static_string("AXBoundsForRange");
    let mut value: CFTypeRef = std::ptr::null();
    let error = unsafe {
        ffi::AXUIElementCopyParameterizedAttributeValue(
            element.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            range_value.as_CFTypeRef(),
            &mut value,
        )
    };
    if error != ffi::kAXErrorSuccess || value.is_null() {
        return Err(error);
    }
    let value = unsafe { CFType::wrap_under_create_rule(value) };
    let mut rect = CGRect::default();
    let ok = unsafe {
        ffi::AXValueGetValue(
            value.as_CFTypeRef(),
            ffi::kAXValueCGRectType,
            &mut rect as *mut CGRect as *mut c_void,
        )
    };
    if ok == 0 {
        Err(VALUE_TYPE_MISMATCH)
    } else {
        Ok(rect)
    }
}

/// 일부 앱은 길이 0 범위에 (0,0,0,0)을 돌려준다. 높이가 없거나 원점이면 쓸 수 없는 값으로 본다.
fn is_usable(rect: &CGRect) -> bool {
    rect.size.height > 0.0 && !(rect.origin.x == 0.0 && rect.origin.y == 0.0)
}

fn caret_from_rect(x: f64, rect: &CGRect) -> ScreenRect {
    ScreenRect {
        x,
        y: rect.origin.y,
        width: 1.0,
        height: rect.size.height,
    }
}

/// 포커스된 텍스트 입력란의 캐럿 사각형을 읽는다.
///
/// 시도 순서:
/// 1. 캐럿 위치의 길이 0 범위
/// 2. 캐럿 바로 앞 글자(길이 1)의 오른쪽 끝
/// 3. 캐럿 바로 뒤 글자(길이 1)의 왼쪽 끝
pub fn focused_caret_rect() -> Result<ScreenRect, CaretError> {
    if !is_trusted() {
        return Err(CaretError::NotTrusted);
    }
    let system = system_wide();
    let focused =
        copy_attribute(&system, "AXFocusedUIElement").map_err(CaretError::NoFocusedElement)?;
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(focused.as_CFTypeRef(), AX_MESSAGING_TIMEOUT_SECONDS)
    };

    let range_value =
        copy_attribute(&focused, "AXSelectedTextRange").map_err(CaretError::NoSelectedRange)?;
    let mut selected = CFRange::default();
    let ok = unsafe {
        ffi::AXValueGetValue(
            range_value.as_CFTypeRef(),
            ffi::kAXValueCFRangeType,
            &mut selected as *mut CFRange as *mut c_void,
        )
    };
    if ok == 0 {
        return Err(CaretError::NoSelectedRange(VALUE_TYPE_MISMATCH));
    }

    let caret_index = selected.location;
    // (요청할 범위, 캐럿을 사각형 오른쪽 끝에서 읽는지)
    let mut attempts = vec![(
        CFRange {
            location: caret_index,
            length: 0,
        },
        false,
    )];
    if caret_index > 0 {
        attempts.push((
            CFRange {
                location: caret_index - 1,
                length: 1,
            },
            true,
        ));
    }
    attempts.push((
        CFRange {
            location: caret_index,
            length: 1,
        },
        false,
    ));

    // 빈 사각형을 한 번이라도 받았으면 그것을 실패 이유로 보고한다(오류 코드보다 원인 파악에 유용).
    let mut failure = CaretError::NoBounds(VALUE_TYPE_MISMATCH);
    for (range, right_edge) in attempts {
        match bounds_for_range(&focused, range) {
            Ok(rect) if is_usable(&rect) => {
                let x = if right_edge {
                    rect.origin.x + rect.size.width
                } else {
                    rect.origin.x
                };
                return Ok(caret_from_rect(x, &rect));
            }
            Ok(_) => failure = CaretError::EmptyBounds,
            Err(error) if failure != CaretError::EmptyBounds => {
                failure = CaretError::NoBounds(error);
            }
            Err(_) => {}
        }
    }
    Err(failure)
}

/// 포커스된 앱의 프로세스 ID
pub fn focused_app_pid() -> Option<i32> {
    let system = system_wide();
    let app = copy_attribute(&system, "AXFocusedApplication").ok()?;
    let mut pid = 0;
    let error = unsafe { ffi::AXUIElementGetPid(app.as_CFTypeRef(), &mut pid) };
    (error == ffi::kAXErrorSuccess).then_some(pid)
}

/// Electron 앱은 보조 기술이 요청해야 접근성 트리를 만든다.
/// `AXManualAccessibility = true`를 설정해 캐럿 위치를 노출하게 한다. AXError 코드를 돌려준다.
pub fn enable_manual_accessibility(pid: i32) -> i32 {
    // SAFETY: Create rule
    let app = unsafe { CFType::wrap_under_create_rule(ffi::AXUIElementCreateApplication(pid)) };
    let attribute = CFString::from_static_string("AXManualAccessibility");
    unsafe {
        ffi::AXUIElementSetAttributeValue(
            app.as_CFTypeRef(),
            attribute.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        )
    }
}
