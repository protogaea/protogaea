//! Times of year and moisture (spec §10).
//!
//! Each time of year has its value at its midpoint; in between, values change linearly tick
//! by tick, so there are no steps. Times of year are, in order: spring, summer, autumn,
//! winter. The world starts at the beginning of spring.

use crate::ruleset::Ruleset;
use crate::state::Biome;

/// Added before interpolating so that integer division only ever sees non-negative numbers
/// (spec §14).
const OFFSET: i64 = 1_000_000;

pub const SPRING: usize = 0;
pub const SUMMER: usize = 1;
pub const AUTUMN: usize = 2;
pub const WINTER: usize = 3;

/// The time of year (0–3) at a global tick.
pub fn season(rules: &Ruleset, global_tick: u64) -> usize {
    let year = rules.year_ticks();
    ((global_tick % year) / (year / 4)) as usize
}

/// A value that follows the times of year smoothly.
pub fn seasonal(rules: &Ruleset, values: [i64; 4], global_tick: u64) -> i64 {
    let year = rules.year_ticks();
    let len = year / 4;
    // Position relative to the midpoint of spring.
    let u = (global_tick % year + year - len / 2) % year;
    let from = (u / len) as usize;
    let to = (from + 1) % 4;
    let f = (u % len) as i64;
    let len = len as i64;
    let a = values[from] + OFFSET;
    let b = values[to] + OFFSET;
    (a * (len - f) + b * f) / len - OFFSET
}

/// The food growth multiplier of a biome at a global tick, in percent.
pub fn growth_pct(rules: &Ruleset, biome: Biome, global_tick: u64) -> u64 {
    let m = rules.climate.season_mult[biome as usize];
    let values = [m[0], m[1], m[2], m[3]].map(i64::from);
    seasonal(rules, values, global_tick).max(0) as u64
}

/// The moisture a cell of this biome drifts toward at a global tick.
pub fn target_moisture(rules: &Ruleset, biome: Biome, global_tick: u64) -> u8 {
    let base = i64::from(rules.climate.moisture_base[biome as usize]);
    let delta = seasonal(
        rules,
        rules.climate.season_moisture_delta.map(i64::from),
        global_tick,
    );
    (base + delta).clamp(0, 100) as u8
}

/// Food growth at a given moisture, in percent: linear between the minimum at 0 and the
/// maximum at 100.
pub fn moisture_pct(rules: &Ruleset, moisture: u8) -> u64 {
    let (lo, hi) = (
        u64::from(rules.climate.moisture_mult_min_pct),
        u64::from(rules.climate.moisture_mult_max_pct),
    );
    lo + (hi - lo) * u64::from(moisture.min(100)) / 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn midpoints_take_the_value_of_their_time_of_year() {
        let rules = Ruleset::default();
        let len = rules.year_ticks() / 4;
        let values = [10, 20, 30, 40];
        for (k, &v) in values.iter().enumerate() {
            assert_eq!(seasonal(&rules, values, len * k as u64 + len / 2), v);
        }
    }

    #[test]
    fn transitions_have_no_steps() {
        let rules = Ruleset::default();
        let values = [110, 120, 90, 40];
        let mut previous = seasonal(&rules, values, 0);
        for tick in 1..rules.year_ticks() * 2 {
            let current = seasonal(&rules, values, tick);
            assert!((current - previous).abs() <= 1, "jump at tick {tick}");
            previous = current;
        }
    }

    #[test]
    fn negative_values_interpolate_correctly() {
        let rules = Ruleset::default();
        let len = rules.year_ticks() / 4;
        // Halfway between the midpoints of summer (-20) and autumn (0).
        assert_eq!(
            seasonal(&rules, [10, -20, 0, 10], len + len / 2 + len / 2),
            -10
        );
    }

    #[test]
    fn seasons_follow_in_order() {
        let rules = Ruleset::default();
        let len = rules.year_ticks() / 4;
        assert_eq!(season(&rules, 0), SPRING);
        assert_eq!(season(&rules, len), SUMMER);
        assert_eq!(season(&rules, 2 * len), AUTUMN);
        assert_eq!(season(&rules, 3 * len), WINTER);
        assert_eq!(season(&rules, 4 * len), SPRING);
    }
}
