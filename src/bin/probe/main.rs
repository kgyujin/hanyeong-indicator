//! 작성자: Git 이력 참조
//! 작성목적: 1단계 기술 검증용 콘솔 도구. 입력 모드와 캐럿 위치가 바뀔 때마다 출력한다.
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | macOS·Windows 프로브 CLI 추가
//!
//! 옵션과 출력 태그 설명은 usage.txt(`--help` 출력)에 있다.

use std::time::Duration;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone)]
pub struct Options {
    pub interval: Duration,
    pub duration: Option<Duration>,
    pub prompt: bool,
    pub electron_fix: bool,
    pub self_test: bool,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        interval: Duration::from_millis(100),
        duration: None,
        prompt: false,
        electron_fix: false,
        self_test: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--interval-ms" => {
                let value = args.next().ok_or("--interval-ms 값이 없음")?;
                let millis: u64 = value.parse().map_err(|_| format!("잘못된 숫자: {value}"))?;
                options.interval = Duration::from_millis(millis.clamp(5, 5_000));
            }
            "--duration" => {
                let value = args.next().ok_or("--duration 값이 없음")?;
                let seconds: u64 = value.parse().map_err(|_| format!("잘못된 숫자: {value}"))?;
                options.duration = Some(Duration::from_secs(seconds));
            }
            "--prompt" => options.prompt = true,
            "--electron-fix" => options.electron_fix = true,
            "--self-test" => options.self_test = true,
            "-h" | "--help" => {
                println!("{}", include_str!("usage.txt"));
                std::process::exit(0);
            }
            other => return Err(format!("알 수 없는 옵션: {other}")),
        }
    }
    Ok(options)
}

/// 경과 시간을 붙여 한 줄 출력한다.
pub fn log(start: std::time::Instant, message: impl std::fmt::Display) {
    println!(
        "[{:>9.1} ms] {message}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}

fn main() {
    let options = match parse_args() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\n\n{}", include_str!("usage.txt"));
            std::process::exit(2);
        }
    };

    #[cfg(target_os = "macos")]
    macos::run(options);
    #[cfg(windows)]
    windows::run(options);
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = options;
        eprintln!("macOS와 Windows만 지원합니다.");
        std::process::exit(1);
    }
}
