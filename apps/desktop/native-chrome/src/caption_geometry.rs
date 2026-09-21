//! Native caption coordinates are physical pixels; CSS targets are 46 by 32 DIPs.

pub fn is_maximize_target(width: i32, x: i32, y: i32, dpi: u32) -> bool {
    if width <= 0 || dpi == 0 || dpi > 960 || y < 0 {
        return false;
    }
    let button_width = (46 * dpi as i32 + 48) / 96;
    let button_height = (32 * dpi as i32 + 48) / 96;
    x >= width - 2 * button_width && x < width - button_width && y < button_height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximize_excludes_the_adjacent_close_and_minimize_buttons() {
        assert!(is_maximize_target(1280, 1188, 0, 96));
        assert!(is_maximize_target(1280, 1233, 31, 96));
        assert!(!is_maximize_target(1280, 1187, 10, 96));
        assert!(!is_maximize_target(1280, 1234, 10, 96));
        assert!(!is_maximize_target(1280, 1200, 32, 96));
        assert!(!is_maximize_target(1280, 1200, -1, 96));
    }

    #[test]
    fn hit_target_tracks_per_monitor_dpi_in_physical_pixels() {
        assert!(is_maximize_target(1920, 1782, 47, 144));
        assert!(!is_maximize_target(1920, 1851, 47, 144));
        assert!(!is_maximize_target(1920, 1800, 48, 144));
        assert!(is_maximize_target(2560, 2376, 63, 192));
        assert!(!is_maximize_target(2560, 2468, 63, 192));
        assert!(!is_maximize_target(0, -1, 1, 96));
        assert!(!is_maximize_target(1280, 1200, 1, 0));
    }
}
