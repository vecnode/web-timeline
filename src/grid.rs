use crate::{context::TimelineCtx, ruler, types::MIN_STEP_GAP};

/// Paints the grid over the timeline `Rect`.
///
/// If using a custom `background`, you may wish to call this after.
///
/// The grid is positioned so that tick 0 always aligns with the left edge of the timeline area
/// (where the header ends), keeping it "glued" to the left edge.
///
/// Uses time-based grid (seconds) instead of musical subdivisions:
/// - Maximum 10 lines per second (0.1 second intervals)
/// - Automatically hides lines that are too close (less than MIN_STEP_GAP pixels apart)
pub fn paint_grid(ui: &mut egui::Ui, timeline: &TimelineCtx, info: &dyn ruler::MusicalInfo) {
    let vis = ui.style().noninteractive();
    let mut stroke = vis.bg_stroke;
    let second_color = stroke.color.linear_multiply(0.5); // Whole seconds - darker
    let subdivision_color = stroke.color.linear_multiply(0.25); // 0.1 second subdivisions - lighter
    
    let tl_rect = timeline.full_rect;
    let visible_len = tl_rect.width();
    let ticks_per_point = info.ticks_per_point();
    let visible_ticks = ticks_per_point * visible_len;
    
    // Calculate ticks per second (1 bar = 1 second)
    let ticks_per_beat = info.ticks_per_beat() as f32;
    const BEATS_PER_BAR: f32 = 4.0; // 4/4 time signature
    let ticks_per_bar = ticks_per_beat * BEATS_PER_BAR;
    let ticks_per_second = ticks_per_bar; // 1 bar = 1 second
    
    // Maximum 10 lines per second = 0.1 second intervals
    const MAX_LINES_PER_SECOND: f32 = 10.0;
    let ticks_per_line = ticks_per_second / MAX_LINES_PER_SECOND; // ticks per 0.1 second
    
    // Get timeline start to calculate absolute positions
    let timeline_start = info.timeline_start().unwrap_or(0.0);
    
    // Calculate the starting tick for the visible area (relative to timeline start)
    // The visible area starts at tick 0 relative to timeline_start
    let start_tick_relative = 0.0;
    
    // Find the first line position (snap to 0.1 second intervals)
    // We need to find the first 0.1 second interval that's visible
    // Convert relative start to absolute, then find the first interval
    let absolute_start_tick = timeline_start + start_tick_relative;
    let absolute_start_seconds = absolute_start_tick / ticks_per_second;
    // Find the first 0.1 second interval that's >= absolute_start_seconds
    let first_line_seconds = (absolute_start_seconds * MAX_LINES_PER_SECOND).floor() / MAX_LINES_PER_SECOND;
    let first_line_absolute_tick = first_line_seconds * ticks_per_second;
    // Convert back to relative tick
    let first_line_tick_relative = first_line_absolute_tick - timeline_start;
    
    // Get maximum absolute tick position (if available)
    let max_absolute_tick = info.max_absolute_tick();
    
    // Draw grid lines
    let mut current_tick_relative = first_line_tick_relative;
    let mut last_x = f32::NEG_INFINITY;
    
    while current_tick_relative <= visible_ticks {
        // Convert relative tick to x position
        let x = tl_rect.left() + (current_tick_relative / ticks_per_point);
        
        // Calculate absolute tick to check against maximum
        let absolute_tick = timeline_start + current_tick_relative;
        
        // Stop drawing if we've exceeded the maximum position
        if let Some(max_tick) = max_absolute_tick {
            if absolute_tick > max_tick {
                break;
            }
        }
        
        // Skip if line is too close to the previous one (less than MIN_STEP_GAP pixels)
        if (x - last_x).abs() < MIN_STEP_GAP && last_x != f32::NEG_INFINITY {
            current_tick_relative += ticks_per_line;
            continue;
        }
        
        // Determine if this is a whole second (darker) or subdivision (lighter)
        let seconds = absolute_tick / ticks_per_second;
        let is_whole_second = (seconds % 1.0).abs() < 0.001; // Check if it's a whole second
        
        stroke.color = if is_whole_second {
            second_color
        } else {
            subdivision_color
        };
        
        // Draw the line
        let a = egui::Pos2::new(x, tl_rect.top());
        let b = egui::Pos2::new(x, tl_rect.bottom());
        ui.painter().line_segment([a, b], stroke);
        
        last_x = x;
        current_tick_relative += ticks_per_line;
    }
}
