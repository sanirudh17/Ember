//! Window chrome: a solid ground and rounded corners.
//!
//! Ember no longer uses Mica or Acrylic. The window paints its own solid
//! surface, so the only compositor work left is clearing any leftover effect
//! (in case an older build applied one) and rounding the corners on Windows 11.

use tauri::WebviewWindow;
use window_vibrancy::{clear_acrylic, clear_mica};

/// Clear Mica/Acrylic once and enable rounded corners. Safe to call repeatedly;
/// a failure is never fatal — the window still paints its solid background.
pub fn apply_solid(window: &WebviewWindow) {
    let _ = clear_mica(window);
    let _ = clear_acrylic(window);
    let _ = enable_rounded_corners(window);
}

/// Win11 rounded corners for the frameless window (no-op on Windows 10).
pub fn enable_rounded_corners(window: &WebviewWindow) -> bool {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
    };

    let Ok(handle) = window.hwnd() else {
        return false;
    };
    let raw = handle.0 as isize;
    if raw == 0 {
        return false;
    }
    let hwnd = HWND(raw as *mut core::ffi::c_void);
    let preference = DWMWCP_ROUND;
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&preference) as u32,
        )
        .is_ok()
    }
}

/// A 16x16 ember dot for the taskbar overlay: warm centre, transparent edge.
pub fn ember_dot_rgba() -> Vec<u8> {
    const SIZE: u32 = 16;
    let mut pixels = vec![0u8; (SIZE * SIZE * 4) as usize];
    let centre = (SIZE as f32 - 1.0) / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - centre;
            let dy = y as f32 - centre;
            let distance = (dx * dx + dy * dy).sqrt();
            let radius = centre + 0.2;
            if distance > radius {
                continue;
            }
            // Soft edge so the dot never looks like a hard square.
            let edge = ((radius - distance) / 1.6).clamp(0.0, 1.0);
            let alpha = (edge * 255.0) as u8;
            let heat = (1.0 - distance / radius).clamp(0.0, 1.0);
            let r = (255.0 * (0.55 + 0.45 * heat)) as u8;
            let g = (255.0 * (0.16 + 0.52 * heat * heat)) as u8;
            let b = (255.0 * (0.02 + 0.22 * heat * heat * heat)) as u8;
            let i = ((y * SIZE + x) * 4) as usize;
            pixels[i] = r;
            pixels[i + 1] = g;
            pixels[i + 2] = b;
            pixels[i + 3] = alpha;
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ember_dot_is_a_16px_rgba_disc_with_a_transparent_corner() {
        let pixels = ember_dot_rgba();
        assert_eq!(pixels.len(), 16 * 16 * 4);
        // Top-left pixel is outside the disc.
        assert_eq!(pixels[3], 0, "corner must be transparent");
        // Centre is opaque and warm.
        let centre = ((8 * 16 + 8) * 4) as usize;
        assert!(pixels[centre + 3] > 200, "centre must be opaque");
        assert!(
            pixels[centre] > pixels[centre + 2],
            "centre must be warmer than blue"
        );
    }
}
