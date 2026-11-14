//! 한영 표시기 앱 진입점. 실제 구현은 OS별 `platform::*::app`에 있다.

#[cfg(target_os = "macos")]
fn main() {
    hanyeong_indicator::platform::macos::app::run();
}

// Windows 앱 본체는 5단계에서 구현한다. 그때까지는 진단 도구(hanyeong-probe)만 동작한다.
#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!(
        "이 플랫폼의 앱 본체는 아직 구현되지 않았습니다. hanyeong-probe로 진단할 수 있습니다."
    );
    std::process::exit(1);
}
