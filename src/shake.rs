//! Shake detection: a stream of raw mouse deltas in, "that was a shake" out.
//!
//! A shake is a run of quick strokes that keep reversing direction. Each axis is
//! tracked on its own, so side to side, up and down, diagonals and small circles
//! all count. Distances are raw mouse counts (what the sensor reports, before
//! Windows applies pointer speed), so the cursor hitting a screen edge doesn't
//! hide the shake.

/// Strokes shorter than this (in counts) are hand tremor, not a shake.
const MIN_TRAVEL: f32 = 24.0;
/// A stroke that takes longer than this is a drag, not a shake.
const MAX_STROKE_MS: u32 = 450;
/// A pause longer than this between movements ends the run.
const MAX_GAP_MS: u32 = 220;
/// Backwards travel (counts) needed before a direction change counts.
const HYSTERESIS: f32 = 10.0;

/// The two knobs a user can turn.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    /// Minimum stroke speed, in counts per second.
    pub min_speed: f32,
    /// Direction changes in a row needed to call it a shake.
    pub wiggles: u32,
}

/// One finished stroke, reported for the live meter and calibration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// Average speed over the stroke, counts per second.
    pub speed: f32,
    /// Whether it was quick enough to count toward a shake.
    pub counts: bool,
}

#[derive(Default, Debug)]
pub struct Output {
    /// Strokes that finished on this sample (at most one per axis).
    pub strokes: [Option<Stroke>; 2],
    /// A shake just completed.
    pub shake: bool,
}

#[derive(Default, Debug, Clone)]
struct Axis {
    dir: i8,
    travel: f32,
    back: f32,
    back_start_ms: u32,
    start_ms: u32,
    last_ms: u32,
    run: u32,
}

impl Axis {
    fn feed(&mut self, d: f32, now: u32, tuning: &Tuning) -> Option<Stroke> {
        if d == 0.0 {
            return None;
        }
        if self.dir != 0 && now.wrapping_sub(self.last_ms) > MAX_GAP_MS {
            *self = Axis::default();
        }
        self.last_ms = now;
        let dir: i8 = if d > 0.0 { 1 } else { -1 };
        let dist = d.abs();

        if self.dir == 0 {
            self.dir = dir;
            self.travel = dist;
            self.back = 0.0;
            self.start_ms = now;
            return None;
        }
        if dir == self.dir {
            self.travel += dist;
            self.back = 0.0;
            return None;
        }

        // Moving against the stroke: only a real reversal once it adds up.
        if self.back == 0.0 {
            self.back_start_ms = now;
        }
        self.back += dist;
        if self.back < HYSTERESIS {
            return None;
        }

        let took = self.back_start_ms.wrapping_sub(self.start_ms).max(1);
        let speed = self.travel * 1000.0 / took as f32;
        let counts = self.travel >= MIN_TRAVEL && took <= MAX_STROKE_MS && speed >= tuning.min_speed;
        self.run = if counts { self.run + 1 } else { 0 };

        self.dir = dir;
        self.travel = self.back;
        self.back = 0.0;
        self.start_ms = self.back_start_ms;
        Some(Stroke { speed, counts })
    }
}

#[derive(Default, Debug)]
pub struct Detector {
    x: Axis,
    y: Axis,
}

impl Detector {
    /// Feed one raw delta, `now` in milliseconds from any fixed start.
    pub fn feed(&mut self, dx: f32, dy: f32, now: u32, tuning: &Tuning) -> Output {
        let sx = self.x.feed(dx, now, tuning);
        let sy = self.y.feed(dy, now, tuning);
        let shake = self.x.run.max(self.y.run) >= tuning.wiggles.max(1);
        if shake {
            self.reset();
        }
        Output { strokes: [sx, sy], shake }
    }

    /// Direction changes in a row on the busier axis.
    pub fn run(&self) -> u32 {
        self.x.run.max(self.y.run)
    }

    pub fn reset(&mut self) {
        self.x = Axis::default();
        self.y = Axis::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: Tuning = Tuning { min_speed: 2000.0, wiggles: 4 };

    /// Plays strokes of `travel` counts lasting `ms` each, alternating direction,
    /// in 1 ms / 8 ms samples like a real mouse. Returns when the shake fired (ms).
    fn play(det: &mut Detector, strokes: usize, travel: f32, ms: u32, dy: bool) -> Option<u32> {
        let mut now = 1000;
        let steps = (ms / 8).max(1);
        for s in 0..strokes {
            let sign = if s % 2 == 0 { 1.0 } else { -1.0 };
            for _ in 0..steps {
                now += 8;
                let d = sign * travel / steps as f32;
                let out = if dy { det.feed(0.0, d, now, &T) } else { det.feed(d, 0.0, now, &T) };
                if out.shake {
                    return Some(now);
                }
            }
        }
        None
    }

    #[test]
    fn a_brisk_shake_fires() {
        // 300 counts in 100 ms = 3000 counts/s, four reversals.
        assert!(play(&mut Detector::default(), 6, 300.0, 100, false).is_some());
    }

    #[test]
    fn vertical_shakes_count_too() {
        assert!(play(&mut Detector::default(), 6, 300.0, 100, true).is_some());
    }

    #[test]
    fn fires_on_the_fourth_reversal_not_before() {
        // Four strokes = three reversals: not enough.
        assert!(play(&mut Detector::default(), 4, 300.0, 100, false).is_none());
        // Five strokes = four reversals: fires during the fifth stroke.
        assert!(play(&mut Detector::default(), 5, 300.0, 100, false).is_some());
    }

    #[test]
    fn slow_back_and_forth_is_not_a_shake() {
        // 300 counts over 400 ms = 750 counts/s.
        assert!(play(&mut Detector::default(), 10, 300.0, 400, false).is_none());
    }

    #[test]
    fn tremor_is_ignored() {
        // Tiny 6-count wobbles never pass the hysteresis.
        assert!(play(&mut Detector::default(), 40, 6.0, 8, false).is_none());
    }

    #[test]
    fn one_long_sweep_is_not_a_shake() {
        let mut det = Detector::default();
        let mut now = 0;
        for _ in 0..200 {
            now += 8;
            assert!(!det.feed(40.0, 3.0, now, &T).shake);
        }
    }

    #[test]
    fn a_pause_breaks_the_run() {
        let mut det = Detector::default();
        assert!(play(&mut det, 4, 300.0, 100, false).is_none());
        // Same detector, after a long pause: it has to start over.
        let mut now = 100_000;
        for s in 0..4 {
            let sign = if s % 2 == 0 { 1.0 } else { -1.0 };
            for _ in 0..12 {
                now += 8;
                assert!(!det.feed(sign * 25.0, 0.0, now, &T).shake);
            }
        }
    }

    #[test]
    fn reports_stroke_speed() {
        let mut det = Detector::default();
        let mut speeds = vec![];
        let mut now = 0;
        for s in 0..3 {
            let sign = if s % 2 == 0 { 1.0 } else { -1.0 };
            for _ in 0..10 {
                now += 10;
                let out = det.feed(sign * 30.0, 0.0, now, &T);
                speeds.extend(out.strokes.iter().flatten().map(|s| s.speed));
            }
        }
        // 300 counts in ~100 ms.
        assert_eq!(speeds.len(), 2);
        assert!(speeds.iter().all(|&s| (2500.0..3500.0).contains(&s)), "{speeds:?}");
    }
}
