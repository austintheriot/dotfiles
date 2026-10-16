//! Where the panel goes. Pure: `panel.rs` reads the screen and applies it.

/// The panel width in points: a 380-point card plus 12 points each side
/// for the shadow and the close button that overhangs the corner.
pub const PANEL_WIDTH: f64 = 404.0;
/// The gap to the screen's top and right edges.
pub const MARGIN: f64 = 16.0;

/// A rectangle in AppKit screen coordinates, origin at the bottom left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Bottom edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// The panel frame for content `content_height_px` tall (CSS pixels equal
/// points) in the screen's `visible` frame, which excludes the menu bar.
#[must_use]
pub fn panel_frame(visible: Rect, content_height_px: u32) -> Rect {
    let width = PANEL_WIDTH.min(visible.width - 2.0 * MARGIN).max(1.0);
    let height = f64::from(content_height_px).min(visible.height - 2.0 * MARGIN).max(1.0);
    Rect { x: visible.x + visible.width - width - MARGIN, y: visible.y + visible.height - height - MARGIN, width, height }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Rect = Rect { x: 0.0, y: 0.0, width: 2560.0, height: 1415.0 };

    #[test]
    fn the_panel_sits_top_right_inside_the_margin() {
        let frame = panel_frame(DISPLAY, 212);
        assert_eq!(frame, Rect { x: 2560.0 - PANEL_WIDTH - MARGIN, y: 1415.0 - 212.0 - MARGIN, width: PANEL_WIDTH, height: 212.0 });
    }

    #[test]
    fn a_second_display_offsets_the_frame() {
        let left = Rect { x: -1920.0, y: 200.0, width: 1920.0, height: 1055.0 };
        let frame = panel_frame(left, 100);
        assert_eq!(frame.x, -1920.0 + 1920.0 - PANEL_WIDTH - MARGIN);
        assert_eq!(frame.y, 200.0 + 1055.0 - 100.0 - MARGIN);
    }

    #[test]
    fn tall_content_is_clamped_to_the_screen() {
        let frame = panel_frame(DISPLAY, 5000);
        assert_eq!(frame.height, 1415.0 - 2.0 * MARGIN);
        assert_eq!(frame.y, DISPLAY.y + MARGIN);
    }

    #[test]
    fn zero_height_is_one_point_and_a_tiny_screen_still_fits() {
        assert_eq!(panel_frame(DISPLAY, 0).height, 1.0);
        let tiny = Rect { x: 0.0, y: 0.0, width: 300.0, height: 200.0 };
        let frame = panel_frame(tiny, 500);
        assert!(frame.width <= 300.0 - 2.0 * MARGIN && frame.x >= MARGIN, "{frame:?}");
    }
}
