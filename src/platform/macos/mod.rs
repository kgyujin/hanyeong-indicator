//! macOS 구현.
//!
//! 주의: TIS(Text Input Source) API는 메인 스레드에서만 호출해야 한다.
//! macOS 14 이후 다른 스레드에서 호출하면 TSM 내부 assertion으로 프로세스가 종료될 수 있다.

pub mod app;
pub mod caret;
mod ffi;
pub mod ime;
mod observer;
pub mod overlay;
