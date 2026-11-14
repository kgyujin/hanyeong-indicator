//! 작성자: Git 이력 참조
//! 작성목적: 화면에 입력 모드 배지를 띄우는 macOS 오버레이 창
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 3단계 macOS 오버레이 | NSPanel 기반 배지, 화면 목록 조회 추가
//! - 2026-11-14 | 사용자 피드백 반영 | 시스템 모양(밝기 모드를 따르는 바탕·글자·테두리·그림자) 추가
//!
//! 창 성질(바꾸면 사용성이 깨지는 값):
//! - NonactivatingPanel: 배지가 떠도 사용자가 입력하던 앱의 포커스를 뺏지 않는다.
//! - ignoresMouseEvents: 클릭이 배지를 통과해 아래 창으로 간다.
//! - CanJoinAllSpaces + FullScreenAuxiliary: 모든 데스크톱(Space)과 전체 화면 앱 위에도 뜬다.
//! - 창 레벨 Status: 일반 창과 떠 있는 패널보다 위.
//!
//! 좌표: 입력은 core::state 좌표(주 모니터 좌상단 원점). Cocoa 창 좌표(주 모니터 좌하단 원점)로 바꿔 배치한다.
//! 비용: 같은 내용·위치면 AppKit 호출을 건너뛴다(100ms마다 호출되므로).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBox, NSBoxType, NSColor, NSFont, NSPanel, NSScreen, NSStatusWindowLevel,
    NSTextAlignment, NSTextField, NSTitlePosition, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use crate::core::state::ScreenRect;
use crate::core::style::{BadgeFill, BadgeLook, BadgeMetrics, badge_metrics};

pub struct BadgeOverlay {
    mtm: MainThreadMarker,
    panel: Retained<NSPanel>,
    background: Retained<NSBox>,
    label: Retained<NSTextField>,
    last_look: Option<BadgeLook>,
    metrics: BadgeMetrics,
    last_origin: Option<(f64, f64)>,
    visible: bool,
}

impl BadgeOverlay {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(20.0, 20.0));
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );
        // SAFETY: 이 패널은 BadgeOverlay가 Retained로 소유하므로 닫힐 때 해제되면 안 된다.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setIgnoresMouseEvents(true);
        panel.setHidesOnDeactivate(false);
        panel.setLevel(NSStatusWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );

        let background = NSBox::initWithFrame(NSBox::alloc(mtm), frame);
        background.setBoxType(NSBoxType::Custom);
        background.setTitlePosition(NSTitlePosition::NoTitle);
        background.setBorderWidth(0.0);
        background.setContentViewMargins(NSSize::new(0.0, 0.0));

        let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        label.setAlignment(NSTextAlignment::Center);
        label.setTextColor(Some(&NSColor::whiteColor()));
        background.addSubview(&label);
        panel.setContentView(Some(&background));

        Self {
            mtm,
            panel,
            background,
            label,
            last_look: None,
            metrics: badge_metrics(13.0, 13.0),
            last_origin: None,
            visible: false,
        }
    }

    /// 배지 크기(px). 위치 계산에 필요하므로 `apply_look` 뒤에 읽는다.
    pub fn size(&self) -> (f64, f64) {
        (self.metrics.width, self.metrics.height)
    }

    /// 글자·색·크기를 바꾼다. 같은 값이면 아무것도 하지 않는다.
    pub fn apply_look(&mut self, look: &BadgeLook) {
        if self.last_look.as_ref() == Some(look) {
            return;
        }
        match look.fill {
            // 동적 시스템 색은 그릴 때 현재 밝기 모드(라이트/다크)로 바뀐다.
            BadgeFill::System => {
                self.background
                    .setFillColor(&NSColor::windowBackgroundColor());
                self.background.setBorderColor(&NSColor::separatorColor());
                self.background.setBorderWidth(1.0);
                self.label.setTextColor(Some(&NSColor::labelColor()));
                self.panel.setHasShadow(true);
            }
            BadgeFill::Colored((r, g, b)) => {
                let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                    f64::from(r) / 255.0,
                    f64::from(g) / 255.0,
                    f64::from(b) / 255.0,
                    1.0,
                );
                self.background.setFillColor(&color);
                self.background.setBorderWidth(0.0);
                self.label.setTextColor(Some(&NSColor::whiteColor()));
                self.panel.setHasShadow(false);
            }
        }
        self.label
            .setFont(Some(&NSFont::boldSystemFontOfSize(look.font_size)));
        self.label.setStringValue(&NSString::from_str(&look.label));

        let text = self.label.fittingSize();
        self.metrics = badge_metrics(look.font_size, text.width);
        self.background.setCornerRadius(self.metrics.corner_radius);
        self.background.setFrame(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(self.metrics.width, self.metrics.height),
        ));
        self.label.setFrame(NSRect::new(
            NSPoint::new(
                ((self.metrics.width - text.width) / 2.0).round(),
                ((self.metrics.height - text.height) / 2.0).round(),
            ),
            text,
        ));
        self.panel.setAlphaValue(look.opacity);
        self.last_look = Some(look.clone());
        // 크기가 바뀌었으므로 다음 move_to에서 창 프레임을 다시 설정한다.
        self.last_origin = None;
    }

    /// 배지 좌상단을 (x, y)로 옮기고 보이게 한다. 좌표는 core::state 좌표 규칙.
    pub fn move_to(&mut self, x: f64, y: f64) {
        let origin = (x.round(), y.round());
        if self.last_origin != Some(origin) {
            let primary_height = primary_screen_height(self.mtm);
            let frame = NSRect::new(
                NSPoint::new(origin.0, primary_height - origin.1 - self.metrics.height),
                NSSize::new(self.metrics.width, self.metrics.height),
            );
            self.panel.setFrame_display(frame, true);
            self.last_origin = Some(origin);
        }
        if !self.visible {
            self.panel.orderFrontRegardless();
            self.visible = true;
        }
    }

    pub fn hide(&mut self) {
        if self.visible {
            self.panel.orderOut(None);
            self.visible = false;
        }
    }
}

fn primary_screen_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm)
        .firstObject()
        .map_or(0.0, |screen| screen.frame().size.height)
}

/// 각 모니터의 사용 가능 영역(메뉴 막대·Dock 제외)을 core::state 좌표로 돌려준다.
pub fn visible_screen_frames(mtm: MainThreadMarker) -> Vec<ScreenRect> {
    let primary_height = primary_screen_height(mtm);
    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.visibleFrame();
            ScreenRect {
                x: frame.origin.x,
                y: primary_height - (frame.origin.y + frame.size.height),
                width: frame.size.width,
                height: frame.size.height,
            }
        })
        .collect()
}
