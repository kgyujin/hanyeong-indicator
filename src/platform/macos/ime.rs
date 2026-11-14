//! 작성자: Git 이력 참조
//! 작성목적: macOS 현재 입력 소스 조회, 입력 소스 변경 알림 구독, (검증용) 입력 소스 선택
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | TIS 기반 조회·알림·선택 추가
//!
//! 권한: 필요 없음(손쉬운 사용 권한과 무관).
//! 스레드: 모든 함수는 메인 스레드에서 호출해야 한다(platform::macos 모듈 설명 참고).

use std::ffi::c_void;

use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::{CFString, CFStringRef};

use super::ffi;
use crate::core::classify::classify_macos_source;
use crate::core::state::InputMode;

/// 입력 소스의 식별 정보
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSourceInfo {
    /// 예: `com.apple.inputmethod.Korean.2SetKorean`
    pub id: String,
    /// 예: `ko`
    pub primary_language: Option<String>,
    /// 사용자에게 보이는 이름. 예: `2벌식`
    pub localized_name: Option<String>,
}

impl InputSourceInfo {
    pub fn mode(&self) -> InputMode {
        classify_macos_source(&self.id, self.primary_language.as_deref())
    }
}

/// 소유한 TISInputSourceRef. drop 시 CFRelease 한다.
struct OwnedSource(CFType);

impl OwnedSource {
    fn as_ptr(&self) -> ffi::TISInputSourceRef {
        self.0.as_CFTypeRef() as ffi::TISInputSourceRef
    }
}

fn string_property(source: ffi::TISInputSourceRef, key: CFStringRef) -> Option<String> {
    // SAFETY: source는 유효한 TISInputSourceRef이고, 반환값은 Get rule(빌린 참조)이다.
    let raw = unsafe { ffi::TISGetInputSourceProperty(source, key) };
    if raw.is_null() {
        return None;
    }
    let value = unsafe { CFString::wrap_under_get_rule(raw as CFStringRef) };
    Some(value.to_string())
}

fn first_language(source: ffi::TISInputSourceRef) -> Option<String> {
    // SAFETY: 위와 같음. 값은 CFString 배열이다.
    let raw =
        unsafe { ffi::TISGetInputSourceProperty(source, ffi::kTISPropertyInputSourceLanguages) };
    if raw.is_null() {
        return None;
    }
    let languages: CFArray<CFString> = unsafe { CFArray::wrap_under_get_rule(raw as CFArrayRef) };
    languages.get(0).map(|lang| lang.to_string())
}

fn read_info(source: ffi::TISInputSourceRef) -> Option<InputSourceInfo> {
    let id = string_property(source, unsafe { ffi::kTISPropertyInputSourceID })?;
    Some(InputSourceInfo {
        id,
        primary_language: first_language(source),
        localized_name: string_property(source, unsafe { ffi::kTISPropertyLocalizedName }),
    })
}

/// 현재 선택된 키보드 입력 소스를 읽는다.
pub fn current_input_source() -> Option<InputSourceInfo> {
    // SAFETY: Copy 함수이므로 반환값을 소유하고 OwnedSource가 해제한다.
    let raw = unsafe { ffi::TISCopyCurrentKeyboardInputSource() };
    if raw.is_null() {
        return None;
    }
    let owned = OwnedSource(unsafe { CFType::wrap_under_create_rule(raw as _) });
    read_info(owned.as_ptr())
}

/// 현재 입력 모드. 조회 실패 시 Unknown.
pub fn current_mode() -> InputMode {
    current_input_source()
        .map(|info| info.mode())
        .unwrap_or(InputMode::Unknown)
}

/// 사용자가 켜 둔 입력 소스 중 선택 가능한 것만 나열한다.
fn select_capable_sources() -> Vec<(InputSourceInfo, OwnedSource)> {
    let key = unsafe { CFString::wrap_under_get_rule(ffi::kTISPropertyInputSourceIsSelectCapable) };
    let filter =
        CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
    // SAFETY: Create 함수이므로 배열을 소유한다. include_all=0 은 "사용자가 켠 소스만".
    let raw = unsafe { ffi::TISCreateInputSourceList(filter.as_concrete_TypeRef(), 0) };
    if raw.is_null() {
        return Vec::new();
    }
    let list: CFArray<CFType> = unsafe { CFArray::wrap_under_create_rule(raw) };
    list.iter()
        .filter_map(|item| {
            let owned = OwnedSource(item.clone());
            read_info(owned.as_ptr()).map(|info| (info, owned))
        })
        .collect()
}

/// 검증용: 사용자가 켜 둔 선택 가능한 입력 소스 목록
pub fn enabled_input_sources() -> Vec<InputSourceInfo> {
    select_capable_sources()
        .into_iter()
        .map(|(info, _)| info)
        .collect()
}

/// 검증용: ID가 일치하는 입력 소스를 선택한다. 앱 본체에서는 쓰지 않는다.
pub fn select_input_source(id: &str) -> Result<(), String> {
    let (_, source) = select_capable_sources()
        .into_iter()
        .find(|(info, _)| info.id == id)
        .ok_or_else(|| format!("입력 소스를 찾지 못함: {id}"))?;
    // SAFETY: source는 유효한 TISInputSourceRef
    let status = unsafe { ffi::TISSelectInputSource(source.as_ptr()) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("TISSelectInputSource 실패: OSStatus {status}"))
    }
}

/// 알림 구독자 식별용 주소. 분산 알림 센터는 observer로 NULL을 허용하지 않는다.
static OBSERVER_TAG: u8 = 0;

/// 입력 소스 변경 분산 알림을 구독한다. 콜백은 메인 런루프에서 호출된다.
/// 런루프(CFRunLoopRun 또는 NSApplication)가 돌고 있어야 알림이 온다.
pub fn subscribe_input_source_changed(callback: ffi::CFNotificationCallback) {
    // SAFETY: 분산 알림 센터는 프로세스 전역 싱글턴이다. observer는 식별용 포인터로만 쓰인다.
    unsafe {
        ffi::CFNotificationCenterAddObserver(
            ffi::CFNotificationCenterGetDistributedCenter(),
            &OBSERVER_TAG as *const u8 as *const c_void,
            callback,
            ffi::kTISNotifySelectedKeyboardInputSourceChanged,
            std::ptr::null(),
            ffi::CF_NOTIFICATION_DELIVER_IMMEDIATELY,
        );
    }
}
