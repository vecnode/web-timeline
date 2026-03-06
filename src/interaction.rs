use crate::{context::TracksCtx, playhead::PlayheadApi};

/// Handle scroll and zoom interactions for the timeline.
pub fn handle_scroll_and_zoom(
    ui: &mut egui::Ui,
    timeline_rect: egui::Rect,
    timeline_api: &mut dyn crate::TimelineApi,
) {
    if ui.rect_contains_pointer(timeline_rect) {
        let ctrl_pressed = ui.input(|i| i.modifiers.ctrl);
        let shift_pressed = ui.input(|i| i.modifiers.shift);
        let smooth_delta = ui.input(|i| i.smooth_scroll_delta);
        let raw_delta = ui.input(|i| i.raw_scroll_delta);
        // When Ctrl is pressed, prefer raw_delta for more immediate response
        // Otherwise, prefer smooth_delta for better UX
        let delta = if ctrl_pressed {
            if raw_delta != egui::Vec2::ZERO {
                raw_delta
            } else {
                smooth_delta
            }
        } else {
            if smooth_delta != egui::Vec2::ZERO {
                smooth_delta
            } else {
                raw_delta
            }
        };
        if ctrl_pressed {
            if delta.x != 0.0 || delta.y != 0.0 {
                timeline_api.zoom(delta.y - delta.x);
            }
        } else if shift_pressed || delta.x != 0.0 {
            // Handle horizontal scrolling (with or without shift modifier)
            if delta.x != 0.0 {
                let ticks_per_point = timeline_api.musical_ruler_info().ticks_per_point();
                let timeline_width = timeline_rect.width();
                let visible_ticks = ticks_per_point * timeline_width;
                
                // Get the maximum absolute tick position (end of timeline)
                let max_absolute_tick = timeline_api.musical_ruler_info().max_absolute_tick();
                
                // Calculate the maximum timeline_start so that the last second is visible at the right edge
                // Only allow scrolling to the right if the timeline overflows the visible area
                let max_timeline_start = if let Some(max_tick) = max_absolute_tick {
                    // Only allow scrolling to the right if timeline overflows visible area
                    if max_tick > visible_ticks {
                        (max_tick - visible_ticks).max(0.0)
                    } else {
                        // Timeline fits in visible area - don't allow scrolling to the right
                        0.0
                    }
                } else {
                    // No maximum defined - allow unlimited scrolling (fallback behavior)
                    f32::INFINITY
                };
                
                let shift_amount = delta.x * ticks_per_point;
                let current_start = timeline_api.timeline_start();
                let mut new_start = current_start + shift_amount;
                
                // Clamp to prevent scrolling past boundaries
                new_start = new_start.max(0.0);
                if new_start > max_timeline_start {
                    new_start = max_timeline_start;
                }
                
                if (new_start - current_start).abs() > 0.001 {
                    timeline_api.shift_timeline_start(new_start - current_start);
                }
            }
        }
    }
}

/// Handle clicks and drags on timeline area to set playhead.
pub fn handle_track_playhead_interaction(
    ui: &mut egui::Ui,
    tracks: &TracksCtx,
    playhead_api: Option<&dyn PlayheadApi>,
) {
    if let Some(api) = playhead_api {
        let timeline_rect = tracks.timeline.full_rect;
        let timeline_w = timeline_rect.width();
        let ticks_per_point = api.ticks_per_point();
        let visible_ticks = ticks_per_point * timeline_w;

        // Check input state without allocating space (to avoid layout issues)
        let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
        let pointer_down = ui.input(|i| i.pointer.primary_down());
        let pointer_pos = ui.input(|i| i.pointer.interact_pos());
        let pointer_over = pointer_pos
            .map(|pos| timeline_rect.contains(pos))
            .unwrap_or(false);

        // Handle both initial click and drag
        if (pointer_pressed && pointer_over) || (pointer_down && pointer_over) {
            if let Some(pt) = pointer_pos {
                let tick = (((pt.x - timeline_rect.min.x) / timeline_w) * visible_ticks).max(0.0);
                api.set_playhead_ticks(tick);
            }
        }
    }
}

/// Handle clicks and drags on a specific track for selection and playhead.
pub fn handle_track_interaction(
    ui: &mut egui::Ui,
    track_rect: egui::Rect, // The actual track area (for pointer detection)
    timeline_rect: egui::Rect, // The full timeline area (for tick calculation)
    track_id: &str,
    playhead_api: Option<&dyn PlayheadApi>,
    selection_api: Option<&dyn TrackSelectionApi>,
) {
    let timeline_w = timeline_rect.width();
    
    let ticks_per_point = if let Some(ref api) = playhead_api {
        api.ticks_per_point()
    } else if let Some(ref api) = selection_api {
        api.ticks_per_point()
    } else {
        return;
    };
    
    let visible_ticks = ticks_per_point * timeline_w;

    let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
    let pointer_released = ui.input(|i| i.pointer.primary_released());
    let pointer_down = ui.input(|i| i.pointer.primary_down());
    let secondary_pressed = ui.input(|i| i.pointer.secondary_pressed());
    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
    // Check if pointer is over the actual track area (not the full timeline)
    let pointer_over_track = pointer_pos
        .map(|pos| track_rect.contains(pos))
        .unwrap_or(false);
    // Check if pointer is over the timeline area (for right-click deselection)
    let pointer_over_timeline = pointer_pos
        .map(|pos| timeline_rect.contains(pos))
        .unwrap_or(false);

    // Check if we're currently dragging on this track
    let is_dragging_this_track = if let Some(api) = selection_api {
        if let Some((drag_track_id, _)) = api.get_drag_start() {
            drag_track_id == track_id
        } else {
            false
        }
    } else {
        false
    };

    if let Some(pt) = pointer_pos {
        // Calculate tick based on position in timeline (not track)
        let tick = (((pt.x - timeline_rect.min.x) / timeline_w) * visible_ticks).max(0.0);

        // Handle playhead (update on click, but not during block drag)
        if let Some(api) = playhead_api {
            // Check if a block is being dragged - if so, don't update playhead during drag
            let is_dragging_block = if let Some(selection_api) = selection_api {
                selection_api.is_dragging_block()
            } else {
                false
            };
            
            // Only update playhead on initial click (pointer_pressed), not during drag (pointer_down)
            // when a block is being dragged. Normal track clicks/drags still update playhead.
            if pointer_pressed && pointer_over_track && !secondary_pressed {
                api.set_playhead_ticks(tick);
            } else if pointer_down && pointer_over_track && !secondary_pressed && !is_dragging_block {
                // Update playhead during drag only if not dragging a block
                api.set_playhead_ticks(tick);
            }
        }

        // Handle selection
        if let Some(api) = selection_api {
            // Right mouse button click - deselect all tracks (works anywhere in timeline area)
            if secondary_pressed && pointer_over_timeline {
                api.clear_all_selections();
            } else if pointer_pressed && pointer_over_track && !secondary_pressed {
                // Check if clicking on a block - if so, don't start track selection
                let is_on_block = if let Some(pos) = pointer_pos {
                    api.is_click_on_block(track_id, pos, timeline_rect)
                } else {
                    false
                };
                
                // Start drag - ONLY if click is inside the track area AND not on a block
                if !is_on_block {
                    // Clear all previous selections first, then store absolute start position
                    api.clear_all_selections();
                    let timeline_start = api.timeline_start();
                    let absolute_start_tick = timeline_start + tick;
                    api.start_selection_drag(track_id, absolute_start_tick);
                }
            } else if pointer_down && is_dragging_this_track && !secondary_pressed {
                // Continue drag - allow dragging even if pointer goes outside track
                // Update end position (absolute) - clamp tick to valid range
                let timeline_start = api.timeline_start();
                let clamped_tick = tick.max(0.0).min(visible_ticks);
                let absolute_end_tick = timeline_start + clamped_tick;
                api.update_selection_drag(track_id, absolute_end_tick);
            } else if pointer_released {
                // End drag - check if it was a click or drag
                if is_dragging_this_track {
                    if let Some((_, absolute_start_tick)) = api.get_drag_start() {
                        let timeline_start = api.timeline_start();
                        // Use current tick position, clamped to valid range
                        let clamped_tick = if pointer_over_timeline { tick } else {
                            // If released outside timeline, use the last valid position
                            (absolute_start_tick - timeline_start).max(0.0).min(visible_ticks)
                        };
                        let absolute_end_tick = timeline_start + clamped_tick.max(0.0).min(visible_ticks);
                        let drag_distance = (absolute_end_tick - absolute_start_tick).abs();
                        if drag_distance < 1.0 {
                            // Click (no significant drag) - clear all selections
                            api.clear_all_selections();
                        } else {
                            // Drag - set selection (absolute ticks) on this track
                            // Clear all first to ensure only one selection exists
                            api.clear_all_selections();
                            api.set_selection(track_id, absolute_start_tick.min(absolute_end_tick), absolute_start_tick.max(absolute_end_tick));
                        }
                        api.end_selection_drag();
                    }
                }
            }
        }
    }
}

/// API for track selection functionality.
pub trait TrackSelectionApi {
    fn ticks_per_point(&self) -> f32;
    fn timeline_start(&self) -> f32;
    fn is_click_on_block(&self, track_id: &str, pos: egui::Pos2, timeline_rect: egui::Rect) -> bool;
    fn is_dragging_block(&self) -> bool;
    fn start_selection_drag(&self, track_id: &str, start_tick: f32);
    fn update_selection_drag(&self, track_id: &str, end_tick: f32);
    fn get_drag_start(&self) -> Option<(String, f32)>;
    fn end_selection_drag(&self);
    fn set_selection(&self, track_id: &str, start_tick: f32, end_tick: f32);
    fn clear_selection(&self, track_id: &str);
    fn clear_all_selections(&self);
    fn get_selection(&self, track_id: &str) -> Option<(f32, f32)>;
    fn get_selected_track_id(&self) -> Option<String>;
}
