use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Asset, Reflect, Serialize, Deserialize, Debug, Clone, PartialEq)]
#[reflect(Default)]
pub struct CurveAsset {
    pub mode: CurveSampleMode,
    pub keys: Vec<CurveKey>,
}

impl CurveAsset {
    pub fn new(mode: CurveSampleMode, mut keys: Vec<CurveKey>) -> Self {
        keys.sort_by(|a, b| a.t.total_cmp(&b.t));
        Self { mode, keys }
    }

    pub fn sample(&self, t: f32) -> f32 {
        sample_curve(self.mode, &self.keys, t)
    }

    pub fn domain(&self) -> Option<(f32, f32)> {
        Some((self.keys.first()?.t, self.keys.last()?.t))
    }
}

impl Default for CurveAsset {
    fn default() -> Self {
        Self {
            mode: CurveSampleMode::Linear,
            keys: vec![CurveKey { t: 0.0, v: 0.0 }, CurveKey { t: 1.0, v: 1.0 }],
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CurveSampleMode {
    #[default]
    Linear,
    Step,
    Smooth,
}

#[derive(Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct CurveKey {
    pub t: f32,
    pub v: f32,
}

pub fn sample_curve(mode: CurveSampleMode, keys: &[CurveKey], t: f32) -> f32 {
    let Some(&first) = keys.first() else {
        return 0.0;
    };
    let Some(&last) = keys.last() else {
        return 0.0;
    };
    if t.is_nan() {
        return t;
    }
    if t <= first.t {
        return first.v;
    }
    if t >= last.t {
        return last.v;
    }
    let upper = keys.partition_point(|k| k.t < t);
    let a = &keys[upper - 1];
    let b = &keys[upper];
    let u = ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
    match mode {
        CurveSampleMode::Step => {
            if u >= 1.0 {
                b.v
            } else {
                a.v
            }
        }
        CurveSampleMode::Linear => a.v + (b.v - a.v) * u,
        CurveSampleMode::Smooth => {
            let s = u * u * (3.0 - 2.0 * u);
            a.v + (b.v - a.v) * s
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(pairs: &[(f32, f32)]) -> Vec<CurveKey> {
        pairs.iter().map(|&(t, v)| CurveKey { t, v }).collect()
    }

    #[test]
    fn linear_endpoints_exact_and_midpoint() {
        let c = CurveAsset::new(CurveSampleMode::Linear, keys(&[(0.0, 1.0), (2.0, 3.0)]));
        assert_eq!(c.sample(0.0), 1.0);
        assert_eq!(c.sample(2.0), 3.0);
        assert!((c.sample(1.0) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn step_holds_left_value_until_next_key() {
        let c = CurveAsset::new(
            CurveSampleMode::Step,
            keys(&[(0.0, 1.0), (1.0, 5.0), (2.0, 9.0)]),
        );
        assert_eq!(c.sample(0.999), 1.0);
        assert_eq!(c.sample(1.0), 5.0);
        assert_eq!(c.sample(1.5), 5.0);
    }

    #[test]
    fn smooth_symmetric_midpoint_matches_linear_midpoint() {
        let c = CurveAsset::new(CurveSampleMode::Smooth, keys(&[(0.0, 0.0), (1.0, 4.0)]));
        assert!((c.sample(0.5) - 2.0).abs() < 1e-6);
        assert!(c.sample(0.25) < 1.0);
        assert!(c.sample(0.75) > 3.0);
    }

    #[test]
    fn out_of_domain_clamps_to_ends() {
        let c = CurveAsset::new(CurveSampleMode::Linear, keys(&[(1.0, 7.0), (3.0, 11.0)]));
        assert_eq!(c.sample(-5.0), 7.0);
        assert_eq!(c.sample(99.0), 11.0);
    }

    #[test]
    fn new_sorts_unsorted_keys() {
        let c = CurveAsset::new(CurveSampleMode::Linear, keys(&[(2.0, 20.0), (0.0, 10.0)]));
        assert_eq!(c.domain(), Some((0.0, 2.0)));
        assert!((c.sample(1.0) - 15.0).abs() < 1e-6);
    }
}
