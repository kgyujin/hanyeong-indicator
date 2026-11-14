//! 작성자: Git 이력 참조
//! 작성목적: 캐럿 또는 마우스 위치에서 배지를 띄울 화면 좌표를 계산한다
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 2단계 core 모듈 | 배지 위치 계산, 모니터 경계 보정, 위로 뒤집기 추가
//! - 2026-11-14 | 입력 시작 시 표시 | 캐럿 좌표가 없을 때 입력란 영역으로 캐럿 위치 추정
//! - 2026-11-14 | 사용자 피드백 반영 | 캐럿 아래 배치를 왼쪽 맞춤에서 가운데 맞춤으로 변경(macOS 기본 표시기와 비슷하게)
//!
//! 좌표는 core::state의 좌표 규칙(주 모니터 좌상단 원점, y 아래로 증가)을 따른다.
//! 모니터 좌표는 음수일 수 있다(주 모니터 왼쪽·위에 놓인 보조 모니터).

use serde::{Deserialize, Serialize};

use super::state::ScreenRect;

/// 캐럿과 배지 사이 간격(px)
const CARET_GAP: f64 = 3.0;
/// 마우스 포인터 끝에서 배지까지 거리(px). 포인터 그림을 가리지 않을 만큼 띄운다.
const MOUSE_OFFSET_X: f64 = 14.0;
const MOUSE_OFFSET_Y: f64 = 20.0;

/// 캐럿 기준 배지 위치(설정 값)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BadgePosition {
    /// 캐럿 바로 아래, 가운데 맞춤. macOS 기본 입력 소스 표시기와 같은 자리이고 입력 중인 글자를 가리지 않는다.
    #[default]
    Below,
    /// 캐럿 오른쪽, 세로 가운데
    Right,
}

/// 배지를 붙일 기준점
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Anchor {
    Caret(ScreenRect),
    Mouse { x: f64, y: f64 },
}

/// 입력란 왼쪽 위 첫 줄에 캐럿이 있다고 가정한 사각형.
/// 캐럿 좌표를 주지 않는 앱(예: macOS의 Chromium 계열)에서 입력란 영역만 알 때 기준점으로 쓴다.
/// 높이는 한 줄 높이(최대 22px)로 제한해, 여러 줄 입력란이어도 배지가 첫 줄 근처에 뜨게 한다.
pub fn estimated_caret_in_field(field: ScreenRect) -> ScreenRect {
    const INSET_X: f64 = 4.0;
    const MAX_LINE_HEIGHT: f64 = 22.0;
    ScreenRect {
        x: field.x + INSET_X,
        y: field.y,
        width: 1.0,
        height: field.height.min(MAX_LINE_HEIGHT),
    }
}

/// 배지 좌상단 좌표를 계산한다.
///
/// - `badge_width`, `badge_height`: 배지 크기(px)
/// - `monitors`: 각 모니터의 사용 가능 영역(메뉴 막대·Dock·작업 표시줄 제외)
///
/// 배지가 모니터 아래로 넘치면 기준점 위로 뒤집고, 그래도 넘치는 부분은 모니터 안으로 밀어 넣는다.
pub fn place_badge(
    anchor: Anchor,
    position: BadgePosition,
    badge_width: f64,
    badge_height: f64,
    monitors: &[ScreenRect],
) -> (f64, f64) {
    let (x, y, flipped_y) = match anchor {
        Anchor::Caret(caret) => match position {
            BadgePosition::Below => (
                caret.x + caret.width / 2.0 - badge_width / 2.0,
                caret.y + caret.height + CARET_GAP,
                caret.y - CARET_GAP - badge_height,
            ),
            BadgePosition::Right => {
                let centered = caret.y + (caret.height - badge_height) / 2.0;
                (caret.x + caret.width + CARET_GAP, centered, centered)
            }
        },
        Anchor::Mouse { x, y } => (
            x + MOUSE_OFFSET_X,
            y + MOUSE_OFFSET_Y,
            y - MOUSE_OFFSET_Y / 2.0 - badge_height,
        ),
    };

    let (anchor_x, anchor_y) = match anchor {
        Anchor::Caret(caret) => (caret.x, caret.y),
        Anchor::Mouse { x, y } => (x, y),
    };
    let Some(monitor) = monitor_for_point(anchor_x, anchor_y, monitors) else {
        return (x, y);
    };

    let bottom = monitor.y + monitor.height;
    let y = if y + badge_height > bottom {
        flipped_y
    } else {
        y
    };
    (
        clamp_start(x, badge_width, monitor.x, monitor.x + monitor.width),
        clamp_start(y, badge_height, monitor.y, bottom),
    )
}

/// 점이 들어 있는 모니터. 어느 모니터에도 없으면(모니터 사이 틈 등) 가장 가까운 모니터.
fn monitor_for_point(x: f64, y: f64, monitors: &[ScreenRect]) -> Option<ScreenRect> {
    monitors
        .iter()
        .copied()
        .find(|m| x >= m.x && x < m.x + m.width && y >= m.y && y < m.y + m.height)
        .or_else(|| {
            monitors
                .iter()
                .copied()
                .min_by(|a, b| distance_squared(x, y, a).total_cmp(&distance_squared(x, y, b)))
        })
}

fn distance_squared(x: f64, y: f64, rect: &ScreenRect) -> f64 {
    let dx = (rect.x - x).max(0.0).max(x - (rect.x + rect.width));
    let dy = (rect.y - y).max(0.0).max(y - (rect.y + rect.height));
    dx * dx + dy * dy
}

/// 길이 `size`인 구간의 시작점을 [min, max] 안으로 넣는다. 구간이 더 길면 min에 붙인다.
fn clamp_start(start: f64, size: f64, min: f64, max: f64) -> f64 {
    start.min(max - size).max(min)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: ScreenRect = ScreenRect {
        x: 0.0,
        y: 25.0,
        width: 1440.0,
        height: 875.0,
    };
    /// 주 모니터 왼쪽에 놓인 보조 모니터(좌표가 음수)
    const LEFT: ScreenRect = ScreenRect {
        x: -1920.0,
        y: -200.0,
        width: 1920.0,
        height: 1080.0,
    };
    const BADGE_W: f64 = 24.0;
    const BADGE_H: f64 = 20.0;
    /// x=100, 폭 1인 캐럿 아래 가운데 맞춤한 배지의 왼쪽 끝
    const CENTERED_X: f64 = 100.0 + 0.5 - BADGE_W / 2.0;

    fn caret(x: f64, y: f64) -> Anchor {
        Anchor::Caret(ScreenRect {
            x,
            y,
            width: 1.0,
            height: 16.0,
        })
    }

    #[test]
    fn field_estimate_uses_first_line_of_field() {
        let single_line = ScreenRect {
            x: 100.0,
            y: 50.0,
            width: 300.0,
            height: 18.0,
        };
        assert_eq!(
            estimated_caret_in_field(single_line),
            ScreenRect {
                x: 104.0,
                y: 50.0,
                width: 1.0,
                height: 18.0
            }
        );
        let text_area = ScreenRect {
            height: 200.0,
            ..single_line
        };
        assert_eq!(estimated_caret_in_field(text_area).height, 22.0);
    }

    #[test]
    fn below_caret_by_default() {
        let placed = place_badge(
            caret(100.0, 200.0),
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert_eq!(placed, (CENTERED_X, 200.0 + 16.0 + CARET_GAP));
    }

    #[test]
    fn right_of_caret_vertically_centered() {
        let placed = place_badge(
            caret(100.0, 200.0),
            BadgePosition::Right,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert_eq!(
            placed,
            (100.0 + 1.0 + CARET_GAP, 200.0 + (16.0 - BADGE_H) / 2.0)
        );
    }

    #[test]
    fn flips_above_caret_at_bottom_edge() {
        // 사용 가능 영역 아래쪽 끝 = 25 + 875 = 900
        let placed = place_badge(
            caret(100.0, 885.0),
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert_eq!(placed, (CENTERED_X, 885.0 - CARET_GAP - BADGE_H));
    }

    #[test]
    fn clamps_to_right_edge() {
        let placed = place_badge(
            caret(1435.0, 200.0),
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert_eq!(placed.0, 1440.0 - BADGE_W);
    }

    #[test]
    fn clamps_below_menu_bar() {
        let placed = place_badge(
            Anchor::Mouse { x: 10.0, y: 26.0 },
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert!(placed.1 >= MAIN.y);
    }

    #[test]
    fn mouse_anchor_offsets_from_pointer() {
        let placed = place_badge(
            Anchor::Mouse { x: 500.0, y: 400.0 },
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN],
        );
        assert_eq!(placed, (500.0 + MOUSE_OFFSET_X, 400.0 + MOUSE_OFFSET_Y));
    }

    #[test]
    fn uses_monitor_with_negative_coordinates() {
        // 왼쪽 모니터의 오른쪽 끝 캐럿: 주 모니터로 넘어가지 않고 왼쪽 모니터 안에 남는다.
        let placed = place_badge(
            caret(-5.0, 100.0),
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN, LEFT],
        );
        assert_eq!(placed, (-BADGE_W, 100.0 + 16.0 + CARET_GAP));
    }

    #[test]
    fn point_outside_all_monitors_uses_nearest() {
        let placed = place_badge(
            Anchor::Mouse {
                x: 2000.0,
                y: 400.0,
            },
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[MAIN, LEFT],
        );
        assert_eq!(placed.0, 1440.0 - BADGE_W);
    }

    #[test]
    fn no_monitor_information_keeps_raw_position() {
        let placed = place_badge(
            caret(100.0, 200.0),
            BadgePosition::Below,
            BADGE_W,
            BADGE_H,
            &[],
        );
        assert_eq!(placed, (CENTERED_X, 200.0 + 16.0 + CARET_GAP));
    }
}
