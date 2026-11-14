//! 작성자: Git 이력 참조
//! 작성목적: 입력 모드 변경을 판정하고 전환 강조 시간을 관리한다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | 모드 변경 판정과 강조 기간 추가
//!
//! macOS는 한 번 전환에 같은 변경 알림을 여러 번(1단계 실측: 32ms 안에 3번) 보낸다.
//! 알림 횟수가 아니라 모드 값이 실제로 바뀌었는지로 판정해 강조가 반복되지 않게 한다.

use std::time::{Duration, Instant};

use super::state::InputMode;

#[derive(Debug, Default)]
pub struct ModeTracker {
    current: Option<InputMode>,
    emphasize_until: Option<Instant>,
}

impl ModeTracker {
    /// 새 관측값을 반영한다. 이전 값과 달라졌으면 true.
    /// 첫 관측(앱 시작 직후)은 변경으로 보지 않아 시작할 때 강조하지 않는다.
    pub fn observe(&mut self, mode: InputMode, now: Instant, emphasis: Duration) -> bool {
        let changed = self
            .current
            .as_ref()
            .is_some_and(|current| *current != mode);
        if changed && !emphasis.is_zero() {
            self.emphasize_until = Some(now + emphasis);
        }
        self.current = Some(mode);
        changed
    }

    pub fn current(&self) -> Option<&InputMode> {
        self.current.as_ref()
    }

    pub fn is_emphasized(&self, now: Instant) -> bool {
        self.emphasize_until.is_some_and(|until| now < until)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPHASIS: Duration = Duration::from_millis(800);

    #[test]
    fn first_observation_is_not_a_change() {
        let mut tracker = ModeTracker::default();
        let now = Instant::now();
        assert!(!tracker.observe(InputMode::Latin, now, EMPHASIS));
        assert!(!tracker.is_emphasized(now));
        assert_eq!(tracker.current(), Some(&InputMode::Latin));
    }

    #[test]
    fn change_starts_emphasis_that_expires() {
        let mut tracker = ModeTracker::default();
        let start = Instant::now();
        tracker.observe(InputMode::Latin, start, EMPHASIS);
        assert!(tracker.observe(InputMode::Hangul, start, EMPHASIS));
        assert!(tracker.is_emphasized(start + Duration::from_millis(799)));
        assert!(!tracker.is_emphasized(start + EMPHASIS));
    }

    #[test]
    fn duplicate_notifications_do_not_restart_emphasis() {
        let mut tracker = ModeTracker::default();
        let start = Instant::now();
        tracker.observe(InputMode::Hangul, start, EMPHASIS);
        tracker.observe(InputMode::Latin, start, EMPHASIS);
        // 같은 값이 30ms 뒤에 다시 와도 변경이 아니고, 강조 종료 시각도 그대로다.
        let later = start + Duration::from_millis(30);
        assert!(!tracker.observe(InputMode::Latin, later, EMPHASIS));
        assert!(!tracker.is_emphasized(start + EMPHASIS));
    }

    #[test]
    fn zero_emphasis_disables_highlight() {
        let mut tracker = ModeTracker::default();
        let now = Instant::now();
        tracker.observe(InputMode::Latin, now, Duration::ZERO);
        assert!(tracker.observe(InputMode::Hangul, now, Duration::ZERO));
        assert!(!tracker.is_emphasized(now));
    }
}
