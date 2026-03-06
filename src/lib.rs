//! egui_timeline - A timeline widget for egui with musical ruler support

pub mod context;
pub mod grid;
pub mod interaction;
pub mod playhead;
pub mod ruler;
pub mod timeline;
pub mod types;

// The implementation required to instantiate a timeline widget.
mod timeline_api {
    use crate::ruler;

    /// The implementation required to instantiate a timeline widget.
    pub trait TimelineApi {
        /// Access to the ruler info.
        fn musical_ruler_info(&self) -> &dyn ruler::MusicalInfo;
        /// Get the current timeline start position in ticks.
        /// This should return 0.0 or greater - negative values are not allowed.
        fn timeline_start(&self) -> f32;
        /// Shift the timeline start by the given number of ticks due to a scroll event.
        /// The implementation should clamp the result to ensure it never goes below 0.0.
        fn shift_timeline_start(&mut self, ticks: f32);
        /// The timeline was scrolled with with `Ctrl` held down to zoom in/out.
        fn zoom(&mut self, y_delta: f32);
    }
}

// Re-export public API
pub use playhead::{Playhead, PlayheadApi};
pub use ruler::MusicalRuler;
pub use context::SetPlayhead;
pub use timeline::{Show, Timeline};
pub use types::{Bar, TimeSig};
pub use interaction::TrackSelectionApi;

// Re-export TimelineApi trait
pub use timeline_api::TimelineApi;

// Re-export context types for convenience
pub use context::{BackgroundCtx, TimelineCtx, TrackCtx, TracksCtx};
