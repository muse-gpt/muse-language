//! Shared musical clock: tempo + meter → seconds conversions.
use muse_parser::{TimePoint, TimeUnit};
use serde::{Deserialize, Serialize};

/// Frozen tempo/meter map for one compile (constant-tempo first slice).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeMap {
    pub bpm: f64,
    /// Beats per bar (meter numerator). Beat unit other than 4 is rejected upstream.
    pub beats_per_bar: u32,
}

impl TimeMap {
    pub fn new(bpm: f64, beats_per_bar: u32) -> Self {
        Self {
            bpm: bpm.max(1e-6),
            beats_per_bar: beats_per_bar.max(1),
        }
    }

    pub fn default_map() -> Self {
        Self::new(120.0, 4)
    }

    pub fn seconds_per_beat(&self) -> f64 {
        60.0 / self.bpm
    }

    pub fn seconds_per_bar(&self) -> f64 {
        self.seconds_per_beat() * f64::from(self.beats_per_bar)
    }

    pub fn bars_to_seconds(&self, bars: f64) -> f64 {
        bars * self.seconds_per_bar()
    }

    pub fn beats_to_seconds(&self, beats: f64) -> f64 {
        beats * self.seconds_per_beat()
    }

    pub fn seconds_to_bars(&self, seconds: f64) -> f64 {
        seconds / self.seconds_per_bar()
    }

    pub fn seconds_to_beats(&self, seconds: f64) -> f64 {
        seconds / self.seconds_per_beat()
    }

    /// Convert a parsed time point into wall-clock seconds.
    pub fn point_to_seconds(&self, point: TimePoint) -> f64 {
        match point.unit {
            TimeUnit::Seconds => point.value,
            TimeUnit::Beats => self.beats_to_seconds(point.value),
            TimeUnit::Bars => self.bars_to_seconds(point.value),
        }
    }

    /// Bar position for schema/compat fields (non-negative).
    pub fn point_to_bars(&self, point: TimePoint) -> f64 {
        match point.unit {
            TimeUnit::Bars => point.value,
            TimeUnit::Beats => point.value / f64::from(self.beats_per_bar),
            TimeUnit::Seconds => self.seconds_to_bars(point.value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_duration_formula() {
        let map = TimeMap::new(130.5, 4);
        // 226 bars × 4 × 60 / 130.5 ≈ 415.63
        let secs = map.bars_to_seconds(226.0);
        assert!((secs - 415.63218390804597).abs() < 1e-6);
    }

    #[test]
    fn meter_three_four() {
        let map = TimeMap::new(120.0, 3);
        assert!((map.bars_to_seconds(1.0) - 1.5).abs() < 1e-9);
    }
}
