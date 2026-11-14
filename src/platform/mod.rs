//! OS별 입력 모드·캐럿 위치 조회 구현.
//! 3단계(오버레이)에서 ImeProbe / CaretProbe / Overlay trait로 정리한다.

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(windows)]
pub mod windows;
