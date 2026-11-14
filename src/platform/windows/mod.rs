//! Windows 구현.
//!
//! 권한: 관리자 권한이 필요 없다. 단, UIPI(User Interface Privilege Isolation) 때문에
//! 일반 권한 프로세스는 관리자 권한 창에 메시지를 보낼 수 없어 그 창의 입력 모드는 Unknown이 된다.
//! 전역 키보드 훅(SetWindowsHookEx)은 쓰지 않는다.

pub mod caret;
pub mod focus;
pub mod ime;
