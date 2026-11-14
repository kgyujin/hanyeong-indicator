//! 작성자: Git 이력 참조
//! 작성목적: "입력을 시작할 때만 표시" 방식에서 배지를 보일지 정하는 상태 기계
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 입력 시작 시 표시 | 입력란 진입·한영 전환 시 표시, 타이핑 시작·2초 경과 시 숨김
//!
//! 규칙(사용자 결정, 2026-11-14):
//! - 띄움: 입력란에 들어간 순간, 입력란 안에서 입력 모드가 바뀐 순간
//! - 숨김: 타이핑을 시작함(캐럿이 일정 거리 이상 움직임) 또는 띄운 지 2초가 지남
//! - 입력란이 아니면 띄우지 않음
//! - 입력란인지 알 수 없으면(macOS 손쉬운 사용 권한 없음 등) 입력 모드가 바뀐 순간에만 2초 띄움
//!
//! 키 입력을 직접 보지 않으므로(전역 키보드 훅 없음) 타이핑 시작은 캐럿 이동으로 추정한다.
//! 화살표 키나 클릭으로 캐럿을 옮겨도 타이핑으로 본다.

use std::time::{Duration, Instant};

/// 띄운 뒤 자동으로 숨기기까지의 시간
pub const SHOW_DURATION: Duration = Duration::from_secs(2);
/// 캐럿이 이만큼(글자 수) 움직이면 타이핑을 시작한 것으로 본다.
/// 한글 조합 중에는 첫 글자가 확정되기 전까지 위치가 안 바뀔 수 있어 2글자로 둔다.
pub const TYPING_CHARACTER_THRESHOLD: i64 = 2;
/// 글자 위치를 모르고 화면 좌표만 알 때의 기준 거리(px). 보통 글자 1~2개 폭.
pub const TYPING_DISTANCE_THRESHOLD: f64 = 12.0;

/// 캐럿 위치 표지. 타이핑 시작 판정에만 쓴다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaretMark {
    /// 텍스트 안 글자 위치(macOS AXSelectedTextRange)
    Index(i64),
    /// 화면 좌표(Windows Win32 캐럿·UIA)
    Point(f64, f64),
}

impl CaretMark {
    /// `start`에서 타이핑을 시작했다고 볼 만큼 멀어졌는지
    fn moved_from(self, start: CaretMark) -> bool {
        match (self, start) {
            (CaretMark::Index(now), CaretMark::Index(then)) => {
                (now - then).abs() >= TYPING_CHARACTER_THRESHOLD
            }
            (CaretMark::Point(x, y), CaretMark::Point(sx, sy)) => {
                ((x - sx).powi(2) + (y - sy).powi(2)).sqrt() >= TYPING_DISTANCE_THRESHOLD
            }
            // 표지 종류가 바뀌면(예: 캐럿 조회 경로가 바뀜) 비교할 수 없으므로 움직이지 않은 것으로 본다.
            _ => false,
        }
    }
}

/// 이번 틱에 관측한 포커스 상황
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputContext {
    /// 입력란에 포커스가 있음. `key`는 입력란을 구별하는 값(같은 입력란이면 같은 값)
    Field { key: u64, caret: Option<CaretMark> },
    /// 입력란이 아님
    NotField,
    /// 입력란인지 알 수 없음
    Unknown,
}

#[derive(Debug, Default)]
pub struct VisibilityTracker {
    current_field: Option<u64>,
    shown_at: Option<Instant>,
    start_caret: Option<CaretMark>,
    last_mode_change: Option<Instant>,
}

impl VisibilityTracker {
    /// 이번 틱의 관측을 반영하고 배지를 보일지 돌려준다.
    ///
    /// - `mode_changed_at`: 입력 모드가 마지막으로 바뀐 시각(ModeTracker::last_change)
    pub fn update(
        &mut self,
        now: Instant,
        context: InputContext,
        mode_changed_at: Option<Instant>,
    ) -> bool {
        let mode_changed = mode_changed_at.is_some() && mode_changed_at != self.last_mode_change;
        self.last_mode_change = mode_changed_at;

        match context {
            InputContext::NotField => {
                self.current_field = None;
                self.shown_at = None;
                false
            }
            InputContext::Unknown => {
                self.current_field = None;
                if mode_changed {
                    self.show(now, None);
                }
                // 캐럿 정보가 없으므로 시간으로만 숨긴다.
                self.within_duration(now)
            }
            InputContext::Field { key, caret } => {
                let entered = self.current_field != Some(key);
                self.current_field = Some(key);
                if entered || mode_changed {
                    self.show(now, caret);
                    return true;
                }
                if !self.within_duration(now) {
                    self.shown_at = None;
                    return false;
                }
                // 처음 띄울 때 캐럿 위치를 몰랐다면 처음 알게 된 위치를 기준점으로 삼는다.
                match (caret, self.start_caret) {
                    (Some(current), Some(start)) if current.moved_from(start) => {
                        self.shown_at = None;
                        false
                    }
                    (Some(current), None) => {
                        self.start_caret = Some(current);
                        true
                    }
                    _ => true,
                }
            }
        }
    }

    fn show(&mut self, now: Instant, caret: Option<CaretMark>) {
        self.shown_at = Some(now);
        self.start_caret = caret;
    }

    fn within_duration(&self, now: Instant) -> bool {
        self.shown_at
            .is_some_and(|shown| now.duration_since(shown) < SHOW_DURATION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(key: u64, index: i64) -> InputContext {
        InputContext::Field {
            key,
            caret: Some(CaretMark::Index(index)),
        }
    }

    fn ms(start: Instant, millis: u64) -> Instant {
        start + Duration::from_millis(millis)
    }

    #[test]
    fn hidden_outside_input_fields() {
        let mut tracker = VisibilityTracker::default();
        assert!(!tracker.update(Instant::now(), InputContext::NotField, None));
    }

    #[test]
    fn shows_on_entering_field_and_hides_after_duration() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        assert!(tracker.update(start, field(1, 0), None));
        assert!(tracker.update(ms(start, 1_999), field(1, 0), None));
        assert!(!tracker.update(ms(start, 2_000), field(1, 0), None));
        // 같은 입력란에 머무르는 동안 다시 뜨지 않는다.
        assert!(!tracker.update(ms(start, 5_000), field(1, 0), None));
    }

    #[test]
    fn hides_when_typing_starts() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        tracker.update(start, field(1, 10), None);
        // 1글자는 한글 조합 중일 수 있어 유지
        assert!(tracker.update(ms(start, 300), field(1, 11), None));
        assert!(!tracker.update(ms(start, 600), field(1, 12), None));
        // 숨긴 뒤에는 2초가 안 됐어도 다시 뜨지 않는다.
        assert!(!tracker.update(ms(start, 700), field(1, 10), None));
    }

    #[test]
    fn deleting_text_also_counts_as_typing() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        tracker.update(start, field(1, 10), None);
        assert!(!tracker.update(ms(start, 300), field(1, 8), None));
    }

    #[test]
    fn shows_again_when_moving_to_another_field() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        tracker.update(start, field(1, 0), None);
        tracker.update(ms(start, 3_000), field(1, 0), None);
        assert!(tracker.update(ms(start, 3_100), field(2, 0), None));
    }

    #[test]
    fn leaving_and_returning_to_same_field_shows_again() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        tracker.update(start, field(1, 0), None);
        tracker.update(ms(start, 3_000), InputContext::NotField, None);
        assert!(tracker.update(ms(start, 3_100), field(1, 0), None));
    }

    #[test]
    fn mode_change_inside_field_shows_again() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        tracker.update(start, field(1, 0), None);
        assert!(!tracker.update(ms(start, 3_000), field(1, 5), None));
        let changed = ms(start, 4_000);
        assert!(tracker.update(changed, field(1, 5), Some(changed)));
        // 같은 변경 시각이 다시 와도 새 변경으로 보지 않는다.
        assert!(!tracker.update(ms(start, 6_500), field(1, 5), Some(changed)));
    }

    #[test]
    fn mode_change_outside_field_does_not_show() {
        let mut tracker = VisibilityTracker::default();
        let now = Instant::now();
        assert!(!tracker.update(now, InputContext::NotField, Some(now)));
    }

    #[test]
    fn unknown_context_shows_only_after_mode_change() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        assert!(!tracker.update(start, InputContext::Unknown, None));
        let changed = ms(start, 100);
        assert!(tracker.update(changed, InputContext::Unknown, Some(changed)));
        assert!(tracker.update(ms(start, 2_000), InputContext::Unknown, Some(changed)));
        assert!(!tracker.update(ms(start, 2_100), InputContext::Unknown, Some(changed)));
    }

    #[test]
    fn point_marks_use_pixel_distance() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        let at = |x: f64| InputContext::Field {
            key: 7,
            caret: Some(CaretMark::Point(x, 100.0)),
        };
        tracker.update(start, at(100.0), None);
        assert!(tracker.update(ms(start, 100), at(108.0), None));
        assert!(!tracker.update(ms(start, 200), at(113.0), None));
    }

    #[test]
    fn unknown_caret_at_entry_uses_first_known_position() {
        let mut tracker = VisibilityTracker::default();
        let start = Instant::now();
        let no_caret = InputContext::Field {
            key: 1,
            caret: None,
        };
        assert!(tracker.update(start, no_caret, None));
        assert!(tracker.update(ms(start, 100), field(1, 4), None));
        assert!(tracker.update(ms(start, 200), field(1, 5), None));
        assert!(!tracker.update(ms(start, 300), field(1, 6), None));
    }
}
