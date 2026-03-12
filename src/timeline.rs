use crate::{
    context::{BackgroundCtx, SetPlayhead, TimelineCtx, TracksCtx},
    grid, interaction, playhead::PlayheadApi, ruler,
};

/// The top-level timeline widget.
pub struct Timeline {
    /// A optional side panel with track headers.
    ///
    /// Can be useful for labelling tracks or providing convenient volume, mute, solo, etc style
    /// widgets.
    header: Option<f32>,
}

/// The result of setting the timeline, ready to start laying out tracks.
pub struct Show {
    tracks: TracksCtx,
    ui: egui::Ui,
    bottom_bar_rect: Option<egui::Rect>,
    top_panel_rect: Option<egui::Rect>,
    ruler_bottom: Option<f32>,
}

impl Timeline {
    /// Begin building the timeline widget.
    pub fn new() -> Self {
        Self { header: None }
    }

    /// A optional track header side panel.
    ///
    /// Can be useful for labelling tracks or providing convenient volume, mute, solo, etc style
    /// widgets.
    pub fn header(mut self, width: f32) -> Self {
        self.header = Some(width);
        self
    }

    /// Set the timeline within the currently available rect.
    pub fn show(self, ui: &mut egui::Ui, timeline: &mut dyn crate::TimelineApi) -> Show {
        // The full area including both headers and timeline.
        let full_rect = ui.available_rect_before_wrap();
        
        // Reserve 40px at the top for the top panel and 20px at the bottom for the bottom bar
        const TOP_PANEL_HEIGHT: f32 = 40.0;
        const BOTTOM_BAR_HEIGHT: f32 = 20.0;
        let mut content_rect = full_rect;
        content_rect.min.y += TOP_PANEL_HEIGHT;
        content_rect.set_height(full_rect.height() - TOP_PANEL_HEIGHT - BOTTOM_BAR_HEIGHT);
        
        // Top panel area (40px height, full width)
        let top_panel_rect = egui::Rect::from_min_max(
            egui::Pos2::new(full_rect.min.x, full_rect.min.y),
            egui::Pos2::new(full_rect.max.x, full_rect.min.y + TOP_PANEL_HEIGHT),
        );
        
        // The area occupied by the timeline (excluding top panel and bottom bar).
        let mut timeline_rect = content_rect;
        // The area occupied by track headers.
        let header_rect = self.header.map(|header_w| {
            let mut r = content_rect;
            r.set_width(header_w);
            timeline_rect.min.x = r.right();
            r
        });
        
        // Bottom bar area (20px height, full width)
        let bottom_bar_rect = egui::Rect::from_min_max(
            egui::Pos2::new(full_rect.min.x, content_rect.max.y),
            egui::Pos2::new(full_rect.max.x, full_rect.max.y),
        );

        // Draw the background.
        let vis = ui.style().noninteractive();
        let bg_stroke = egui::Stroke {
            width: 0.0,
            ..vis.bg_stroke
        };
        ui.painter().rect(full_rect, 0.0, vis.bg_fill, bg_stroke);

        // Draw top panel background (no border)
        let vis = ui.style().noninteractive();
        ui.painter().rect(top_panel_rect, 0.0, vis.bg_fill, vis.bg_stroke);
        
        // Draw a 1px green border around the entire timeline widget (including header column, top panel, and bottom bar)
        // to visualize the complete viewport
        let green_border = egui::Stroke {
            width: 1.0,
            color: egui::Color32::from_rgb(0, 255, 0),
        };
        // full_rect includes the top panel and bottom bar area, so the border will encompass everything
        ui.painter().rect_stroke(full_rect, 0.0, green_border);

        // The child widgets (content area, excluding bottom bar).
        let layout = egui::Layout::top_down(egui::Align::Min);
        let info = timeline.musical_ruler_info();
        let visible_ticks = info.ticks_per_point() * timeline_rect.width();
        let timeline_ctx = TimelineCtx::new(timeline_rect, visible_ticks);
        let tracks = TracksCtx::new(content_rect, header_rect, timeline_ctx);
        let ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect).layout(layout));
        Show { tracks, ui, bottom_bar_rect: Some(bottom_bar_rect), top_panel_rect: Some(top_panel_rect), ruler_bottom: None }
    }
}

impl Show {
    /// Allows for drawing some widgets in the background before showing the grid.
    ///
    /// Can be useful for subtly colouring different ranges, etc.
    pub fn background(mut self, background: impl FnOnce(&BackgroundCtx, &mut egui::Ui)) -> Self {
        let Show {
            ref mut ui,
            ref tracks,
            bottom_bar_rect: _,
            top_panel_rect: _,
            ruler_bottom: _,
        } = self;
        let bg = BackgroundCtx {
            header_full_rect: tracks.header_full_rect,
            timeline: &tracks.timeline,
        };
        background(&bg, ui);
        self
    }

    /// Paints the grid over the timeline `Rect`.
    ///
    /// If using a custom `background`, you may wish to call this after.
    pub fn paint_grid(mut self, info: &dyn ruler::MusicalInfo) -> Self {
        grid::paint_grid(&mut self.ui, &self.tracks.timeline, info);
        self
    }

    /// Set some tracks that should be pinned to the top.
    ///
    /// Often useful for the ruler or other tracks that should always be visible.
    pub fn pinned_tracks(mut self, tracks_fn: impl FnOnce(&TracksCtx, &mut egui::Ui)) -> Self {
        let Self {
            ref mut ui,
            ref tracks,
            bottom_bar_rect: _,
            top_panel_rect: _,
            ref mut ruler_bottom,
        } = self;

        // Reset pinned track counter before rendering pinned tracks
        tracks.reset_pinned_track_index();
        
        // Use no spacing by default so we can get exact position for line separator.
        // The ruler will add its own 4px spacing after itself (same as regular tracks)
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            tracks_fn(tracks, ui);
        });

        // Ensure no spacing between pinned tracks and regular tracks
        // Explicitly set spacing to 0 and consume any remaining space to prevent extra spacing
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().interact_size.y = 0.0;
        let rect = ui.available_rect_before_wrap();
        // FIX: ui.scope() adds 3px of spacing when it closes, so we need to subtract it
        // The last pinned track (Marker) adds 10px spacing correctly (marker bottom + 10px)
        // But after the scope, available_rect has an extra 3px from the scope
        // So we subtract 3px to get the correct position (marker bottom + 10px)
        let corrected_position = rect.top() - 3.0;
        // Store the position where the first track should start (10px below marker's bottom border)
        *ruler_bottom = Some(corrected_position);
        // Force the UI to consume all available space up to this point to prevent extra spacing
        ui.allocate_space(egui::Vec2::new(0.0, 0.0));
        self.ui.set_clip_rect(rect);
        self
    }

    /// Set all remaining tracks for the timeline.
    ///
    /// These tracks will become vertically scrollable in the case that there are two many to fit
    /// on the view. The given `egui::Rect` is the viewport (visible area) relative to the
    /// timeline.
    ///
    /// If `playhead_api` is provided, clicking and dragging on the timeline area of tracks will set the playhead position.
    /// If `selection_api` is provided, clicking and dragging on tracks will create selections.
    pub fn tracks(
        mut self,
        tracks_fn: impl FnOnce(&TracksCtx, egui::Rect, &mut egui::Ui, Option<&dyn PlayheadApi>, Option<&dyn crate::interaction::TrackSelectionApi>),
        playhead_api: Option<&dyn PlayheadApi>,
        selection_api: Option<&dyn crate::interaction::TrackSelectionApi>,
    ) -> SetPlayhead {
        let Self {
            ref mut ui,
            ref tracks,
            bottom_bar_rect,
            top_panel_rect: _,
            ref ruler_bottom,
        } = self;
        let rect = ui.available_rect_before_wrap();
        const TRACKS_BOTTOM_PADDING: f32 = 10.0;
        let tracks_viewport_height = (rect.height() - TRACKS_BOTTOM_PADDING).max(0.0);
        // Ensure no spacing between pinned tracks (ruler) and regular tracks
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.spacing_mut().interact_size.y = 0.0;
        // Remove any default window padding that might add extra space
        ui.spacing_mut().window_margin = egui::Margin::same(0.0);
        let enable_scrolling = !ui.input(|i| i.modifiers.ctrl);
        // Calculate where tracks should start: exactly 10px below marker's bottom border
        // (Marker is the last pinned track, and it adds 10px spacing)
        let tracks_start_y = ruler_bottom.as_ref().copied().unwrap_or(rect.top());
        
        let res = egui::ScrollArea::vertical()
            .max_height((rect.height() - TRACKS_BOTTOM_PADDING).max(0.0))
            .enable_scrolling(enable_scrolling)
            .animated(true)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = 0.0;
                ui.spacing_mut().window_margin = egui::Margin::same(0.0);
                let view = ui.available_rect_before_wrap();
                tracks_fn(tracks, view, ui, playhead_api, selection_api);
            });
        let timeline_rect = tracks.timeline.full_rect;

        // Clamp playhead bottom to the visible tracks viewport so it does not overflow
        // when tracks exceed the scroll area's visible height.
        let tracks_viewport_bottom = tracks_start_y + tracks_viewport_height;
        let tracks_bottom = if res.content_size.y < 1.0 {
            // No tracks: use the position where first track would start (10px below marker's bottom border)
            tracks_start_y
        } else {
            // Has tracks: calculate the actual bottom of the last track's border
            // content_size.y is the total height of all tracks (including their spacing)
            // Since the last track doesn't add spacing after itself, the bottom is at tracks_start_y + content_size.y
            // but never below the visible scroll area bottom.
            (tracks_start_y + res.content_size.y).min(tracks_viewport_bottom)
        };
        // Calculate tracks_top: where the first track starts (exactly 10px below marker's bottom border)
        let tracks_top = tracks_start_y;
        let mut set_playhead = SetPlayhead::new(timeline_rect, tracks_top, tracks_bottom);
        set_playhead.bottom_bar_rect = bottom_bar_rect;
        set_playhead.top_panel_rect = self.top_panel_rect;
        set_playhead
    }
}

impl SetPlayhead {
    /// Instantiate the playhead over the top of the whole timeline.
    pub fn playhead(
        &self,
        ui: &mut egui::Ui,
        info: &mut dyn PlayheadApi,
        playhead: crate::playhead::Playhead,
    ) -> &Self {
        crate::playhead::set(ui, info, self.timeline_rect(), self.tracks_top(), self.tracks_bottom(), playhead);
        self
    }

    /// Run scroll/zoom after track layout so vertical scroll goes to ScrollArea first.
    pub fn run_scroll_and_zoom(&self, ui: &mut egui::Ui, timeline: &mut dyn crate::TimelineApi) -> &Self {
        interaction::handle_scroll_and_zoom(ui, self.timeline_rect(), timeline);
        self
    }

    /// Return the main panel name at a given pointer position.
    pub fn panel_name_at_pos(&self, pos: egui::Pos2) -> Option<&'static str> {
        if let Some(top_rect) = self.top_panel_rect {
            if top_rect.contains(pos) {
                return Some("Top Panel");
            }
        }
        if let Some(bottom_rect) = self.bottom_bar_rect {
            if bottom_rect.contains(pos) {
                return Some("Bottom Bar (Global)");
            }
        }

        let content_top = self
            .top_panel_rect
            .map(|r| r.max.y)
            .unwrap_or(self.timeline_rect().min.y);
        let content_bottom = self
            .bottom_bar_rect
            .map(|r| r.min.y)
            .unwrap_or(self.timeline_rect().max.y);

        if pos.y < content_top || pos.y > content_bottom {
            return None;
        }

        if pos.x < self.timeline_rect().min.x {
            if pos.y < self.tracks_top() {
                Some("Header (Ruler/Marker)")
            } else {
                Some("Track Header")
            }
        } else if pos.x <= self.timeline_rect().max.x {
            if pos.y < self.tracks_top() {
                Some("Ruler/Marker")
            } else if pos.y <= self.tracks_bottom() {
                Some("Tracks")
            } else {
                Some("Timeline Area Below Tracks")
            }
        } else {
            None
        }
    }

    /// Display time in the top panel.
    /// 
    /// `playhead_api` should provide access to the current playhead position.
    /// Show the time in the top panel.
    /// `playhead_api` should provide access to the current playhead position.
    /// `get_is_playing` closure returns the current play state.
    /// `set_is_playing` closure sets the play state.
    /// `track_count` should be the number of tracks (excluding ruler).
    /// `max_playhead_pos` is the maximum absolute playhead position (end of timeline).
    /// `add_track_callback` closure is called when "Add Track" button is clicked.
    /// `remove_track_callback` closure is called when "Remove Track" button is clicked.
    /// `add_block_callback` closure is called when "Add Block" button is clicked.
    /// `has_selected_track` closure returns whether a track is currently selected.
    pub fn top_panel_time(
        &self,
        ui: &mut egui::Ui,
        playhead_api: Option<&dyn crate::playhead::PlayheadApi>,
        get_is_playing: impl Fn() -> bool,
        mut set_is_playing: impl FnMut(bool),
        track_count: usize,
        max_playhead_pos: f32,
        mut add_track_callback: impl FnMut(),
        mut remove_track_callback: impl FnMut(),
        mut add_block_callback: impl FnMut(),
        has_selected_track: impl Fn() -> bool,
        get_total_seconds: impl Fn() -> u32,
        mut set_total_seconds: impl FnMut(u32),
    ) -> &Self {
        if let Some(top_panel_rect) = self.top_panel_rect {
            // Create UI for top panel to display time
            let mut top_panel_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(top_panel_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            
            // Add 2px top padding
            top_panel_ui.add_space(2.0);
            
            // Layout: buttons on left, time on right
            top_panel_ui.horizontal(|ui| {
                // Left side: Play and Stop buttons
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.add_space(4.0); // Left padding
                    
                    ui.vertical(|ui| {
                        // First line: Play, Stop, navigation, and Add Track buttons (horizontal)
                        ui.horizontal(|ui| {
                            let is_playing = get_is_playing();
                            
                            // Play button
                            if ui.selectable_label(is_playing, "Play").clicked() {
                                set_is_playing(true);
                            }
                            
                            ui.add_space(4.0); // Spacing between buttons
                            
                            // Stop button
                            if ui.selectable_label(!is_playing, "Stop").clicked() {
                                set_is_playing(false);
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // "<" button - set playhead to start (position 0)
                            if ui.button("<").clicked() {
                                if let Some(api) = playhead_api {
                                    // Get current timeline_start (scroll offset) from the API
                                    let timeline_start = api.timeline_start().unwrap_or(0.0);
                                    // Calculate relative ticks to set absolute position to 0
                                    // new_pos = timeline_start + ticks = 0, so ticks = -timeline_start
                                    let ticks = -timeline_start;
                                    if ticks.is_finite() {
                                        api.set_playhead_ticks(ticks);
                                    }
                                }
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // ">" button - set playhead to end (maximum position)
                            if ui.button(">").clicked() {
                                if let Some(api) = playhead_api {
                                    // Get current timeline_start (scroll offset) from the API
                                    let timeline_start = api.timeline_start().unwrap_or(0.0);
                                    // Calculate relative ticks to set absolute position to max_playhead_pos
                                    // new_pos = timeline_start + ticks = max_playhead_pos, so ticks = max_playhead_pos - timeline_start
                                    let ticks = max_playhead_pos - timeline_start;
                                    if ticks.is_finite() {
                                        api.set_playhead_ticks(ticks);
                                    }
                                }
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // Add Track button - shows track count
                            if ui.button(format!("Add Track ({})", track_count)).clicked() {
                                add_track_callback();
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // Remove Track button - only enabled when a track is selected
                            let has_selection = has_selected_track();
                            if ui.add_enabled(has_selection, egui::Button::new("Remove Track")).clicked() {
                                remove_track_callback();
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // Add Block button - only enabled when a track is selected
                            if ui.add_enabled(has_selection, egui::Button::new("Add Block")).clicked() {
                                add_block_callback();
                            }
                            
                            ui.add_space(4.0); // Spacing
                            
                            // Total seconds input - integer input with minimum of 16
                            let mut total_secs = get_total_seconds() as i32;
                            let response = ui.add(
                                egui::DragValue::new(&mut total_secs)
                                    .range(16..=i32::MAX)
                                    .speed(1.0)
                                    .prefix("Total: ")
                                    .suffix("s")
                            );
                            if response.changed() {
                                // Ensure minimum of 16 seconds
                                let new_value = total_secs.max(16) as u32;
                                set_total_seconds(new_value);
                            }
                        });

                    });
                });
                
                // Right side: Time display
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(api) = playhead_api {
                    // Get current playhead position in ticks (absolute from timeline start at 0)
                    // playhead_ticks() returns relative ticks from timeline start (scroll position)
                    // We need the absolute position from the beginning of the timeline (tick 0)
                    let playhead_ticks_relative = api.playhead_ticks();
                    // Get timeline_start from MusicalInfo trait (this is the scroll offset)
                    let timeline_start = api.timeline_start().unwrap_or(0.0);
                    // Absolute playhead position from the beginning of the timeline
                    let absolute_playhead_ticks = timeline_start + playhead_ticks_relative;
                    
                    // Convert ticks to time based on bars
                    // Each bar should be 1 second, so calculate which bar we're in and the fraction within that bar
                    let ticks_per_beat = api.ticks_per_beat() as f32;
                    // 4/4 time signature = 4 beats per bar
                    const BEATS_PER_BAR: f32 = 4.0;
                    let ticks_per_bar = ticks_per_beat * BEATS_PER_BAR;
                    
                    // Calculate which bar we're in and the fraction within that bar
                    let bar_number = absolute_playhead_ticks / ticks_per_bar;
                    let total_seconds = bar_number; // Each bar = 1 second
                    
                    let minutes = (total_seconds / 60.0).floor() as u32;
                    let seconds = (total_seconds % 60.0).floor() as u32;
                    let centiseconds = ((total_seconds % 1.0) * 100.0).floor() as u32;
                    
                    let time_string = format!("{:02}:{:02}:{:02}", minutes, seconds, centiseconds);
                    ui.label(time_string);
                } else {
                    // Fallback if no playhead API
                    ui.label("00:00:00");
                }
                });
            });
        }
        self
    }

    /// Show the bottom bar with global buttons.
    /// 
    /// `global_panel_visible` should be a mutable reference to a bool that tracks
    /// whether the global panel is visible. It will be toggled when the "Global" button is clicked.
    pub fn bottom_bar(&self, ui: &mut egui::Ui, global_panel_visible: &mut bool) {
        if let Some(bottom_bar_rect) = self.bottom_bar_rect {
            // Get style before creating child UI
            let vis = ui.style().noninteractive();
            let bg_fill = vis.bg_fill;
            let bg_stroke = vis.bg_stroke;
            
            // Draw bottom bar background
            ui.painter().rect(bottom_bar_rect, 0.0, bg_fill, bg_stroke);
            
            // Create UI for bottom bar
            let mut bottom_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(bottom_bar_rect)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            
            // Add "Global" button
            if bottom_ui.button("Global").clicked() {
                *global_panel_visible = !*global_panel_visible;
            }
            
            // Draw global panel if visible (100px height, above everything)
            if *global_panel_visible {
                const PANEL_HEIGHT: f32 = 200.0;
                let panel_rect = egui::Rect::from_min_max(
                    egui::Pos2::new(bottom_bar_rect.min.x, bottom_bar_rect.min.y - PANEL_HEIGHT),
                    egui::Pos2::new(bottom_bar_rect.max.x, bottom_bar_rect.min.y),
                );
                
                // Draw panel background
                ui.painter().rect(panel_rect, 0.0, bg_fill, bg_stroke);
                
                // Create UI for panel (using a new child to ensure it's above everything)
                let mut panel_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(panel_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                
                // Divide panel into 4 columns
                panel_ui.horizontal(|ui| {
                    // Column 1: "Global Panel" label
                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.set_width((panel_rect.width() / 4.0 - 4.0).max(0.0));
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Keyboard:").size(12.0));
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Space: Play/Pause").size(12.0));
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Left Mouse: Move Playhead/Select Track").size(12.0));
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Shift + Mouse Wheel: Zoom Left/Right").size(12.0));
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Ctrl + Mouse Wheel: Zoom In/Out").size(12.0));
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Mouse Wheel: Scroll Up/Down").size(12.0));
                        });
                    });
                    
                    // Column 2: Available for widgets
                    ui.vertical(|ui| {
                        ui.set_width(panel_rect.width() / 4.0);
                        // Add widgets here
                    });
                    
                    // Column 3: Available for widgets
                    ui.vertical(|ui| {
                        ui.set_width(panel_rect.width() / 4.0);
                        // Add widgets here
                    });
                    
                    // Column 4: Available for widgets
                    ui.vertical(|ui| {
                        ui.set_width(panel_rect.width() / 4.0);
                        // Add widgets here
                    });
                    
                   
                });
                
                // Draw 1px grey vertical borders between columns (100% height)
                let grey_border = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::from_rgb(100, 100, 100), // Grey
                };
                let column_width = panel_rect.width() / 4.0;
                for i in 1..6 {
                    let x = panel_rect.min.x + (column_width * i as f32);
                    let top = egui::Pos2::new(x, panel_rect.min.y);
                    let bottom = egui::Pos2::new(x, panel_rect.max.y);
                    ui.painter().line_segment([top, bottom], grey_border);
                }
            }
        }
    }
}
