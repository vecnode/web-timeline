use crate::types::Bar;

pub trait MusicalInfo {
    /// The number of ticks per beat, also known as PPQN (parts per quarter note).
    fn ticks_per_beat(&self) -> u32;
    /// The bar at the given tick offset starting from the beginning (left) of the timeline view.
    fn bar_at_ticks(&self, tick: f32) -> Bar;
    /// Affects how "zoomed" the timeline is. By default, uses 16 points per beat.
    fn ticks_per_point(&self) -> f32 {
        self.ticks_per_beat() as f32 / 16.0
    }
    /// Get the current timeline start position in ticks (for calculating absolute bar numbers).
    /// Returns None if not available.
    fn timeline_start(&self) -> Option<f32> {
        None
    }
    /// Get the maximum absolute tick position (end of timeline).
    /// Returns None if not available (will draw indefinitely).
    fn max_absolute_tick(&self) -> Option<f32> {
        None
    }
}

/// Respond to when the user clicks on the ruler.
pub trait MusicalInteract {
    /// The given tick location was clicked
    fn click_at_tick(&mut self, tick: f32);
}

/// The required API for the musical ruler widget.
pub trait MusicalRuler {
    fn info(&self) -> &dyn MusicalInfo;
    fn interact(&mut self) -> &mut dyn MusicalInteract;
}

pub fn musical(ui: &mut egui::Ui, api: &mut dyn MusicalRuler) -> egui::Response {
    // Use fixed height to match track height and prevent overflow
    const RULER_HEIGHT: f32 = 20.0;
    let w = ui.available_rect_before_wrap().width();
    let desired_size = egui::Vec2::new(w, RULER_HEIGHT);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click_and_drag());

    let w = rect.width();
    let ticks_per_point = api.info().ticks_per_point();
    let visible_ticks = w * ticks_per_point;
    let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    let pointer_over = ui.input(|i| {
        i.pointer.hover_pos()
            .map(|pos| rect.contains(pos))
            .unwrap_or(false)
    });
    // Handle both click and drag to move playhead (same as tracks)
    if (pointer_pressed && pointer_over) || (pointer_down && pointer_over) || response.dragged() {
        if let Some(pt) = response.interact_pointer_pos() {
            // Calculate tick relative to timeline start (same calculation as tracks)
            let tick = (((pt.x - rect.min.x) / w) * visible_ticks).max(0.0);
            api.interact().click_at_tick(tick);
            response.mark_changed();
        }
    }

    let vis = ui.style().noninteractive();
    // Note: Pink border is drawn by the track's show() method to include header + timeline
    // No need to draw border here as it would only cover the timeline area

    let mut stroke = vis.fg_stroke;
    let bar_color = stroke.color.linear_multiply(0.5);
    let step_color = stroke.color.linear_multiply(0.125);
    let bar_y = rect.center().y;
    let step_even_y = rect.top() + rect.height() * 0.25;
    let step_odd_y = rect.top() + rect.height() * 0.125;

    let visible_len = w;
    let info = api.info();
    let ticks_per_point = info.ticks_per_point();
    let visible_ticks = ticks_per_point * visible_len;
    
    // Calculate ticks per second (1 bar = 1 second) - same logic as grid
    let ticks_per_beat = info.ticks_per_beat() as f32;
    const BEATS_PER_BAR: f32 = 4.0; // 4/4 time signature
    let ticks_per_bar = ticks_per_beat * BEATS_PER_BAR;
    let ticks_per_second = ticks_per_bar; // 1 bar = 1 second
    
    // Maximum 10 lines per second = 0.1 second intervals - same as grid
    const MAX_LINES_PER_SECOND: f32 = 10.0;
    let ticks_per_line = ticks_per_second / MAX_LINES_PER_SECOND; // ticks per 0.1 second
    
    // Get timeline start to calculate absolute positions
    let timeline_start = info.timeline_start().unwrap_or(0.0);
    
    // Calculate the starting tick for the visible area (relative to timeline start)
    // The visible area starts at tick 0 relative to timeline_start
    let start_tick_relative = 0.0;
    
    // Find the first line position (snap to 0.1 second intervals) - same as grid
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
    
    // Draw ruler lines using same logic as grid
    let mut current_tick_relative = first_line_tick_relative;
    let mut last_x = f32::NEG_INFINITY;
    let mut last_bar_number_at_x: Option<(u32, f32)> = None; // Track (bar_number, x_position)
    
    while current_tick_relative <= visible_ticks {
        // Determine if this is a whole second (bar) or subdivision
        let absolute_tick = timeline_start + current_tick_relative;
        
        // Stop drawing if we've reached or exceeded the maximum position
        if let Some(max_tick) = max_absolute_tick {
            if absolute_tick >= max_tick {
                break;
            }
        }
        
        // Convert relative tick to x position - same calculation as grid
        let x = rect.left() + (current_tick_relative / ticks_per_point);
        
        let seconds = absolute_tick / ticks_per_second;
        let is_whole_second = (seconds % 1.0).abs() < 0.001;
        
        // Check if line is too close to the previous one (less than MIN_STEP_GAP pixels)
        let line_too_close = (x - last_x).abs() < crate::types::MIN_STEP_GAP && last_x != f32::NEG_INFINITY;
        
        // Draw the line with appropriate style (skip subdivisions if too close, but always draw whole seconds)
        if is_whole_second {
            // Whole second (bar) - always draw the line, even if close (but might be shorter)
            stroke.color = bar_color;
            let a = egui::Pos2::new(x, rect.top());
            let b = egui::Pos2::new(x, bar_y);
            ui.painter().line_segment([a, b], stroke);
            
            // Draw bar number - only draw each number once, never duplicate
            let bar_number = seconds.floor() as u32;
            // Calculate maximum seconds from max_absolute_tick
            let max_seconds = if let Some(max_tick) = max_absolute_tick {
                (max_tick / ticks_per_second).floor() as u32
            } else {
                500 // Fallback if max not available
            };
            
            // Only clamp if we're actually past the maximum (don't always clamp to max-1)
            // If bar_number equals max_seconds, that's the last valid second, so allow it
            let bar_number = if bar_number > max_seconds {
                max_seconds
            } else {
                bar_number
            };
            
            // FIX: Only draw if it's a NEW bar number (never draw the same number twice)
            // This prevents duplicates when zooming changes x positions
            let should_draw_number = match last_bar_number_at_x {
                None => true, // First bar number
                Some((last_bar, _last_x_pos)) => {
                    // Only draw if it's a different bar number (ignore x position changes)
                    bar_number != last_bar
                }
            };
            
            if should_draw_number {
                const MIN_LEFT_MARGIN: f32 = 20.0;
                const MIN_RIGHT_MARGIN: f32 = 30.0;
                let text = format!("{}", bar_number);
                let estimated_text_width = text.len() as f32 * 6.0;
                let fits_left = x >= rect.left() + MIN_LEFT_MARGIN;
                let fits_right = x + estimated_text_width <= rect.right() - MIN_RIGHT_MARGIN;
                
                if fits_left && fits_right {
                    let text_color = vis.fg_stroke.color;
                    let text_pos = egui::Pos2::new(x + 2.0, rect.center().y);
                    let default_font_size = ui.style().text_styles.get(&egui::TextStyle::Body)
                        .map(|f| f.size)
                        .unwrap_or(14.0);
                    let small_font = egui::FontId::new(default_font_size * 0.75, egui::FontFamily::Proportional);
                    ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, text, small_font, text_color);
                    // Update tracking with the bar number we just drew (x position not needed for duplicate prevention)
                    last_bar_number_at_x = Some((bar_number, x));
                }
            }
        } else if !line_too_close {
            // Subdivision (0.1 second) - only draw if not too close
            stroke.color = step_color;
            // Alternate between step_even_y and step_odd_y for visual distinction
            let subdivision_index = ((seconds * MAX_LINES_PER_SECOND) % MAX_LINES_PER_SECOND).floor() as usize;
            let y = if subdivision_index % 2 == 0 {
                step_even_y
            } else {
                step_odd_y
            };
        let a = egui::Pos2::new(x, rect.top());
        let b = egui::Pos2::new(x, y);
        ui.painter().line_segment([a, b], stroke);
        }
        
        // Update last_x only if we actually drew a line (or it's a whole second)
        if !line_too_close || is_whole_second {
            last_x = x;
        }
        current_tick_relative += ticks_per_line;
    }

    response
}
