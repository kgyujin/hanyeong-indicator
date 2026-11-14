//! 한영 표시기 앱 진입점. 실제 구현은 OS별 `platform::*::app`에 있다.

// Windows release 빌드에서 콘솔 창이 뜨지 않게 한다(디버그 빌드는 진단 출력을 보려고 콘솔 유지).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
fn main() {
    hanyeong_indicator::platform::macos::app::run();
}

#[cfg(windows)]
fn main() {
    hanyeong_indicator::platform::windows::app::run();
}

#[cfg(not(any(target_os = "macos", windows)))]
fn main() {
    eprintln!("macOS와 Windows만 지원합니다.");
    std::process::exit(1);
}
