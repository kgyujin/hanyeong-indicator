//! 작성자: Git 이력 참조
//! 작성목적: 로그인할 때 자동 실행 등록·해제
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 7단계 자동 실행 | macOS LaunchAgent, Windows 현재 사용자 Run 레지스트리 등록
//!
//! 방식:
//! - macOS: `~/Library/LaunchAgents/dev.hanyeong.indicator.plist`를 만든다. 관리자 권한 불필요.
//!   SMAppService(macOS 13+)는 서명된 앱 번들이 있어야 해서 지금은 쓰지 않는다.
//! - Windows: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`에 등록한다. 관리자 권한 불필요.
//!
//! 등록 경로는 지금 실행 중인 실행 파일 경로다. 개발 중 `cargo run`으로 켜면 target 폴더 경로가 등록되므로,
//! 앱 번들(설치본)로 실행한 상태에서 켜야 한다. 시작할 때 설정이 켜져 있으면 경로를 다시 등록해 최신으로 맞춘다.

use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode, WindowsEnableMode};

/// LaunchAgent 파일 이름과 Windows Run 값 이름
const LAUNCHER_NAME: &str = "dev.hanyeong.indicator";

fn launcher() -> Result<AutoLaunch, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let path = executable
        .to_str()
        .ok_or("실행 파일 경로를 문자열로 바꿀 수 없음")?;
    AutoLaunchBuilder::new()
        .set_app_name(LAUNCHER_NAME)
        .set_app_path(path)
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
        .build()
        .map_err(|error| error.to_string())
}

/// 자동 실행이 등록돼 있는지. 확인할 수 없으면 false.
pub fn is_enabled() -> bool {
    launcher()
        .and_then(|launcher| launcher.is_enabled().map_err(|error| error.to_string()))
        .unwrap_or(false)
}

/// 자동 실행을 원하는 상태로 맞춘다. 켤 때는 이미 등록돼 있어도 현재 경로로 다시 쓴다.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let launcher = launcher()?;
    if enabled {
        launcher.enable().map_err(|error| error.to_string())
    } else if launcher.is_enabled().unwrap_or(false) {
        launcher.disable().map_err(|error| error.to_string())
    } else {
        Ok(())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// 실제 홈 폴더의 LaunchAgents를 건드리지 않도록 HOME을 임시 폴더로 바꿔 검증한다.
    /// HOME을 읽는 테스트는 이것 하나뿐이라 병렬 실행과 충돌하지 않는다.
    #[test]
    fn enable_and_disable_write_and_remove_launch_agent() {
        let home = std::env::temp_dir().join(format!("hanyeong-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        // 실제 홈처럼 ~/Library는 있어야 한다(crate는 LaunchAgents 폴더만 만든다).
        std::fs::create_dir_all(home.join("Library")).unwrap();
        // SAFETY: 이 테스트 외에는 HOME 환경 변수를 읽지 않는다.
        unsafe { std::env::set_var("HOME", &home) };

        let plist = home
            .join("Library/LaunchAgents")
            .join(format!("{LAUNCHER_NAME}.plist"));
        assert!(!is_enabled());

        set_enabled(true).unwrap();
        assert!(is_enabled());
        let content = std::fs::read_to_string(&plist).unwrap();
        let executable = std::env::current_exe().unwrap();
        assert!(content.contains(executable.to_str().unwrap()));
        assert!(content.contains(LAUNCHER_NAME));

        set_enabled(false).unwrap();
        assert!(!is_enabled());
        assert!(!plist.exists());
        // 이미 꺼진 상태에서 다시 꺼도 오류가 아니다.
        set_enabled(false).unwrap();

        std::fs::remove_dir_all(&home).unwrap();
    }
}
