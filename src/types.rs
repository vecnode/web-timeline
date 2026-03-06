use std::ops::Range;

/// Minimum gap between step lines in points.
pub const MIN_STEP_GAP: f32 = 4.0;

/// Represents a musical bar with its time signature and tick range.
#[derive(Clone, Debug)]
pub struct Bar {
    /// The start and end offsets of the bar.
    pub tick_range: Range<f32>,
    /// The time signature of this bar.
    pub time_sig: TimeSig,
}

/// Represents a musical time signature.
#[derive(Clone, Debug)]
pub struct TimeSig {
    pub top: u16,
    pub bottom: u16,
}

impl TimeSig {
    /// The number of beats per bar of this time signature.
    pub fn beats_per_bar(&self) -> f32 {
        4.0 * self.top as f32 / self.bottom as f32
    }
}
