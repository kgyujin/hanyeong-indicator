//! 작성자: Git 이력 참조
//! 작성목적: 앞에 있는 앱의 포커스 변경·텍스트 선택 변경 알림(AXObserver)을 받아 배지를 즉시 갱신한다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 반응 속도 개선 | 앱 단위 AXObserver 연결·해제 추가
//!
//! 왜 필요한가: 100ms 폴링만으로는 입력란에 들어간 뒤 배지가 최대 100ms(+조회 시간) 늦게 뜨고,
//! 타이핑 시작도 늦게 감지한다. 알림을 받으면 그 순간 갱신한다. 폴링은 알림 누락 대비로 남겨 둔다.
//!
//! 범위: 앱 요소(AXUIElementCreateApplication)에 등록하면 그 앱 안 모든 요소의 알림을 받는다.
//! 앞에 있는 앱이 바뀌면 이전 관찰자를 해제하고 새 앱에 다시 연결한다.
//! 권한: 손쉬운 사용 권한이 있어야 등록된다.

use std::ptr;

use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::runloop::{CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes};
use core_foundation::string::CFString;

use super::ffi::{self, AXObserverCallback};

/// 구독하는 알림. 포커스 이동(입력란 진입)과 캐럿·선택 이동(타이핑)을 잡는다.
const NOTIFICATIONS: [&str; 2] = ["AXFocusedUIElementChanged", "AXSelectedTextChanged"];

pub struct AppObserver {
    pid: i32,
    _observer: CFType,
    source: CFRunLoopSource,
}

impl AppObserver {
    /// `pid` 앱의 알림을 메인 런루프에 연결한다. 등록된 알림이 하나도 없으면 마지막 AXError를 돌려준다.
    pub fn attach(pid: i32, callback: AXObserverCallback) -> Result<Self, i32> {
        let mut raw: CFTypeRef = ptr::null();
        let error = unsafe { ffi::AXObserverCreate(pid, callback, &mut raw) };
        if error != ffi::kAXErrorSuccess || raw.is_null() {
            return Err(error);
        }
        // SAFETY: Create 함수이므로 관찰자를 소유한다.
        let observer = unsafe { CFType::wrap_under_create_rule(raw) };
        let application =
            unsafe { CFType::wrap_under_create_rule(ffi::AXUIElementCreateApplication(pid)) };

        let mut registered = 0;
        let mut last_error = ffi::kAXErrorSuccess;
        for name in NOTIFICATIONS {
            let notification = CFString::from_static_string(name);
            let error = unsafe {
                ffi::AXObserverAddNotification(
                    observer.as_CFTypeRef(),
                    application.as_CFTypeRef(),
                    notification.as_concrete_TypeRef(),
                    ptr::null_mut(),
                )
            };
            if error == ffi::kAXErrorSuccess {
                registered += 1;
            } else {
                last_error = error;
            }
        }
        if registered == 0 {
            return Err(last_error);
        }

        // SAFETY: Get rule(빌린 참조)이므로 get rule로 감싸 참조 수를 늘린다.
        let source = unsafe {
            CFRunLoopSource::wrap_under_get_rule(ffi::AXObserverGetRunLoopSource(
                observer.as_CFTypeRef(),
            ))
        };
        CFRunLoop::get_main().add_source(&source, unsafe { kCFRunLoopCommonModes });
        Ok(Self {
            pid,
            _observer: observer,
            source,
        })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }
}

impl Drop for AppObserver {
    fn drop(&mut self) {
        CFRunLoop::get_main().remove_source(&self.source, unsafe { kCFRunLoopCommonModes });
    }
}
