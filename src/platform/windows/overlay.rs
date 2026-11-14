//! 작성자: Git 이력 참조
//! 작성목적: 화면에 입력 모드 배지를 띄우는 Windows 오버레이 창
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 5단계 Windows 오버레이 | 레이어드 팝업 창, 둥근 영역, 모니터 작업 영역·DPI 조회 추가
//!
//! 창 성질(바꾸면 사용성이 깨지는 값):
//! - WS_EX_NOACTIVATE: 배지가 떠도 사용자가 입력하던 창의 포커스를 뺏지 않는다.
//! - WS_EX_TRANSPARENT + WM_NCHITTEST=HTTRANSPARENT: 클릭이 배지를 통과한다.
//! - WS_EX_TOOLWINDOW: 작업 표시줄과 Alt+Tab에 나타나지 않는다.
//! - WS_EX_TOPMOST: 일반 창 위. 다른 최상위 창에 가려지지 않도록 이동할 때마다 다시 최상위로 올린다.
//! - WS_EX_LAYERED + LWA_ALPHA: 창 전체 불투명도. 모양은 SetWindowRgn 둥근 사각형 영역으로 자른다.
//!
//! 좌표: Per-Monitor DPI v2 물리 픽셀. 글자 크기(논리 pt)는 배지가 놓일 모니터의 DPI로 픽셀로 바꾼다.

use std::cell::RefCell;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateRoundRectRgn, EndPaint, EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, PAINTSTRUCT,
    SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, HTTRANSPARENT, HWND_TOPMOST, LWA_ALPHA, RegisterClassW,
    SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes, SetWindowPos, ShowWindow,
    WM_NCHITTEST, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

use super::render::{draw_badge, measure_text};
use crate::core::state::ScreenRect;
use crate::core::style::{BadgeLook, BadgeMetrics, badge_metrics};

/// 기준 DPI(배율 100%)
const BASE_DPI: f64 = 96.0;

/// WM_PAINT가 그릴 내용. 창 프로시저는 BadgeOverlay에 접근할 수 없어 따로 둔다.
#[derive(Default)]
struct PaintState {
    label: String,
    rgb: (u8, u8, u8),
    pixel_height: i32,
}

thread_local! {
    static PAINT: RefCell<PaintState> = RefCell::new(PaintState::default());
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_PAINT => {
            unsafe {
                let mut paint = PAINTSTRUCT::default();
                let dc: HDC = BeginPaint(window, &mut paint);
                PAINT.with(|state| {
                    let state = state.borrow();
                    let mut rect = RECT::default();
                    let _ =
                        windows::Win32::UI::WindowsAndMessaging::GetClientRect(window, &mut rect);
                    draw_badge(dc, &rect, &state.label, state.rgb, state.pixel_height);
                });
                let _ = EndPaint(window, &paint);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

pub struct BadgeOverlay {
    window: HWND,
    last_look: Option<(BadgeLook, u32)>,
    metrics: BadgeMetrics,
    last_origin: Option<(i32, i32)>,
    visible: bool,
}

impl BadgeOverlay {
    pub fn new() -> windows::core::Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: w!("HanyeongIndicatorBadge"),
                ..Default::default()
            };
            // 이미 등록된 경우(0 반환)에도 CreateWindowExW는 성공하므로 결과를 따로 확인하지 않는다.
            RegisterClassW(&class);
            let window = CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST
                    | WS_EX_NOACTIVATE,
                w!("HanyeongIndicatorBadge"),
                w!(""),
                WS_POPUP,
                0,
                0,
                20,
                20,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            Ok(Self {
                window,
                last_look: None,
                metrics: badge_metrics(13.0, 13.0),
                last_origin: None,
                visible: false,
            })
        }
    }

    pub fn size(&self) -> (f64, f64) {
        (self.metrics.width, self.metrics.height)
    }

    /// 글자·색·크기·불투명도를 바꾼다. `dpi`는 배지가 놓일 모니터의 DPI.
    pub fn apply_look(&mut self, look: &BadgeLook, dpi: u32) {
        if self
            .last_look
            .as_ref()
            .is_some_and(|(last, last_dpi)| last == look && *last_dpi == dpi)
        {
            return;
        }
        let scale = f64::from(dpi) / BASE_DPI;
        let pixel_height = (look.font_size * scale).round() as i32;
        let (text_width, _) = measure_text(&look.label, pixel_height);
        self.metrics = badge_metrics(f64::from(pixel_height), f64::from(text_width));
        let width = self.metrics.width as i32;
        let height = self.metrics.height as i32;
        let diameter = (self.metrics.corner_radius * 2.0) as i32;

        PAINT.with(|state| {
            *state.borrow_mut() = PaintState {
                label: look.label.clone(),
                rgb: look.rgb,
                pixel_height,
            };
        });
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                0,
                0,
                width,
                height,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOMOVE | SWP_NOACTIVATE,
            );
            // 영역 소유권은 시스템으로 넘어가므로 해제하지 않는다.
            let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
            SetWindowRgn(self.window, Some(region), true);
            let alpha = (look.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
            let _ = SetLayeredWindowAttributes(self.window, Default::default(), alpha, LWA_ALPHA);
            let _ = InvalidateRect(Some(self.window), None, true);
        }
        self.last_look = Some((look.clone(), dpi));
        self.last_origin = None;
    }

    /// 배지 좌상단을 (x, y)로 옮기고 보이게 한다.
    pub fn move_to(&mut self, x: f64, y: f64) {
        let origin = (x.round() as i32, y.round() as i32);
        if self.last_origin == Some(origin) && self.visible {
            return;
        }
        unsafe {
            let _ = SetWindowPos(
                self.window,
                Some(HWND_TOPMOST),
                origin.0,
                origin.1,
                self.metrics.width as i32,
                self.metrics.height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        self.last_origin = Some(origin);
        self.visible = true;
    }

    pub fn hide(&mut self) {
        if self.visible {
            unsafe {
                let _ = ShowWindow(self.window, SW_HIDE);
            }
            self.visible = false;
        }
    }
}

/// 점이 있는 모니터(없으면 가장 가까운 모니터)의 DPI
pub fn dpi_for_point(x: f64, y: f64) -> u32 {
    unsafe {
        let monitor = MonitorFromPoint(
            POINT {
                x: x.round() as i32,
                y: y.round() as i32,
            },
            MONITOR_DEFAULTTONEAREST,
        );
        let (mut dpi_x, mut dpi_y) = (0, 0);
        match GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) {
            Ok(()) if dpi_x > 0 => dpi_x,
            _ => BASE_DPI as u32,
        }
    }
}

unsafe extern "system" fn collect_monitor(
    monitor: HMONITOR,
    _dc: HDC,
    _clip: *mut RECT,
    data: LPARAM,
) -> windows::core::BOOL {
    unsafe {
        let areas = &mut *(data.0 as *mut Vec<ScreenRect>);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let work = info.rcWork;
            areas.push(ScreenRect {
                x: f64::from(work.left),
                y: f64::from(work.top),
                width: f64::from(work.right - work.left),
                height: f64::from(work.bottom - work.top),
            });
        }
    }
    true.into()
}

/// 각 모니터의 작업 영역(작업 표시줄 제외, 물리 픽셀)
pub fn monitor_work_areas() -> Vec<ScreenRect> {
    let mut areas: Vec<ScreenRect> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_monitor),
            LPARAM(&mut areas as *mut Vec<ScreenRect> as isize),
        );
    }
    areas
}
