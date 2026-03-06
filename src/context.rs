use egui::Rect;

/// A context for instantiating tracks, either pinned or unpinned.
pub struct TracksCtx {
    /// The rectangle encompassing the entire widget area including both header and timeline and
    /// both pinned and unpinned track areas.
    pub full_rect: Rect,
    /// The rect encompassing the left-hand-side track headers including pinned and unpinned.
    pub header_full_rect: Option<Rect>,
    /// Context specific to the timeline (non-header) area.
    pub timeline: TimelineCtx,
    /// The first track's selection rect (for redrawing on top after border).
    pub(crate) first_track_selection_rect: std::cell::RefCell<Option<Rect>>,
}

/// Some context for the timeline, providing short-hand for setting some useful widgets.
pub struct TimelineCtx {
    /// The total visible rect of the timeline area including pinned and unpinned tracks.
    pub full_rect: Rect,
    /// The total number of ticks visible on the timeline area.
    pub visible_ticks: f32,
}

/// A type used to assist with setting a track with an optional `header`.
pub struct TrackCtx<'a> {
    tracks: &'a TracksCtx,
    ui: &'a mut egui::Ui,
    available_rect: Rect,
    header_height: f32,
    track_id: Option<String>,
    is_first_track: bool,
    is_last_track: bool,
}

/// Context for instantiating the playhead after all tracks have been set.
pub struct SetPlayhead {
    timeline_rect: Rect,
    /// The y position at the top of the first track (after ruler + spacing).
    tracks_top: f32,
    /// The y position at the bottom of the last track, or the bottom of the
    /// tracks' scrollable area in the case that the size of the tracks
    /// exceed the visible height.
    tracks_bottom: f32,
    /// The y position where the first track's top border should be drawn (for redrawing on top).
    first_track_top_border_y: Option<f32>,
    /// The full rect including header area (for redrawing border at full width).
    pub(crate) full_rect: Option<Rect>,
    /// The first track's selection rect (for redrawing on top after border).
    pub(crate) first_track_selection_rect: Option<Rect>,
    /// The bottom bar rectangle (20px height at the bottom).
    pub(crate) bottom_bar_rect: Option<Rect>,
    /// The top panel rectangle (40px height at the top).
    pub(crate) top_panel_rect: Option<Rect>,
}

/// Relevant information for displaying a background for the timeline.
pub struct BackgroundCtx<'a> {
    pub header_full_rect: Option<Rect>,
    pub timeline: &'a TimelineCtx,
}

impl TracksCtx {
    /// Begin showing the next `Track`.
    pub fn next<'a>(&'a self, ui: &'a mut egui::Ui) -> TrackCtx<'a> {
        let available_rect = ui.available_rect_before_wrap();
        TrackCtx {
            tracks: self,
            ui,
            available_rect,
            header_height: 0.0,
            track_id: None,
            is_first_track: false,
            is_last_track: false,
        }
    }
}

impl<'a> TrackCtx<'a> {
    /// Set the track identifier for selection tracking.
    pub fn with_id(mut self, track_id: impl Into<String>) -> Self {
        self.track_id = Some(track_id.into());
        self
    }
    
    /// Mark this track as the first regular track (after ruler)
    pub fn mark_first_track(mut self) -> Self {
        self.is_first_track = true;
        self
    }
    
    /// Mark this track as the last regular track
    pub fn mark_last_track(mut self) -> Self {
        self.is_last_track = true;
        self
    }

    /// UI for the track's header.
    ///
    /// The header content (text, buttons, etc.) is automatically padded 4px from the left edge
    /// to provide consistent spacing for track labels and controls like mute/solo buttons.
    ///
    /// NOTE: Both the ruler (pinned track) and regular tracks use the same `header_full_rect`
    /// from `TracksCtx`, ensuring they always have the same width. The border is drawn at
    /// `header_full_rect.max.x` for both, guaranteeing alignment.
    pub fn header(mut self, header: impl FnOnce(&mut egui::Ui)) -> Self {
        const LEFT_PADDING: f32 = 4.0;
        let header_h = self
            .tracks
            .header_full_rect
            .map(|mut rect| {
                // IMPORTANT: Both ruler and tracks use the same header_full_rect, so they have the same width
                // The rect.max.x (header_right_x) is the same for both, ensuring the grey border aligns perfectly
                
                // FIX: For the first track, compensate for the 2px spacer added in ScrollArea
                // The spacer is needed for track content (border visibility) but shouldn't affect header alignment
                // Subtract 2px from header's starting position for first track only
                let header_start_y = if self.is_first_track {
                    self.available_rect.min.y - 2.0
                } else {
                    self.available_rect.min.y
                };
                rect.min.y = header_start_y;
                // Constrain header height to available rect to prevent overlap with next track
                rect.max.y = rect.min.y.min(self.available_rect.max.y);
                
                // Fill the header area with background FIRST (before content) to prevent grid lines from showing through
                // This ensures the background is behind the widgets
                let vis = self.ui.style().noninteractive();
                
                // Store original header rect boundaries before modifying rect
                // This ensures the border is drawn at the same x position for both ruler and tracks
                let header_right_x = rect.max.x; // Right edge of header (where grey border will be drawn)
                
                // FIX: For the ruler (pinned track), constrain header background to only the header area
                // The ruler's header should only cover its own header, not extend into the track content or beyond
                // Track 1 starts at ruler bottom (111.00) + 4px spacing = 115.00, so we must stop well before that
                let header_fill_max_y = if self.track_id.is_none() {
                    // Ruler: only cover the header area itself (typically ~20-24px)
                    // Use rect.max.y which is the header rect's bottom, not the full available_rect
                    rect.max.y
                } else {
                    // Regular tracks: can use full available height (they're in ScrollArea)
                    self.available_rect.max.y
                };
                
                // FIX: Header background should extend to the right border (header_right_x) before grid lines start
                // This ensures the gray rectangle goes all the way to the grey vertical border
                let header_fill_rect = egui::Rect::from_min_max(
                    egui::Pos2::new(rect.min.x, rect.min.y),
                    egui::Pos2::new(header_right_x, header_fill_max_y),
                );
                
                self.ui.painter().rect_filled(header_fill_rect, 0.0, vis.bg_fill);
                
                // Add 4px left padding by adjusting the rect
                rect.min.x += LEFT_PADDING;
                let ui = &mut self.ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect)
                        .layout(*self.ui.layout()),
                );
                header(ui);
                ui.min_rect().height()
            })
            .unwrap_or(0.0);
        self.header_height = header_h;
        self
    }

    /// Set the track, with a function for instantiating contents for the timeline.
    /// `on_track_click` is called when the full track area (header + content) is clicked.
    pub fn show(
        self,
        track: impl FnOnce(&TimelineCtx, &mut egui::Ui),
        playhead_api: Option<&dyn crate::playhead::PlayheadApi>,
        selection_api: Option<&dyn crate::interaction::TrackSelectionApi>,
        on_track_click: Option<impl FnOnce(String)>,
        is_selected: bool,
    ) {
        // The UI and area for the track timeline.
        let track_timeline_rect = {
            let mut rect = self.tracks.timeline.full_rect;
            rect.min.y = self.available_rect.min.y;
            rect
        };
        
        // Draw selection overlay BEFORE track content so blocks appear on top (higher z-order)
        // Use estimated full track rect - overlay will cover full area, blocks drawn later will appear on top
        if is_selected {
            let selection_overlay = egui::Color32::from_rgba_unmultiplied(128, 128, 128, 5);
            // Estimate full track height (header + minimum content height)
            let estimated_track_h = 40.0; // Minimum track height
            let estimated_full_track_height = self.header_height.max(estimated_track_h);
            let estimated_full_track_rect = egui::Rect::from_min_max(
                egui::Pos2::new(
                    self.tracks.full_rect.min.x, // Left edge (includes header)
                    self.available_rect.min.y,    // Top of this track
                ),
                egui::Pos2::new(
                    self.tracks.full_rect.max.x,              // Right edge (full width)
                    self.available_rect.min.y + estimated_full_track_height, // Bottom of this track
                ),
            );
            self.ui.painter().rect_filled(estimated_full_track_rect, 0.0, selection_overlay);
        }
        
        let track_h = {
            let ui = &mut self.ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(track_timeline_rect)
                    .layout(*self.ui.layout()),
            );
            track(&self.tracks.timeline, ui);
            ui.min_rect().height()
        };
        
        // Calculate the actual track area (only the height of this track, not the full timeline)
        let actual_track_rect = {
            let mut rect = track_timeline_rect;
            rect.max.y = track_timeline_rect.min.y + track_h;
            rect
        };
        
        // Calculate the full track rect (header + timeline, 100% width) - calculate once and reuse
        // This rect is ONLY for the track content, NOT including spacing
        let full_track_height = self.header_height.max(track_h);
        let full_track_rect = egui::Rect::from_min_max(
            egui::Pos2::new(
                self.tracks.full_rect.min.x, // Left edge (includes header)
                self.available_rect.min.y,    // Top of this track
            ),
            egui::Pos2::new(
                self.tracks.full_rect.max.x,              // Right edge (full width)
                self.available_rect.min.y + full_track_height, // Bottom of this track (NOT including spacing)
            ),
        );
        
        // Handle interaction for this track
        if let Some(track_id) = &self.track_id {
            // Get selection data before calling handle_track_interaction (which takes ownership)
            // Check if this track has the selection (only one selection exists across all tracks)
            let selection_data = selection_api.as_ref().and_then(|api| {
                if api.get_selected_track_id().as_ref() == Some(track_id) {
                    api.get_selection(track_id)
                } else {
                    None
                }
            });
            let ticks_per_point_for_selection = selection_api.as_ref().map(|api| api.ticks_per_point());
            
            crate::interaction::handle_track_interaction(
                self.ui,
                actual_track_rect,
                track_timeline_rect, // Pass full timeline rect for tick calculation
                track_id,
                playhead_api,
                selection_api,
            );
            
            // Draw selection if it exists on this track (now that we have full_track_rect)
            if let (Some((absolute_start_tick, absolute_end_tick)), Some(ticks_per_point)) = (selection_data, ticks_per_point_for_selection) {
                let timeline_w = track_timeline_rect.width();
                let visible_ticks = ticks_per_point * timeline_w;
                let timeline_start = selection_api.as_ref().map(|api| api.timeline_start()).unwrap_or(0.0);
                
                // Convert absolute ticks to relative ticks for drawing
                let relative_start_tick = absolute_start_tick - timeline_start;
                let relative_end_tick = absolute_end_tick - timeline_start;
                
                // Only draw if selection is visible in current viewport
                if relative_end_tick >= 0.0 && relative_start_tick <= visible_ticks {
                    let start_x = track_timeline_rect.min.x
                        + (relative_start_tick.max(0.0) / visible_ticks) * timeline_w;
                    let end_x = track_timeline_rect.min.x
                        + (relative_end_tick.min(visible_ticks) / visible_ticks) * timeline_w;
                    
                    // Selection should match exactly from top border to bottom border
                    // Top border is at full_track_rect.min.y, bottom border is at full_track_rect.max.y
                    // But selection only spans the timeline area (not header), so use timeline x coordinates
                    // FIX: For first track, pull selection up by 1px to align properly
                    let selection_top = if self.is_first_track {
                        full_track_rect.min.y - 1.0
                    } else {
                        full_track_rect.min.y
                    };
                    let selection_rect = egui::Rect::from_min_max(
                        egui::Pos2::new(start_x.min(end_x), selection_top),
                        egui::Pos2::new(start_x.max(end_x), full_track_rect.max.y),
                    );
                    
                    // FIX: For first track, don't draw the selection here - only store it for redraw
                    // This prevents drawing it twice (once here, once in redraw)
                    if self.is_first_track {
                        *self.tracks.first_track_selection_rect.borrow_mut() = Some(selection_rect);
                    } else {
                        // For other tracks, draw normally
                        let selection_fill = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 100);
                        self.ui.painter().rect_filled(selection_rect, 0.0, selection_fill);
                    }
                }
            }
        }
        
        // Handle track selection click (on full track area, 100% width and height)
        if let Some(track_id) = &self.track_id {
            if let Some(on_click) = on_track_click {
                // Check if pointer clicked on the full track area
                let pointer_pos = self.ui.input(|i| i.pointer.interact_pos());
                let pointer_pressed = self.ui.input(|i| i.pointer.primary_pressed());
                
                if pointer_pressed {
                    if let Some(pos) = pointer_pos {
                        if full_track_rect.contains(pos) {
                            // Select track on any click within the full track area (header + content)
                            // This includes the input string area and the timeline content area
                            on_click(track_id.clone());
                        }
                    }
                }
            }
        }
        
        // Draw a pink border around the track ONLY (not including spacing)
        // For the ruler (track_id is None), draw full border. For regular tracks, draw borders around track content only.
        let pink_border = egui::Stroke {
            width: 1.0,
            color: egui::Color32::from_rgb(255, 192, 203), // Pink
        };
        
        if self.track_id.is_none() {
            // Ruler: draw complete border (all 4 sides)
            // There's 4px spacing after ruler, so bottom border will be separate from Track 1's top border
            let left_top = egui::Pos2::new(full_track_rect.min.x, full_track_rect.min.y);
            let right_top = egui::Pos2::new(full_track_rect.max.x, full_track_rect.min.y);
            let left_bottom = egui::Pos2::new(full_track_rect.min.x, full_track_rect.max.y);
            let right_bottom = egui::Pos2::new(full_track_rect.max.x, full_track_rect.max.y);
            
            // Top border
            self.ui.painter().line_segment([left_top, right_top], pink_border);
            // Left border (pink border at left edge)
            self.ui.painter().line_segment([left_top, left_bottom], pink_border);
            // Right border
            self.ui.painter().line_segment([right_top, right_bottom], pink_border);
            // Bottom border (separated by 4px spacing from Track 1's top border)
            self.ui.painter().line_segment([left_bottom, right_bottom], pink_border);
            
            // Draw left grey border for the ruler header area to match the header's right border position
            if let Some(header_rect) = self.tracks.header_full_rect {
                let header_right_x = header_rect.max.x;
                let ruler_header_border_top = egui::Pos2::new(header_right_x, full_track_rect.min.y);
                let ruler_header_border_bottom = egui::Pos2::new(header_right_x, full_track_rect.max.y);
                // Use grey border to match the header divider
                let header_border = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::from_rgb(128, 128, 128), // Grey
                };
                self.ui.painter().line_segment([ruler_header_border_top, ruler_header_border_bottom], header_border);
            }
        } else {
            // Regular tracks: draw complete borders around track content (all 4 sides)
            // Since we have 4px spacing between tracks, each track gets its own complete border
            let left_top = egui::Pos2::new(full_track_rect.min.x, full_track_rect.min.y);
            let right_top = egui::Pos2::new(full_track_rect.max.x, full_track_rect.min.y);
            let left_bottom = egui::Pos2::new(full_track_rect.min.x, full_track_rect.max.y);
            let right_bottom = egui::Pos2::new(full_track_rect.max.x, full_track_rect.max.y);
            
            
            // Draw borders in order: left, right, bottom, then top LAST to ensure it's on top of everything
            // This is especially important for Track 1's top border which might be affected by grid lines
            // Left border
            self.ui.painter().line_segment([left_top, left_bottom], pink_border);
            // Right border
            self.ui.painter().line_segment([right_top, right_bottom], pink_border);
            // Bottom border (at the bottom of the track, before spacing)
            self.ui.painter().line_segment([left_bottom, right_bottom], pink_border);
            // Top border: ensure it's drawn with pixel-perfect alignment and full visibility
            // FIX: For first track, don't draw the top border here - only store it for redraw
            // This prevents drawing it twice (once here, once in redraw)
            if !self.is_first_track {
                // Draw it slightly inside the rect (0.5px) to ensure it's not clipped and appears the same as other borders
                let top_border_y = full_track_rect.min.y + 0.5;
                let top_left = egui::Pos2::new(full_track_rect.min.x, top_border_y);
                let top_right = egui::Pos2::new(full_track_rect.max.x, top_border_y);
                self.ui.painter().line_segment([top_left, top_right], pink_border);
            }
            
            // Draw right border for the header area to separate it from the timeline/grid
            if let Some(header_rect) = self.tracks.header_full_rect {
                let header_right_x = header_rect.max.x;
                let header_border_top = egui::Pos2::new(header_right_x, full_track_rect.min.y);
                let header_border_bottom = egui::Pos2::new(header_right_x, full_track_rect.max.y);
                // Use grey border for the header divider to differentiate from track borders
                let header_border = egui::Stroke {
                    width: 1.0,
                    color: egui::Color32::from_rgb(128, 128, 128), // Grey
                };
                self.ui.painter().line_segment([header_border_top, header_border_bottom], header_border);
            }
        }
        
        // Manually add space occuppied by the child UIs, otherwise `ScrollArea` won't consider the
        // space occuppied. The spacing is added AFTER the border is drawn, so borders are tight around tracks.
        let w = self.tracks.full_rect.width();
        let h = full_track_height;
        // Add 4px spacing after track (except for last track)
        // Ruler also adds spacing so Track 1's available_rect is correctly positioned
        // This spacing is separate from the track rect, so borders don't include it
        // For the ruler (track_id is None), ensure spacing is exactly 4px to match track spacing
        let spacing_after = if self.is_last_track { 0.0 } else { 4.0 };
        // Add spacing directly to parent UI (not in scope) to ensure it's properly consumed
        // This ensures the next track's available_rect is correctly positioned
        self.ui.spacing_mut().item_spacing.y = 0.0;
        self.ui.spacing_mut().interact_size.y = 0.0;
        self.ui.horizontal(|ui| ui.add_space(w));
        // For ruler, ensure we add exactly the track height + 4px spacing (same as regular tracks)
        self.ui.add_space(h + spacing_after);
    }
}

impl TimelineCtx {
    /// The number of visible ticks across the width of the timeline.
    pub fn visible_ticks(&self) -> f32 {
        self.visible_ticks
    }

    /// Get the left edge X position where tick 0 should be displayed.
    pub fn left_edge_x(&self) -> f32 {
        self.full_rect.min.x
    }
}

// Internal access for timeline module
impl TracksCtx {
    pub(crate) fn new(full_rect: Rect, header_full_rect: Option<Rect>, timeline: TimelineCtx) -> Self {
        Self {
            full_rect,
            header_full_rect,
            timeline,
            first_track_selection_rect: std::cell::RefCell::new(None),
        }
    }
}

impl TimelineCtx {
    pub(crate) fn new(full_rect: Rect, visible_ticks: f32) -> Self {
        Self {
            full_rect,
            visible_ticks,
        }
    }
}

impl SetPlayhead {
    pub(crate) fn new(timeline_rect: Rect, tracks_top: f32, tracks_bottom: f32) -> Self {
        Self {
            timeline_rect,
            tracks_top,
            tracks_bottom,
            first_track_top_border_y: Some(tracks_top + 0.5), // First track top border y position
            full_rect: None,
            first_track_selection_rect: None,
            bottom_bar_rect: None,
            top_panel_rect: None,
        }
    }

    pub(crate) fn timeline_rect(&self) -> Rect {
        self.timeline_rect
    }

    pub(crate) fn tracks_top(&self) -> f32 {
        self.tracks_top
    }

    pub(crate) fn tracks_bottom(&self) -> f32 {
        self.tracks_bottom
    }

    /// Redraw the first track's top border to ensure it's visible on top of everything.
    /// Uses the full width including header area (same as the original track border).
    /// Also redraws the selection if it exists to ensure it's visible above everything.
    pub(crate) fn redraw_first_track_top_border(&self, ui: &mut egui::Ui) {
        if let (Some(border_y), Some(full_rect)) = (self.first_track_top_border_y, self.full_rect) {
            let pink_border = egui::Stroke {
                width: 1.0,
                color: egui::Color32::from_rgb(255, 192, 203), // Pink
            };
            // Draw across full width including header (same as original track border)
            let top_left = egui::Pos2::new(full_rect.min.x, border_y);
            let top_right = egui::Pos2::new(full_rect.max.x, border_y);
            ui.painter().line_segment([top_left, top_right], pink_border);
        }
        
        // FIX: Redraw the first track's selection after the border to ensure it's visible
        if let Some(selection_rect) = self.first_track_selection_rect {
            let selection_fill = egui::Color32::from_rgba_unmultiplied(100, 150, 255, 100);
            ui.painter().rect_filled(selection_rect, 0.0, selection_fill);
        }
    }
}
