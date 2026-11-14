//! 작성자: Git 이력 참조
//! 작성목적: Carbon(TIS), ApplicationServices(AX), CoreGraphics, CoreFoundation 알림 C API 선언
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | TIS, AX, CGEvent, CFNotificationCenter 선언 추가
//! - 2026-11-14 | 반응 속도 개선 | AXObserver 선언 추가
//!
//! 공개 Rust 바인딩 crate가 오래됐거나 범위가 넓어서, 필요한 함수만 직접 선언한다.
//! 소유권 규칙: 이름에 Create/Copy가 들어간 함수의 반환값은 호출자가 CFRelease 해야 한다(Create rule).
//! TISGetInputSourceProperty 반환값은 빌린 참조라서 해제하지 않는다(Get rule).

#![allow(non_upper_case_globals, non_snake_case, dead_code)]

use std::ffi::c_void;

use core_foundation::array::CFArrayRef;
use core_foundation::base::{CFIndex, CFTypeRef};
use core_foundation::dictionary::CFDictionaryRef;
use core_foundation::runloop::CFRunLoopSourceRef;
use core_foundation::string::CFStringRef;

pub type Boolean = u8;
pub type OSStatus = i32;
pub type TISInputSourceRef = *mut c_void;
pub type AXUIElementRef = CFTypeRef;
pub type AXValueRef = CFTypeRef;
pub type AXError = i32;
pub type AXObserverRef = CFTypeRef;
/// AX 알림 콜백. 메인 런루프에서 호출된다.
pub type AXObserverCallback = extern "C" fn(
    observer: AXObserverRef,
    element: AXUIElementRef,
    notification: CFStringRef,
    refcon: *mut c_void,
);

pub const kAXErrorSuccess: AXError = 0;
/// AXValueType: CGPoint=1, CGSize=2, CGRect=3, CFRange=4
pub const kAXValueCGPointType: u32 = 1;
pub const kAXValueCGSizeType: u32 = 2;
pub const kAXValueCGRectType: u32 = 3;
pub const kAXValueCFRangeType: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    pub fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    pub fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFStringRef) -> *const c_void;
    pub fn TISCreateInputSourceList(
        properties: CFDictionaryRef,
        include_all: Boolean,
    ) -> CFArrayRef;
    pub fn TISSelectInputSource(source: TISInputSourceRef) -> OSStatus;

    pub static kTISPropertyInputSourceID: CFStringRef;
    pub static kTISPropertyInputSourceLanguages: CFStringRef;
    pub static kTISPropertyLocalizedName: CFStringRef;
    pub static kTISPropertyInputSourceIsSelectCapable: CFStringRef;
    /// 분산 알림(distributed notification) 이름: 선택된 입력 소스가 바뀜
    pub static kTISNotifySelectedKeyboardInputSourceChanged: CFStringRef;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;

    pub fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    pub fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
    pub fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    pub fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut Boolean,
    ) -> AXError;
    pub fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_seconds: f32)
    -> AXError;
    pub fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;

    pub fn AXObserverCreate(
        application: i32,
        callback: AXObserverCallback,
        out_observer: *mut AXObserverRef,
    ) -> AXError;
    pub fn AXObserverAddNotification(
        observer: AXObserverRef,
        element: AXUIElementRef,
        notification: CFStringRef,
        refcon: *mut c_void,
    ) -> AXError;
    /// 반환값은 빌린 참조(Get rule)
    pub fn AXObserverGetRunLoopSource(observer: AXObserverRef) -> CFRunLoopSourceRef;

    pub fn AXValueCreate(value_type: u32, value_ptr: *const c_void) -> AXValueRef;
    pub fn AXValueGetValue(value: AXValueRef, value_type: u32, value_ptr: *mut c_void) -> Boolean;

    /// 권한 없이 마우스 위치를 읽는 데 쓴다. 좌표는 주 모니터 좌상단 원점.
    pub fn CGEventCreate(source: *const c_void) -> CFTypeRef;
    pub fn CGEventGetLocation(event: CFTypeRef) -> CGPoint;
}

pub type CFNotificationCallback = extern "C" fn(
    center: *const c_void,
    observer: *const c_void,
    name: CFStringRef,
    object: *const c_void,
    user_info: CFDictionaryRef,
);

/// CFNotificationSuspensionBehaviorDeliverImmediately
pub const CF_NOTIFICATION_DELIVER_IMMEDIATELY: CFIndex = 4;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    /// 같은 객체(CFEqual)면 같은 값. AXUIElement는 같은 UI 요소면 같은 해시를 돌려준다.
    pub fn CFHash(cf: CFTypeRef) -> usize;
    pub fn CFNotificationCenterGetDistributedCenter() -> *const c_void;
    pub fn CFNotificationCenterAddObserver(
        center: *const c_void,
        observer: *const c_void,
        callback: CFNotificationCallback,
        name: CFStringRef,
        object: *const c_void,
        suspension_behavior: CFIndex,
    );
}
