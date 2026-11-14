//! 작성자: Git 이력 참조
//! 작성목적: GDI로 배지 글자를 재고 그리는 공통 함수 (배지 창, 알림 영역 아이콘)
//! 작성일: 2026-11-14
//! 변경사항 내역:
//! - 2026-11-14 | 5단계 Windows 오버레이 | 글꼴 생성, 글자 폭 측정, 아이콘 RGBA 렌더링 추가
//! - 2026-11-14 | 사용자 피드백 반영 | 배경·글자 색을 인자로 받음(시스템 모양 지원)
//!
//! 글꼴: "Segoe UI" 굵게. 한글·가나·한자는 Windows 글꼴 연결(font linking)로 맑은 고딕 등이 대신 그린다.
//! 품질: ClearType은 투명 배경에서 색 번짐이 생기므로 회색조 안티에일리어싱(ANTIALIASED_QUALITY)을 쓴다.
//! GDI 객체는 만든 함수 안에서 모두 해제한다.

use windows::Win32::Foundation::{COLORREF, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    ANTIALIASED_QUALITY, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CLIP_DEFAULT_PRECIS,
    CreateCompatibleDC, CreateDIBSection, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DIB_RGB_COLORS, DT_CENTER, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW,
    FW_BOLD, FillRect, GetDC, GetTextExtentPoint32W, HDC, HFONT, OUT_DEFAULT_PRECIS, ReleaseDC,
    SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::core::w;

use crate::core::style::rounded_rect_coverage;

pub fn colorref((r, g, b): (u8, u8, u8)) -> COLORREF {
    COLORREF(u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16))
}

/// 글자 높이 `pixel_height`(px)의 굵은 글꼴. 호출자가 DeleteObject로 해제한다.
pub fn create_font(pixel_height: i32) -> HFONT {
    unsafe {
        CreateFontW(
            -pixel_height.max(1),
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            0,
            w!("Segoe UI"),
        )
    }
}

pub fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// 글꼴로 그린 글자의 폭과 높이(px)
pub fn measure_text(text: &str, pixel_height: i32) -> (i32, i32) {
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        ReleaseDC(None, screen);
        let font = create_font(pixel_height);
        let previous = SelectObject(dc, font.into());
        let mut size = SIZE::default();
        let _ = GetTextExtentPoint32W(dc, &to_wide(text), &mut size);
        SelectObject(dc, previous);
        let _ = DeleteObject(font.into());
        let _ = DeleteDC(dc);
        (size.cx, size.cy)
    }
}

/// 시스템 모양 배지 색(밝은 회색 바탕, 검은 글자). Windows 11 밝은 테마의 도구 설명 색에 맞춘다.
/// 어두운 테마 감지는 아직 하지 않는다.
pub const SYSTEM_BACKGROUND: (u8, u8, u8) = (0xF3, 0xF3, 0xF3);
pub const SYSTEM_TEXT: (u8, u8, u8) = (0x1F, 0x1F, 0x1F);
pub const WHITE: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);

/// `dc`의 `rect` 영역을 배경색으로 채우고 가운데에 글자를 그린다.
pub fn draw_badge(
    dc: HDC,
    rect: &RECT,
    text: &str,
    background: (u8, u8, u8),
    foreground: (u8, u8, u8),
    pixel_height: i32,
) {
    unsafe {
        let brush = CreateSolidBrush(colorref(background));
        FillRect(dc, rect, brush);
        let _ = DeleteObject(brush.into());

        let font = create_font(pixel_height);
        let previous = SelectObject(dc, font.into());
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, colorref(foreground));
        let mut text_rect = *rect;
        DrawTextW(
            dc,
            &mut to_wide(text),
            &mut text_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
        SelectObject(dc, previous);
        let _ = DeleteObject(font.into());
    }
}

/// 알림 영역 아이콘용 정사각형 배지 이미지(RGBA, 위에서 아래 행 순서).
/// 실패하면 글자 없이 배경색만 칠한 이미지를 돌려준다.
pub fn render_badge_rgba(text: &str, rgb: (u8, u8, u8), size: i32) -> Vec<u8> {
    let side = size.max(1);
    let radius = f64::from(side) * 0.22;
    let pixel_count = (side * side) as usize;
    let mut bgra: Option<Vec<u8>> = None;

    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        ReleaseDC(None, screen);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: side,
                // 음수 높이 = 위에서 아래로 저장(top-down)
                biHeight: -side,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        if let Ok(bitmap) = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
            && !bits.is_null()
        {
            let previous = SelectObject(dc, bitmap.into());
            let rect = RECT {
                left: 0,
                top: 0,
                right: side,
                bottom: side,
            };
            draw_badge(
                dc,
                &rect,
                text,
                rgb,
                WHITE,
                (f64::from(side) * 0.68).round() as i32,
            );
            bgra = Some(std::slice::from_raw_parts(bits as *const u8, pixel_count * 4).to_vec());
            SelectObject(dc, previous);
            let _ = DeleteObject(bitmap.into());
        }
        let _ = DeleteDC(dc);
    }

    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for index in 0..pixel_count {
        let x = (index as i32 % side) as f64;
        let y = (index as i32 / side) as f64;
        let coverage = rounded_rect_coverage(x, y, f64::from(side), f64::from(side), radius);
        let (r, g, b) = match &bgra {
            Some(pixels) => (
                pixels[index * 4 + 2],
                pixels[index * 4 + 1],
                pixels[index * 4],
            ),
            None => rgb,
        };
        rgba.extend_from_slice(&[r, g, b, (coverage * 255.0).round() as u8]);
    }
    rgba
}
