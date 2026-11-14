//! 작성자: Git 이력 참조
//! 작성목적: 한/영 입력 상태 표시 앱의 공통 로직(core)과 OS별 구현(platform)을 묶는 라이브러리 진입점
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 1단계 기술 검증 | 입력 모드 분류, macOS·Windows 입력 모드/캐럿 프로브 추가
//! - 2026-11-14 | 3단계 macOS 오버레이 | 메뉴바·알림 영역 메뉴(tray) 추가

pub mod core;
pub mod platform;
pub mod tray;
