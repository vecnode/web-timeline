use ams_timeline::{
    playhead::{Info, Interaction, Playhead, PlayheadApi},
    ruler::{musical, MusicalInfo, MusicalInteract, MusicalRuler},
    Bar, TimeSig, Timeline, TimelineApi, TrackSelectionApi,
};
use std::ops::Range;
use std::collections::HashMap;
use std::cell::RefCell;

/// A block in a track, representing a time range
#[derive(Clone, Debug)]
struct Block {
    id: String,  // Globally unique block ID (8 characters, e.g., "BLK00001")
    start_tick: f32,  // Absolute tick position (from timeline start at 0)
    duration_ticks: f32,  // Duration in ticks (5 seconds = 5 * ticks_per_second)
}

/// Manages playback state and playhead position
struct PlaybackState {
    playhead_pos: RefCell<f32>,
    is_playing: RefCell<bool>,
    play_start_time: RefCell<Option<f64>>,
    play_start_playhead_pos: RefCell<f32>,
}

impl PlaybackState {
    fn new() -> Self {
        Self {
            playhead_pos: RefCell::new(0.0),
            is_playing: RefCell::new(false),
            play_start_time: RefCell::new(None),
            play_start_playhead_pos: RefCell::new(0.0),
        }
    }
}

/// Manages tracks (names, IDs, selections, and track operations)
struct TrackManager {
    track_names: RefCell<HashMap<String, String>>,
    track_ids: RefCell<Vec<String>>,
    track_selections: RefCell<HashMap<String, (f32, f32)>>,
    selected_track_id: RefCell<Option<String>>,
    selected_track_ids: RefCell<Vec<String>>,
    drag_start_tick: RefCell<Option<(String, f32)>>,
    pending_add_track: RefCell<bool>,
    track_solo: RefCell<HashMap<String, bool>>, // Track solo state (S button)
    track_mute: RefCell<HashMap<String, bool>>, // Track mute state (M button)
}

impl TrackManager {
    fn new() -> Self {
        Self {
            track_names: RefCell::new(HashMap::new()),
            track_ids: RefCell::new(Vec::new()),
            track_selections: RefCell::new(HashMap::new()),
            selected_track_id: RefCell::new(None),
            selected_track_ids: RefCell::new(Vec::new()),
            drag_start_tick: RefCell::new(None),
            pending_add_track: RefCell::new(false),
            track_solo: RefCell::new(HashMap::new()),
            track_mute: RefCell::new(HashMap::new()),
        }
    }
    
    /// Add a track with the given ID and name
    fn add_track(&self, track_id: String, track_name: String) {
        self.track_ids.borrow_mut().push(track_id.clone());
        self.track_names.borrow_mut().insert(track_id.clone(), track_name);
        // Initialize solo and mute states to false
        self.track_solo.borrow_mut().insert(track_id.clone(), false);
        self.track_mute.borrow_mut().insert(track_id, false);
    }
}

/// Manages blocks (creation, selection, dragging)
struct BlockManager {
    blocks: RefCell<HashMap<String, Vec<Block>>>,
    selected_block: RefCell<Option<(String, usize)>>,
    block_drag_start: RefCell<Option<(String, usize, f32, f32)>>,
    block_edge_resize: RefCell<Option<(String, usize, bool, f32)>>, // (track_id, block_idx, is_left_edge, original_edge_tick)
    next_block_id: RefCell<u32>,
}

impl BlockManager {
    fn new() -> Self {
        Self {
            blocks: RefCell::new(HashMap::new()),
            selected_block: RefCell::new(None),
            block_drag_start: RefCell::new(None),
            block_edge_resize: RefCell::new(None),
            next_block_id: RefCell::new(1),
        }
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "ams-timeline",
        options,
        Box::new(|_cc| Ok(Box::new(TimelineApp::default()) as Box<dyn eframe::App>)),
    )
}

/// Contains all timeline state/data
struct TimelineModel {
    timeline_start: f32,
    zoom_level: f32,
    ticks_per_beat: u32,
    global_panel_visible: bool,
    total_seconds: RefCell<u32>, // Total seconds in timeline (minimum 16)
    playback: PlaybackState,
    tracks: TrackManager,
    blocks: BlockManager,
}

impl TimelineModel {
    fn new() -> Self {
        let model = Self {
            timeline_start: 0.0,
            zoom_level: 1.0,
            ticks_per_beat: 960, // Standard MIDI PPQN
            global_panel_visible: false,
            total_seconds: RefCell::new(500), // Start with 500 seconds
            playback: PlaybackState::new(),
            tracks: TrackManager::new(),
            blocks: BlockManager::new(),
        };
        
        // Initialize with default demo tracks (created when app runs)
        model.tracks.add_track("track1".to_string(), "Track 1".to_string());
        model.tracks.add_track("track2".to_string(), "Track 2".to_string());
        
        model
    }
}

/// Contains all business logic for timeline operations
struct TimelineController;

impl TimelineController {
    /// Target frame rate for smooth playhead animation
    const TARGET_FPS: f64 = 60.0;
    
    fn new() -> Self {
        Self
    }
    
    /// Calculate ticks per bar
    fn ticks_per_bar(&self, model: &TimelineModel) -> f32 {
        let beats_per_bar = 4.0; // 4/4 time signature
        model.ticks_per_beat as f32 * beats_per_bar
    }
    
    /// Calculate ticks per second (1 bar = 1 second)
    fn ticks_per_second(&self, model: &TimelineModel) -> f32 {
        self.ticks_per_bar(model)
    }
    
    /// Get maximum playhead position (end of total seconds)
    fn max_playhead_pos(&self, model: &TimelineModel) -> f32 {
        *model.total_seconds.borrow() as f32 * self.ticks_per_bar(model)
    }
    
    /// Clamp a value to timeline bounds [0, max_pos]
    fn clamp_to_timeline_bounds(&self, value: f32, max_pos: f32) -> f32 {
        value.min(max_pos).max(0.0)
    }
    
    /// Request to add a new track (will be processed on next frame)
    fn request_add_track(&self, model: &TimelineModel) {
        *model.tracks.pending_add_track.borrow_mut() = true;
    }
    
    /// Process pending track addition (called at start of frame)
    fn process_pending_add_track(&self, model: &TimelineModel) {
        if *model.tracks.pending_add_track.borrow() {
            *model.tracks.pending_add_track.borrow_mut() = false;
            
            let mut track_ids = model.tracks.track_ids.borrow_mut();
            let mut track_names = model.tracks.track_names.borrow_mut();
            
            // Find the next available track number that doesn't have a duplicate name
            let mut track_num = track_ids.len() + 1;
            let mut new_track_name = format!("Track {}", track_num);
            
            // Check if the name already exists, if so, increment until we find a unique one
            while track_names.values().any(|name| name == &new_track_name) {
                track_num += 1;
                new_track_name = format!("Track {}", track_num);
            }
            
            // Generate new track ID
            let new_track_id = format!("track{}", track_num);
            
            // Add to ordered list and name map
            track_ids.push(new_track_id.clone());
            track_names.insert(new_track_id, new_track_name);
        }
    }
    
    /// Remove all currently selected tracks (or the primary selected one if only one exists).
    fn remove_selected_track(&self, model: &TimelineModel) {
        let mut ids_to_remove = model.tracks.selected_track_ids.borrow().clone();
        if ids_to_remove.is_empty() {
            if let Some(track_id) = model.tracks.selected_track_id.borrow().clone() {
                ids_to_remove.push(track_id);
            }
        }
        if ids_to_remove.is_empty() {
            return;
        }

        // Remove from track_ids (ordered list)
        model.tracks
            .track_ids
            .borrow_mut()
            .retain(|id| !ids_to_remove.contains(id));

        // Remove per-track data for all selected tracks
        let mut track_names = model.tracks.track_names.borrow_mut();
        let mut track_selections = model.tracks.track_selections.borrow_mut();
        let mut track_solo = model.tracks.track_solo.borrow_mut();
        let mut track_mute = model.tracks.track_mute.borrow_mut();
        let mut blocks = model.blocks.blocks.borrow_mut();
        for track_id in &ids_to_remove {
            track_names.remove(track_id);
            track_selections.remove(track_id);
            track_solo.remove(track_id);
            track_mute.remove(track_id);
            blocks.remove(track_id);
        }

        // Clear selection state after removal.
        model.tracks.selected_track_ids.borrow_mut().clear();
        *model.tracks.selected_track_id.borrow_mut() = None;
    }
    
    /// Add a block to the selected track at the current playhead position
    /// Block extends 2 seconds to the right
    fn add_block(&self, model: &TimelineModel) {
        let selected_id = model.tracks.selected_track_id.borrow().clone();
        
        if let Some(track_id) = selected_id {
            // Get current playhead position (absolute ticks)
            let playhead_absolute = *model.playback.playhead_pos.borrow();
            
            // Calculate 2 seconds in ticks (1 bar = 1 second)
            let duration_ticks = 2.0 * self.ticks_per_second(model);
            
            // Generate unique block ID (8 characters: "BLK00001", "BLK00002", etc.)
            let mut next_id = model.blocks.next_block_id.borrow_mut();
            let block_id = format!("BLK{:05}", *next_id);
            *next_id += 1;
            
            // Create the block
            let block = Block {
                id: block_id,
                start_tick: playhead_absolute,
                duration_ticks,
            };
            
            // Add to blocks map
            let mut blocks = model.blocks.blocks.borrow_mut();
            blocks.entry(track_id).or_insert_with(Vec::new).push(block);
        }
    }
    
    /// Update playhead position based on playback state
    /// Called at the start of each frame to update playhead if playing
    /// Uses time-based calculation for frame-rate independent, smooth animation
    fn update_playhead_position(&self, model: &TimelineModel, ctx: &egui::Context) {
        let is_playing = *model.playback.is_playing.borrow();
        
        if is_playing {
            let current_time = ctx.input(|i| i.time);
            let mut play_start_time = model.playback.play_start_time.borrow_mut();
            let mut play_start_playhead_pos = model.playback.play_start_playhead_pos.borrow_mut();
            
            // Initialize play start time and position if not set
            if play_start_time.is_none() {
                *play_start_time = Some(current_time);
                *play_start_playhead_pos = *model.playback.playhead_pos.borrow();
            }
            
            // Calculate elapsed time since play started
            if let Some(start_time) = *play_start_time {
                let elapsed_seconds = (current_time - start_time) as f32;
                
                // Calculate new playhead position: start position + elapsed time in ticks
                let ticks_per_second = self.ticks_per_second(model);
                let new_pos = *play_start_playhead_pos + (elapsed_seconds * ticks_per_second);
                
                // Clamp to maximum position (end of bar 500)
                let max_pos = self.max_playhead_pos(model);
                let clamped_pos = self.clamp_to_timeline_bounds(new_pos, max_pos);
                
                // Update playhead position
                *model.playback.playhead_pos.borrow_mut() = clamped_pos;
                
                // Request continuous repaints for smooth animation at target FPS
                // This creates a continuous animation loop while playing
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(1.0 / Self::TARGET_FPS));
                
                // If we reached the end, stop playback automatically
                if clamped_pos >= max_pos {
                    *model.playback.is_playing.borrow_mut() = false;
                    *play_start_time = None;
                }
            }
        } else {
            // Not playing: clear play start time so it reinitializes on next play
            *model.playback.play_start_time.borrow_mut() = None;
        }
    }
}

/// Contains all UI rendering logic
struct TimelineView;

impl TimelineView {
    fn new() -> Self {
        Self
    }
}

struct ContextMenuState {
    pos: egui::Pos2,
    panel: String,
}

/// Main application struct that composes Model, Controller, and View
struct TimelineApp {
    model: TimelineModel,
    controller: TimelineController,
    _view: TimelineView,
    context_menu: Option<ContextMenuState>,
}

impl TimelineApp {
    // Delegate methods to controller
    fn request_add_track(&self) {
        self.controller.request_add_track(&self.model);
    }
    
    fn process_pending_add_track(&self) {
        self.controller.process_pending_add_track(&self.model);
    }
    
    fn remove_selected_track(&self) {
        self.controller.remove_selected_track(&self.model);
    }
    
    fn add_block(&self) {
        self.controller.add_block(&self.model);
    }
    
    fn update_playhead_position(&self, ctx: &egui::Context) {
        self.controller.update_playhead_position(&self.model, ctx);
    }
    
    fn ticks_per_bar(&self) -> f32 {
        self.controller.ticks_per_bar(&self.model)
    }
    
    fn max_playhead_pos(&self) -> f32 {
        self.controller.max_playhead_pos(&self.model)
    }
}

impl Default for TimelineApp {
    fn default() -> Self {
        Self {
            model: TimelineModel::new(),
            controller: TimelineController::new(),
            _view: TimelineView::new(),
            context_menu: None,
        }
    }
}

impl TimelineApi for TimelineApp {
    fn musical_ruler_info(&self) -> &dyn MusicalInfo {
        self
    }

    fn timeline_start(&self) -> f32 {
        self.model.timeline_start
    }

    fn shift_timeline_start(&mut self, ticks: f32) {
        // Apply the shift - clamping is handled in the interaction handler
        // where we have access to the visible width to calculate proper max
        self.model.timeline_start += ticks;
    }

    fn zoom(&mut self, y_delta: f32) {
        self.model.zoom_level = (self.model.zoom_level * (1.0 + y_delta * 0.01)).max(0.1).min(3.0);
    }
}

impl MusicalInfo for TimelineApp {
    fn ticks_per_beat(&self) -> u32 {
        self.model.ticks_per_beat
    }

    fn timeline_start(&self) -> Option<f32> {
        Some(self.model.timeline_start)
    }
    
    fn max_absolute_tick(&self) -> Option<f32> {
        Some(self.max_playhead_pos())
    }

    fn bar_at_ticks(&self, tick: f32) -> Bar {
        let absolute_tick = self.model.timeline_start + tick;
        let ticks_per_bar = self.ticks_per_bar();
        let mut bar_number = (absolute_tick / ticks_per_bar).floor() as u32;
        
        // Clamp bar number to 0-500 (where 1 bar = 1 second)
        bar_number = bar_number.min(*self.model.total_seconds.borrow() as u32 - 1);
        
        let bar_start = bar_number as f32 * ticks_per_bar;
        let bar_end = bar_start + ticks_per_bar;
        Bar {
            tick_range: Range {
                start: bar_start - self.model.timeline_start,
                end: bar_end - self.model.timeline_start,
            },
            time_sig: TimeSig { top: 4, bottom: 4 },
        }
    }

    fn ticks_per_point(&self) -> f32 {
        (self.model.ticks_per_beat as f32 / 16.0) * self.model.zoom_level
    }
}

impl MusicalInteract for TimelineApp {
    fn click_at_tick(&mut self, tick: f32) {
        let new_pos = self.model.timeline_start + tick;
        // Clamp to maximum position (never exceed timeline end)
        let max_pos = self.max_playhead_pos();
        let clamped_pos = self.controller.clamp_to_timeline_bounds(new_pos, max_pos);
        *self.model.playback.playhead_pos.borrow_mut() = clamped_pos;
        
        // If playing and user clicks/drags playhead on ruler, reset play start to continue from new position
        // Same logic as set_playhead_ticks for tracks
        if *self.model.playback.is_playing.borrow() {
            *self.model.playback.play_start_playhead_pos.borrow_mut() = clamped_pos;
            // Reset play start time so it reinitializes with current time on next update
            *self.model.playback.play_start_time.borrow_mut() = None;
        }
    }
}

impl MusicalRuler for TimelineApp {
    fn info(&self) -> &dyn MusicalInfo {
        self
    }

    fn interact(&mut self) -> &mut dyn MusicalInteract {
        self
    }
}

impl Info for TimelineApp {
    fn playhead_ticks(&self) -> f32 {
        *self.model.playback.playhead_pos.borrow() - self.model.timeline_start
    }
}

impl Interaction for TimelineApp {
    fn set_playhead_ticks(&self, ticks: f32) {
        let new_pos = self.model.timeline_start + ticks;
        // Clamp to maximum position (never exceed timeline end)
        let max_pos = self.max_playhead_pos();
        let clamped_pos = self.controller.clamp_to_timeline_bounds(new_pos, max_pos);
        *self.model.playback.playhead_pos.borrow_mut() = clamped_pos;
        
        // If playing and user drags playhead, reset play start to continue from new position
        // We'll handle this in update_playhead_position by checking if play_start_time is None
        if *self.model.playback.is_playing.borrow() {
            *self.model.playback.play_start_playhead_pos.borrow_mut() = clamped_pos;
            // Reset play start time so it reinitializes with current time on next update
            *self.model.playback.play_start_time.borrow_mut() = None;
        }
    }
}

impl TrackSelectionApi for TimelineApp {
    fn ticks_per_point(&self) -> f32 {
        (self.model.ticks_per_beat as f32 / 16.0) * self.model.zoom_level
    }

    fn timeline_start(&self) -> f32 {
        self.model.timeline_start
    }
    
    fn is_click_on_block(&self, track_id: &str, pos: egui::Pos2, timeline_rect: egui::Rect) -> bool {
        // Check if the click position is on any block in this track
        let blocks = self.model.blocks.blocks.borrow();
        if let Some(track_blocks) = blocks.get(track_id) {
            let ticks_per_point = TrackSelectionApi::ticks_per_point(self);
            let timeline_start = self.model.timeline_start;
            
            for block in track_blocks.iter() {
                // Convert absolute ticks to relative ticks
                let start_relative_tick = block.start_tick - timeline_start;
                let end_relative_tick = (block.start_tick + block.duration_ticks) - timeline_start;
                
                // Convert to x positions
                let start_x = timeline_rect.left() + (start_relative_tick / ticks_per_point);
                let end_x = timeline_rect.left() + (end_relative_tick / ticks_per_point);
                
                // Check if position is within block's x range (y will be checked by track_rect)
                if pos.x >= start_x && pos.x <= end_x {
                    return true;
                }
            }
        }
        false
    }
    
    fn is_dragging_block(&self) -> bool {
        self.model.blocks.block_drag_start.borrow().is_some() || 
        self.model.blocks.block_edge_resize.borrow().is_some()
    }

    fn start_selection_drag(&self, track_id: &str, start_tick: f32) {
        // Clamp start tick to maximum position (never exceed timeline end)
        let max_pos = self.max_playhead_pos();
        let clamped_start = self.controller.clamp_to_timeline_bounds(start_tick, max_pos);
        *self.model.tracks.drag_start_tick.borrow_mut() = Some((track_id.to_string(), clamped_start));
    }

    fn update_selection_drag(&self, track_id: &str, end_tick: f32) {
        if let Some((drag_track_id, start_tick)) = self.model.tracks.drag_start_tick.borrow().as_ref() {
            if drag_track_id == track_id {
                // Clamp both start and end to maximum position (never exceed timeline end)
                let max_pos = self.max_playhead_pos();
                let start = self.controller.clamp_to_timeline_bounds(start_tick.min(end_tick), max_pos);
                let end = self.controller.clamp_to_timeline_bounds(start_tick.max(end_tick), max_pos);
                self.model.tracks.track_selections.borrow_mut().insert(track_id.to_string(), (start, end));
            }
        }
    }

    fn get_drag_start(&self) -> Option<(String, f32)> {
        self.model.tracks.drag_start_tick.borrow().clone()
    }

    fn end_selection_drag(&self) {
        *self.model.tracks.drag_start_tick.borrow_mut() = None;
    }

    fn set_selection(&self, track_id: &str, start_tick: f32, end_tick: f32) {
        // Clamp both start and end to maximum position (never exceed timeline end)
        let max_pos = self.max_playhead_pos();
        let clamped_start = self.controller.clamp_to_timeline_bounds(start_tick, max_pos);
        let clamped_end = self.controller.clamp_to_timeline_bounds(end_tick, max_pos);
        self.model.tracks.track_selections.borrow_mut().insert(track_id.to_string(), (clamped_start, clamped_end));
    }

    fn clear_selection(&self, track_id: &str) {
        self.model.tracks.track_selections.borrow_mut().remove(track_id);
    }

    fn clear_all_selections(&self) {
        self.model.tracks.track_selections.borrow_mut().clear();
    }

    fn get_selection(&self, track_id: &str) -> Option<(f32, f32)> {
        self.model.tracks.track_selections.borrow().get(track_id).copied()
    }

    fn get_selected_track_id(&self) -> Option<String> {
        self.model.tracks.track_selections.borrow().keys().next().cloned()
    }
}

impl eframe::App for TimelineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process pending track additions (before rendering)
        self.process_pending_add_track();
        
        // Update playhead position if playing (before rendering)
        self.update_playhead_position(ctx);
        
        // Space toggles Play/Stop (same behavior as clicking the buttons).
        // Ignore while typing into a text field.
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            let is_playing = *self.model.playback.is_playing.borrow();
            *self.model.playback.is_playing.borrow_mut() = !is_playing;
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("ams-timeline");
                ui.separator();
            });

            ui.add_space(10.0);

            // Create and show the timeline
            // Make the header column 20% wider so the input + S/M buttons fit comfortably
            let timeline = Timeline::new().header(180.0);
            let show = timeline.show(ui, self);

            let set_playhead = show.paint_grid(self)
                .pinned_tracks(|tracks, ui| {
                    // Ruler track
                    tracks.next(ui).header(|ui| {
                        // Add 4px top padding to match regular track headers
                        ui.add_space(4.0);
                        ui.label("Ruler");
                    }).show(
                        |_timeline, ui| {
                        musical(ui, self);
                        },
                        None,
                        None,
                        None::<fn(String, bool)>, // No track click handler for ruler
                        false, // Ruler is never selected
                    );
                    
                    // Marker track - same height as Ruler, starts right after Ruler
                    tracks.next(ui).header(|ui| {
                        // Add 4px top padding to match regular track headers
                        ui.add_space(4.0);
                        ui.label("Marker");
                    }).show(
                        |_timeline, ui| {
                            // Allocate exactly 20px height (same as Ruler) for empty content
                            const MARKER_HEIGHT: f32 = 20.0;
                            let w = ui.available_rect_before_wrap().width();
                            let desired_size = egui::Vec2::new(w, MARKER_HEIGHT);
                            ui.allocate_exact_size(desired_size, egui::Sense::click());
                        },
                        None,
                        None,
                        None::<fn(String, bool)>, // No track click handler for marker
                        false, // Marker is never selected
                    );
                })
                .tracks(
                    |tracks, _viewport, ui, playhead_api, selection_api| {
                    // Collect track data into local Vecs to drop RefCell borrows early
                    // This prevents borrow conflicts when the "Add Track" button is clicked
                    let track_ids_vec: Vec<String> = {
                        let track_ids = self.model.tracks.track_ids.borrow();
                        track_ids.clone()
                    };
                    let track_names_map: HashMap<String, String> = {
                        let track_names = self.model.tracks.track_names.borrow();
                        track_names.clone()
                    };
                    
                    // Get selected track IDs before the loop
                    let selected_track_ids = self.model.tracks.selected_track_ids.borrow().clone();
                    
                    let total_tracks = track_ids_vec.len();
                    for (index, track_id) in track_ids_vec.iter().enumerate() {
                        let track_name = track_names_map.get(track_id).cloned().unwrap_or_else(|| format!("Track {}", track_id));
                        let track_id_clone = track_id.clone();
                        let is_selected = selected_track_ids.contains(track_id);
                        let is_first_track = index == 0;
                        let is_last_track = index == total_tracks - 1;
                        
                        let mut track_ctx = tracks.next(ui);
                        if is_first_track {
                            track_ctx = track_ctx.mark_first_track();
                        }
                        if is_last_track {
                            track_ctx = track_ctx.mark_last_track();
                        }
                        track_ctx
                            .with_id(track_id_clone.as_str())
                            .header(|ui| {
                                // Add 4px top padding so input string is not too close to top border
                                ui.add_space(6.0);
                                let mut name = track_name.clone();
                                
                                // Use horizontal layout for input string and buttons
                                ui.horizontal(|ui| {
                                    // Natural text height for the track name
                                    let text_height = ui.text_style_height(&egui::TextStyle::Body);
                                    let input_height = text_height + 4.0;
                                    
                                    // Squares: button side equals the input height so they visually match
                                    const BUTTON_SPACING: f32 = 4.0;
                                    let button_side = input_height;
                                    
                                    // Fixed input width: 200px (not relative)
                                    const INPUT_WIDTH: f32 = 80.0;
                                    
                                    // Create TextEdit with frame disabled so it doesn't draw its own background
                                    let mut text_edit = egui::TextEdit::singleline(&mut name);
                                    text_edit = text_edit.desired_width(INPUT_WIDTH);
                                    text_edit = text_edit.frame(false); // Disable TextEdit's frame/background
                                    
                                    let input_size = egui::Vec2::new(INPUT_WIDTH, input_height);
                                    
                                    // Allocate space and draw background (no border radius - 0.0)
                                    let (rect, _response) = ui.allocate_exact_size(input_size, egui::Sense::click());
                                    let dark_grey = egui::Color32::from_rgb(50, 50, 50);
                                    ui.painter().rect_filled(rect, 3.0, dark_grey);
                                    
                                    // Add TextEdit on top
                                    let text_response = ui.put(rect, text_edit);
                                    
                                    if text_response.changed() {
                                        self.model.tracks.track_names.borrow_mut().insert(track_id_clone.clone(), name);
                                    }
                                    
                                    // Add spacing between input and buttons
                                    ui.add_space(BUTTON_SPACING);
                                    
                                    // Get current solo and mute states
                                    let solo_state = self.model.tracks.track_solo.borrow().get(&track_id_clone).copied().unwrap_or(false);
                                    let mute_state = self.model.tracks.track_mute.borrow().get(&track_id_clone).copied().unwrap_or(false);
                                    
                                    // Create square toggle buttons whose side matches the input height
                                    let button_size = egui::Vec2::new(button_side, button_side);
                                    
                                    // "S" button (Solo)
                                    let mut solo_button = egui::Button::new("S").min_size(button_size);
                                    if solo_state {
                                        // Highlight when active
                                        solo_button = solo_button.fill(egui::Color32::from_rgb(100, 150, 255));
                                    }
                                    if ui.add(solo_button).clicked() {
                                        let mut solo_map = self.model.tracks.track_solo.borrow_mut();
                                        solo_map.insert(track_id_clone.clone(), !solo_state);
                                    }
                                    
                                    // "M" button (Mute)
                                    let mut mute_button = egui::Button::new("M").min_size(button_size);
                                    if mute_state {
                                        // Highlight when active
                                        mute_button = mute_button.fill(egui::Color32::from_rgb(255, 100, 100));
                                    }
                                    if ui.add(mute_button).clicked() {
                                        let mut mute_map = self.model.tracks.track_mute.borrow_mut();
                                        mute_map.insert(track_id_clone.clone(), !mute_state);
                                    }
                                });
                            })
                            .show(
                                |timeline, ui| {
                                    // Track content area - ready for custom track data rendering
                                    // Allocate 40px height to ensure track is interactive for selection
                                    let track_height = 40.0;
                                    
                                    // Get blocks for this track
                                    let blocks_for_track = {
                                        let blocks = self.model.blocks.blocks.borrow();
                                        blocks.get(&track_id_clone).cloned().unwrap_or_default()
                                    };
                                    
                                    // Get timeline info for converting ticks to x positions
                                    let timeline_start = self.model.timeline_start;
                                    let ticks_per_point = MusicalInfo::ticks_per_point(self);
                                    let timeline_rect = timeline.full_rect;
                                    
                                    // Get the track content rect (this track's area)
                                    let track_content_rect = ui.available_rect_before_wrap();
                                    
                                    // Get selected block info
                                    let selected_block = self.model.blocks.selected_block.borrow().clone();
                                    let is_block_selected = |idx: usize| {
                                        selected_block.as_ref()
                                            .map(|(tid, bidx)| tid == &track_id_clone && *bidx == idx)
                                            .unwrap_or(false)
                                    };
                                    
                                    // Check for block clicks and drags
                                    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                                    let pointer_pressed = ui.input(|i| i.pointer.primary_pressed());
                                    let pointer_down = ui.input(|i| i.pointer.primary_down());
                                    let pointer_released = ui.input(|i| i.pointer.primary_released());

                                    // Track if we've already handled a block click this frame (only one block can be selected)
                                    let mut block_clicked_this_frame = false;
                                    
                                    // Check if we're dragging a block
                                    let block_drag_info = self.model.blocks.block_drag_start.borrow().clone();
                                    let is_dragging_block = block_drag_info.as_ref()
                                        .map(|(tid, _, _, _)| tid == &track_id_clone)
                                        .unwrap_or(false);
                                    
                                    // Draw blocks and handle clicks/drags
                                    for (block_idx, block) in blocks_for_track.iter().enumerate() {
                                        // Convert absolute ticks to relative ticks (from timeline_start)
                                        let start_relative_tick = block.start_tick - timeline_start;
                                        let end_relative_tick = (block.start_tick + block.duration_ticks) - timeline_start;
                                        
                                        // Convert relative ticks to x positions
                                        let start_x = timeline_rect.left() + (start_relative_tick / ticks_per_point);
                                        let end_x = timeline_rect.left() + (end_relative_tick / ticks_per_point);
                                        
                                        // Only draw if block is visible (at least partially in view)
                                        if end_x >= timeline_rect.left() && start_x <= timeline_rect.right() {
                                            // Clamp to visible area
                                            let block_start_x = start_x.max(timeline_rect.left());
                                            let block_end_x = end_x.min(timeline_rect.right());
                                            
                                            // Use the track content rect's y coordinates to draw inside the track
                                            // Make blocks 2px smaller in height (2px padding top, 2px padding bottom)
                                            let block_top = track_content_rect.min.y + 2.0;
                                            let block_bottom = track_content_rect.min.y + track_height - 2.0;
                                            
                                            // Draw light green rectangle with higher opacity
                                            let block_rect = egui::Rect::from_min_max(
                                                egui::Pos2::new(block_start_x, block_top),
                                                egui::Pos2::new(block_end_x, block_bottom),
                                            );
                                            
                                            // Check if this block is selected
                                            let is_selected = is_block_selected(block_idx);
                                            
                                            let light_green = egui::Color32::from_rgba_unmultiplied(144, 238, 144, 30); // Reduced opacity by 40% (50 * 0.6 = 30)
                                            ui.painter().rect_filled(block_rect, 0.0, light_green);
                                            
                                            // Draw 10px top bar with block ID
                                            const TOP_BAR_HEIGHT: f32 = 10.0;
                                            let top_bar_rect = egui::Rect::from_min_max(
                                                egui::Pos2::new(block_start_x, block_top),
                                                egui::Pos2::new(block_end_x, block_top + TOP_BAR_HEIGHT),
                                            );
                                            // Use a slightly darker green for the top bar
                                            let top_bar_color = egui::Color32::from_rgba_unmultiplied(144, 238, 144, 48); // Reduced opacity by 40% (80 * 0.6 = 48)
                                            ui.painter().rect_filled(top_bar_rect, 0.0, top_bar_color);
                                            
                                            // Draw block ID text in the top bar (8 characters, ruler font size)
                                            // Only draw if text fits within the visible block area to prevent overflow
                                            let vis = ui.style().noninteractive();
                                            let default_font_size = ui.style().text_styles.get(&egui::TextStyle::Body)
                                                .map(|f| f.size)
                                                .unwrap_or(14.0);
                                            let ruler_font = egui::FontId::new(default_font_size * 0.75, egui::FontFamily::Proportional);
                                            let id_text = &block.id; // Already 8 characters (BLK00001 format)
                                            let text_color = vis.fg_stroke.color;

                                            // Check if text fits within the visible block area to prevent overflow
                                            let text_start_x = block_start_x + 2.0;
                                            // Estimate text width (8 chars * ~4px per char for small font)
                                            let estimated_text_width = 8.0 * 4.0;
                                            let text_fits = text_start_x + estimated_text_width <= block_end_x;
                                            
                                            if text_fits {
                                                // Center the text vertically in the top bar, left-align horizontally with small padding
                                                let text_pos = egui::Pos2::new(text_start_x, block_top + TOP_BAR_HEIGHT / 2.0);
                                                ui.painter().text(text_pos, egui::Align2::LEFT_CENTER, id_text, ruler_font, text_color);
                                            }
                                            
                                            // Draw border for selected block
                                            if is_selected {
                                                let border_stroke = egui::Stroke {
                                                    width: 1.0,
                                                    color: egui::Color32::from_rgb(0, 200, 0), // Darker green border
                                                };
                                                ui.painter().rect_stroke(block_rect, 0.0, border_stroke);
                                            }
                                            
                                            // Check for hover and draw edge indicators
                                            // The hover zone extends 6px beyond the block edges, so check even if mouse is outside block
                                            if let Some(mouse_pos) = pointer_pos {
                                                const EDGE_DETECTION_WIDTH: f32 = 6.0; // 6px detection zone on each side
                                                const EDGE_INDICATOR_WIDTH: f32 = 4.0; // 4px vertical line
                                                
                                                // Create extended hover detection area: block height, but 6px wider on each side
                                                let hover_zone_top = block_top;
                                                let hover_zone_bottom = block_bottom;
                                                let hover_zone_left = start_x - EDGE_DETECTION_WIDTH;
                                                let hover_zone_right = end_x + EDGE_DETECTION_WIDTH;
                                                
                                                // Check if mouse is within the hover zone (vertically aligned with block, horizontally extended)
                                                let is_in_hover_zone = mouse_pos.y >= hover_zone_top 
                                                    && mouse_pos.y <= hover_zone_bottom
                                                    && mouse_pos.x >= hover_zone_left
                                                    && mouse_pos.x <= hover_zone_right;
                                                
                                                if is_in_hover_zone {
                                                    // Use actual block edges (not clamped) for distance calculation
                                                    let distance_from_left = (mouse_pos.x - start_x).abs();
                                                    let distance_from_right = (mouse_pos.x - end_x).abs();
                                                    
                                                    // Check if mouse is near left edge (within 6px)
                                                    if distance_from_left <= EDGE_DETECTION_WIDTH {
                                                        let edge_x = start_x;
                                                        let edge_top = block_top;
                                                        let edge_bottom = block_bottom;
                                                        let edge_stroke = egui::Stroke {
                                                            width: EDGE_INDICATOR_WIDTH,
                                                            color: egui::Color32::from_rgb(0, 150, 0), // Dark green indicator
                                                        };
                                                        ui.painter().line_segment(
                                                            [egui::Pos2::new(edge_x, edge_top), egui::Pos2::new(edge_x, edge_bottom)],
                                                            edge_stroke
                                                        );
                                                    }
                                                    
                                                    // Check if mouse is near right edge (within 6px)
                                                    if distance_from_right <= EDGE_DETECTION_WIDTH {
                                                        let edge_x = end_x;
                                                        let edge_top = block_top;
                                                        let edge_bottom = block_bottom;
                                                        let edge_stroke = egui::Stroke {
                                                            width: EDGE_INDICATOR_WIDTH,
                                                            color: egui::Color32::from_rgb(0, 150, 0), // Dark green indicator
                                                        };
                                                        ui.painter().line_segment(
                                                            [egui::Pos2::new(edge_x, edge_top), egui::Pos2::new(edge_x, edge_bottom)],
                                                            edge_stroke
                                                        );
                                                    }
                                                }
                                            }
                                            
                                            // Check for edge resize state
                                            let edge_resize_info = self.model.blocks.block_edge_resize.borrow().clone();
                                            let is_resizing_edge = edge_resize_info.as_ref()
                                                .map(|(tid, _, _, _)| tid == &track_id_clone)
                                                .unwrap_or(false);
                                            
                                            // Handle block click - check for edge resize first, then block drag
                                            if pointer_pressed && !block_clicked_this_frame {
                                                if let Some(pos) = pointer_pos {
                                                    const EDGE_DETECTION_WIDTH: f32 = 6.0;
                                                    
                                                    // Check if click is in edge resize zones (6px on each side)
                                                    let hover_zone_top = block_top;
                                                    let hover_zone_bottom = block_bottom;
                                                    let hover_zone_left = start_x - EDGE_DETECTION_WIDTH;
                                                    let hover_zone_right = end_x + EDGE_DETECTION_WIDTH;
                                                    
                                                    let is_in_hover_zone = pos.y >= hover_zone_top 
                                                        && pos.y <= hover_zone_bottom
                                                        && pos.x >= hover_zone_left
                                                        && pos.x <= hover_zone_right;
                                                    
                                                    if is_in_hover_zone {
                                                        let distance_from_left = (pos.x - start_x).abs();
                                                        let distance_from_right = (pos.x - end_x).abs();
                                                        
                                                        // Check if click is on left edge (within 6px)
                                                        if distance_from_left <= EDGE_DETECTION_WIDTH && distance_from_left < distance_from_right {
                                                            // Start left edge resize - clear any existing drag
                                                            *self.model.blocks.selected_block.borrow_mut() = Some((track_id_clone.clone(), block_idx));
                                                            *self.model.blocks.block_drag_start.borrow_mut() = None;
                                                            *self.model.blocks.block_edge_resize.borrow_mut() = Some((track_id_clone.clone(), block_idx, true, block.start_tick));
                                                            block_clicked_this_frame = true;
                                                        }
                                                        // Check if click is on right edge (within 6px)
                                                        else if distance_from_right <= EDGE_DETECTION_WIDTH {
                                                            // Start right edge resize - clear any existing drag
                                                            *self.model.blocks.selected_block.borrow_mut() = Some((track_id_clone.clone(), block_idx));
                                                            *self.model.blocks.block_drag_start.borrow_mut() = None;
                                                            let right_edge_tick = block.start_tick + block.duration_ticks;
                                                            *self.model.blocks.block_edge_resize.borrow_mut() = Some((track_id_clone.clone(), block_idx, false, right_edge_tick));
                                                            block_clicked_this_frame = true;
                                                        }
                                                    }
                                                    
                                                    // If not on edge, check if click is in block center (for dragging)
                                                    if !block_clicked_this_frame && block_rect.contains(pos) {
                                                        // Clear previous selection and select this block
                                                        *self.model.blocks.selected_block.borrow_mut() = Some((track_id_clone.clone(), block_idx));
                                                        // Start block drag - clear any existing edge resize
                                                        *self.model.blocks.block_edge_resize.borrow_mut() = None;
                                                        let block_center_x = (block_start_x + block_end_x) / 2.0;
                                                        let mouse_offset_x = pos.x - block_center_x;
                                                        *self.model.blocks.block_drag_start.borrow_mut() = Some((track_id_clone.clone(), block_idx, block.start_tick, mouse_offset_x));
                                                        block_clicked_this_frame = true;
                                                    }
                                                }
                                            }
                                            
                                            // Handle edge resize
                                            if is_resizing_edge && pointer_down {
                                                if let Some((_, resize_block_idx, is_left_edge, _original_edge_tick)) = edge_resize_info.as_ref() {
                                                    if *resize_block_idx == block_idx {
                                                        if let Some(pos) = pointer_pos {
                                                            let relative_tick = (pos.x - timeline_rect.left()) * ticks_per_point;
                                                            let new_edge_tick = timeline_start + relative_tick;
                                                            
                                                            let mut blocks = self.model.blocks.blocks.borrow_mut();
                                                            if let Some(track_blocks) = blocks.get_mut(&track_id_clone) {
                                                                if let Some(block_to_resize) = track_blocks.get_mut(block_idx) {
                                                                    let max_pos = self.max_playhead_pos();
                                                                    
                                                                    if *is_left_edge {
                                                                        // Resize from left edge: adjust start_tick and duration
                                                                        let clamped_edge = self.controller.clamp_to_timeline_bounds(new_edge_tick, max_pos);
                                                                        let new_duration = (block_to_resize.start_tick + block_to_resize.duration_ticks) - clamped_edge;
                                                                        if new_duration > 0.0 && clamped_edge >= 0.0 {
                                                                            block_to_resize.duration_ticks = new_duration;
                                                                            block_to_resize.start_tick = clamped_edge;
                                                                        }
                                                                    } else {
                                                                        // Resize from right edge: adjust duration only
                                                                        let clamped_edge = self.controller.clamp_to_timeline_bounds(new_edge_tick, max_pos);
                                                                        let new_duration = clamped_edge - block_to_resize.start_tick;
                                                                        if new_duration > 0.0 && clamped_edge <= max_pos {
                                                                            block_to_resize.duration_ticks = new_duration;
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // Handle block drag
                                            if is_dragging_block && pointer_down {
                                                if let Some((_, drag_block_idx, _original_block_start, mouse_offset_x)) = block_drag_info.as_ref() {
                                                    if *drag_block_idx == block_idx {
                                                        if let Some(pos) = pointer_pos {
                                                            // Calculate new block position: mouse_x - offset = block center x
                                                            let block_center_x = pos.x - mouse_offset_x;
                                                            let relative_tick = (block_center_x - timeline_rect.left()) * ticks_per_point;
                                                            let new_block_start = timeline_start + relative_tick - (block.duration_ticks / 2.0);
                                                            
                                                            // Update block position
                                                            let mut blocks = self.model.blocks.blocks.borrow_mut();
                                                            if let Some(track_blocks) = blocks.get_mut(&track_id_clone) {
                                                                if let Some(block_to_move) = track_blocks.get_mut(block_idx) {
                                                                    // Clamp to valid range (0 to max_playhead_pos - duration)
                                                                    let max_pos = self.max_playhead_pos() - block_to_move.duration_ticks;
                                                                    block_to_move.start_tick = self.controller.clamp_to_timeline_bounds(new_block_start, max_pos);
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            
                                            // End edge resize
                                            if is_resizing_edge && pointer_released {
                                                if let Some((_, resize_block_idx, _, _)) = edge_resize_info.as_ref() {
                                                    if *resize_block_idx == block_idx {
                                                        *self.model.blocks.block_edge_resize.borrow_mut() = None;
                                                    }
                                                }
                                            }
                                            
                                            // End block drag
                                            if is_dragging_block && pointer_released {
                                                if let Some((_, drag_block_idx, _, _)) = block_drag_info.as_ref() {
                                                    if *drag_block_idx == block_idx {
                                                        *self.model.blocks.block_drag_start.borrow_mut() = None;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    
                                    // End block drag/resize if released outside any block
                                    if pointer_released {
                                        if is_dragging_block {
                                            *self.model.blocks.block_drag_start.borrow_mut() = None;
                                        }
                                        if self.model.blocks.block_edge_resize.borrow().is_some() {
                                            *self.model.blocks.block_edge_resize.borrow_mut() = None;
                                        }
                                    }
                                    
                                    // If clicking on track content but not on any block, clear block selection
                                    if pointer_pressed && !block_clicked_this_frame {
                                        if let Some(pos) = pointer_pos {
                                            // If click is in track content area but not on any block, clear block selection
                                            if track_content_rect.contains(pos) {
                                                // Check if click is not on any block (already checked above)
                                                // Clear block selection when clicking on empty track area
                                                *self.model.blocks.selected_block.borrow_mut() = None;
                                            }
                                        }
                                    }
                                    
                                    ui.add_space(track_height);
                                },
                                playhead_api,
                                selection_api,
                                Some({
                                    let selected_track_id_ref = &self.model.tracks.selected_track_id;
                                    let selected_track_ids_ref = &self.model.tracks.selected_track_ids;
                                    move |track_id: String, shift_pressed: bool| {
                                        if shift_pressed {
                                            let mut selected_ids = selected_track_ids_ref.borrow_mut();
                                            if !selected_ids.contains(&track_id) {
                                                selected_ids.push(track_id.clone());
                                            }
                                            *selected_track_id_ref.borrow_mut() = Some(track_id);
                                        } else {
                                            *selected_track_ids_ref.borrow_mut() = vec![track_id.clone()];
                                            *selected_track_id_ref.borrow_mut() = Some(track_id);
                                        }
                                    }
                                }),
                                is_selected,
                            );
                    }
                    },
                    Some(self as &dyn PlayheadApi),
                    Some(self as &dyn TrackSelectionApi),
                );
            set_playhead
                .playhead(ui, self, Playhead::new())
                .run_scroll_and_zoom(ui, self)
                .top_panel_time(
                    ui,
                    Some(self as &dyn PlayheadApi),
                    || *self.model.playback.is_playing.borrow(), // Get is_playing
                    |val| *self.model.playback.is_playing.borrow_mut() = val, // Set is_playing
                    {
                        // Get track count without holding borrow
                        let count = self.model.tracks.track_ids.borrow().len();
                        count
                    }, // Track count
                    self.max_playhead_pos(), // Maximum absolute playhead position
                    || self.request_add_track(), // Add track callback
                    || self.remove_selected_track(), // Remove track callback
                    || self.add_block(), // Add block callback
                    || !self.model.tracks.selected_track_ids.borrow().is_empty(), // Has selected track
                    || *self.model.total_seconds.borrow(), // Get total_seconds
                    {
                        // Set total_seconds with minimum of 16 or rightmost block end
                        let total_seconds_ref = &self.model.total_seconds;
                        let blocks_ref = &self.model.blocks.blocks;
                        let ticks_per_beat = self.model.ticks_per_beat;
                        move |val| {
                            // Get the rightmost block's end position in seconds
                            // Calculate ticks_per_second (1 bar = 1 second, 4 beats per bar)
                            let ticks_per_second = ticks_per_beat as f32 * 4.0;
                            
                            let blocks = blocks_ref.borrow();
                            let mut max_end_seconds = 0.0;
                            
                            // Iterate through all tracks and their blocks
                            for track_blocks in blocks.values() {
                                for block in track_blocks.iter() {
                                    // Calculate block end position in absolute ticks
                                    let block_end_tick = block.start_tick + block.duration_ticks;
                                    // Convert to seconds
                                    let block_end_seconds = block_end_tick / ticks_per_second;
                                    // Track the maximum
                                    if block_end_seconds > max_end_seconds {
                                        max_end_seconds = block_end_seconds;
                                    }
                                }
                            }
                            
                            // Calculate minimum required seconds: max of 16, rightmost block end, or requested value
                            let min_required = (max_end_seconds.ceil() as u32).max(16);
                            // Set to the maximum of requested value and minimum required
                            *total_seconds_ref.borrow_mut() = val.max(min_required);
                        }
                    }, // Set total_seconds
                )
                .bottom_bar(ui, &mut self.model.global_panel_visible);

            // Right-click context menu (keep existing right-click deselect behavior from track interaction).
            if ui.input(|i| i.pointer.secondary_pressed()) {
                if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                    if let Some(panel_name) = set_playhead.panel_name_at_pos(pos) {
                        println!("Right-click panel: {panel_name}");
                        self.context_menu = Some(ContextMenuState {
                            pos,
                            panel: panel_name.to_string(),
                        });
                    } else {
                        self.context_menu = None;
                    }
                }
            }

            let mut close_menu = false;
            if let Some(menu_state) = self.context_menu.as_ref() {
                egui::Area::new(egui::Id::new("timeline_context_menu"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(menu_state.pos)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.label(format!("Panel: {}", menu_state.panel));
                            ui.separator();
                            if ui.button("Copy").clicked() {
                                println!("Context menu action: Copy ({})", menu_state.panel);
                                close_menu = true;
                            }
                            if ui.button("Paste").clicked() {
                                println!("Context menu action: Paste ({})", menu_state.panel);
                                close_menu = true;
                            }
                        });
                    });
            }
            if close_menu
                || ui.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.primary_clicked())
            {
                self.context_menu = None;
            }


            
        });
    }
}

