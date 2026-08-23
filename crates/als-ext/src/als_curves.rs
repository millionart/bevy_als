use std::collections::HashMap;

use bevy::asset::{Asset, LoadContext};
use bevy::prelude::*;
use bevy_gltf::extensions::{
    ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
};
use bevy_gltf::gltf::Animation;
use serde::Deserialize;

pub const ALS_CURVES_EXTENSION: &str = "ALS_float_curves";
pub const ALS_CURVES_LABEL: &str = "AlsCurves";

#[derive(Asset, TypePath, Debug, Default, Clone)]
pub struct AlsAnimCurves(pub HashMap<Box<str>, AlsPoseCurves>);

impl AlsAnimCurves {
    pub fn get(&self, anim: &str) -> Option<&AlsPoseCurves> {
        self.0.get(anim)
    }

    pub fn first(&self) -> Option<&AlsPoseCurves> {
        self.0.values().next()
    }
}

#[derive(Debug, Default, Clone)]
pub struct AlsPoseCurves(pub HashMap<Box<str>, AlsFloatCurve>);

impl AlsPoseCurves {
    pub fn sample(&self, curve: &str, t: f32) -> Option<f32> {
        self.0.get(curve).map(|c| c.sample(t))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsExtrapolation {
    #[default]
    Constant,
    Linear,
    Cycle,
}

#[derive(Debug, Clone, Default)]
pub struct AlsFloatCurve {
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub interps: Vec<AlsKeyInterp>,
    pub pre_infinity: AlsExtrapolation,
    pub post_infinity: AlsExtrapolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsKeyInterp {
    #[default]
    Linear,
    Constant,
}

#[derive(Debug, Clone)]
pub struct AlsCurvesParseError(pub String);

impl std::fmt::Display for AlsCurvesParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "als_float_curves parse error: {}", self.0)
    }
}

impl std::error::Error for AlsCurvesParseError {}

#[derive(Deserialize)]
struct RawCurves {
    curves: Vec<RawCurve>,
}

#[derive(Deserialize)]
struct RawCurve {
    name: String,
    #[serde(default)]
    interps: Vec<String>,
    times: Vec<f32>,
    values: Vec<f32>,
    #[serde(default)]
    pre_infinity: Option<String>,
    #[serde(default)]
    post_infinity: Option<String>,
}

fn parse_extrapolation(raw: Option<&String>) -> AlsExtrapolation {
    let Some(s) = raw else {
        return AlsExtrapolation::Constant;
    };
    match s.as_str() {
        "RCCE_Linear" => AlsExtrapolation::Linear,
        "RCCE_Cycle" | "RCCE_CycleWithOffset" | "RCCE_Oscillate" => AlsExtrapolation::Cycle,
        _ => AlsExtrapolation::Constant,
    }
}

fn parse_interp(raw: &str) -> AlsKeyInterp {
    match raw {
        "CONSTANT" => AlsKeyInterp::Constant,
        _ => AlsKeyInterp::Linear,
    }
}

pub fn parse_curves(extension_value: &serde_json::Value) -> Result<AlsPoseCurves, AlsCurvesParseError> {
    let raw: RawCurves = serde_json::from_value(extension_value.clone())
        .map_err(|e| AlsCurvesParseError(format!("json: {e}")))?;
    let mut map = HashMap::new();
    for c in raw.curves {
        if c.times.len() != c.values.len() {
            return Err(AlsCurvesParseError(format!(
                "curve {}: times.len {} != values.len {}",
                c.name,
                c.times.len(),
                c.values.len()
            )));
        }
        let mut interps: Vec<AlsKeyInterp> = c
            .interps
            .iter()
            .map(|s| parse_interp(s))
            .collect();
        interps.resize(c.times.len(), AlsKeyInterp::Linear);
        let curve = AlsFloatCurve {
            times: c.times,
            values: c.values,
            interps,
            pre_infinity: parse_extrapolation(c.pre_infinity.as_ref()),
            post_infinity: parse_extrapolation(c.post_infinity.as_ref()),
        };
        map.insert(c.name.into_boxed_str(), curve);
    }
    Ok(AlsPoseCurves(map))
}

impl AlsFloatCurve {
    pub fn sample(&self, t: f32) -> f32 {
        let n = self.times.len();
        if n == 0 {
            return 0.0;
        }
        if n == 1 {
            return self.values[0];
        }
        let first = self.times[0];
        let last = self.times[n - 1];
        let span = last - first;
        let mut t = t;
        if span <= 0.0 {
            return self.values[0];
        }
        match (t < first, t > last) {
            (true, _) => match self.pre_infinity {
                AlsExtrapolation::Constant => return self.values[0],
                AlsExtrapolation::Cycle => t += span * ((first - t) / span).ceil(),
                AlsExtrapolation::Linear => {}
            },
            (_, true) => match self.post_infinity {
                AlsExtrapolation::Constant => return self.values[n - 1],
                AlsExtrapolation::Cycle => t -= span * ((t - last) / span).floor(),
                AlsExtrapolation::Linear => {}
            },
            _ => {}
        }

        if t <= first {
            return self.extrapolate_start(t);
        }
        if t >= last {
            return self.extrapolate_end(t);
        }

        let mut lo = 0usize;
        let mut hi = n - 1;
        while hi - lo > 1 {
            let mid = (lo + hi) / 2;
            if self.times[mid] <= t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let t0 = self.times[lo];
        let t1 = self.times[hi];
        let v0 = self.values[lo];
        let v1 = self.values[hi];
        match self.interps.get(lo).copied().unwrap_or_default() {
            AlsKeyInterp::Constant => v0,
            AlsKeyInterp::Linear => {
                let a = (t - t0) / (t1 - t0);
                v0 + (v1 - v0) * a
            }
        }
    }

    fn extrapolate_start(&self, t: f32) -> f32 {
        let v0 = self.values[0];
        if self.pre_infinity != AlsExtrapolation::Linear || self.times.len() < 2 {
            return v0;
        }
        let t0 = self.times[0];
        let t1 = self.times[1];
        let slope = (self.values[1] - v0) / (t1 - t0);
        v0 + slope * (t - t0)
    }

    fn extrapolate_end(&self, t: f32) -> f32 {
        let n = self.times.len();
        let vn = self.values[n - 1];
        if self.post_infinity != AlsExtrapolation::Linear || n < 2 {
            return vn;
        }
        let tn = self.times[n - 1];
        let tp = self.times[n - 2];
        let slope = (vn - self.values[n - 2]) / (tn - tp);
        vn + slope * (t - tn)
    }
}

#[derive(Default, Clone)]
pub struct AlsCurvesHandler {
    pending: Vec<(String, AlsPoseCurves)>,
}

impl AlsCurvesHandler {
    pub fn register(app: &mut App) {
        if app.world().get_resource::<GltfExtensionHandlers>().is_none() {
            app.init_resource::<GltfExtensionHandlers>();
        }
        let handlers = app.world_mut().resource_mut::<GltfExtensionHandlers>();
        handlers.0.write_blocking().push(Box::new(Self::default()));
    }
}

impl GltfExtensionHandler for AlsCurvesHandler {
    fn dyn_clone(&self) -> Box<dyn ErasedGltfExtensionHandler> {
        Box::new(self.clone())
    }

    fn on_animation(
        &mut self,
        _load_context: &mut LoadContext<'_>,
        animation: &Animation,
        _animation_clip: &mut bevy::animation::AnimationClip,
    ) {
        let Some(exts) = animation.extensions() else {
            return;
        };
        let Some(value) = exts.get(ALS_CURVES_EXTENSION) else {
            return;
        };
        let curves = match parse_curves(value) {
            Ok(c) => c,
            Err(e) => {
                warn!("[AlsCurves] {e}");
                return;
            }
        };
        let name = animation.name().unwrap_or("animation").to_string();
        self.pending.push((name, curves));
    }

    fn on_animations_collected(
        &mut self,
        load_context: &mut LoadContext<'_>,
        _animations: &[Handle<bevy::animation::AnimationClip>],
        _named_animations: &bevy::platform::collections::HashMap<
            Box<str>,
            Handle<bevy::animation::AnimationClip>,
        >,
        _animation_roots: &bevy::platform::collections::HashSet<usize>,
    ) {
        if self.pending.is_empty() {
            return;
        }
        let mut map = HashMap::new();
        for (name, curves) in self.pending.drain(..) {
            map.insert(name.into_boxed_str(), curves);
        }
        load_context.add_labeled_asset(ALS_CURVES_LABEL.to_string(), AlsAnimCurves(map));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "curves": [
            {
                "name": "LayerArmRight",
                "flags": 4,
                "default": null,
                "pre_infinity": "RCCE_Constant",
                "post_infinity": "RCCE_Linear",
                "interps": ["LINEAR", "CONSTANT", "LINEAR"],
                "times": [0.0, 0.5, 1.0],
                "values": [0.0, 1.0, 0.5]
            },
            {
                "name": "FootPlanted",
                "flags": 4,
                "default": null,
                "pre_infinity": "RCCE_Cycle",
                "post_infinity": "RCCE_Constant",
                "interps": ["LINEAR", "LINEAR"],
                "times": [0.25, 0.75],
                "values": [-0.25, 1.0]
            }
        ]
    }"#;

    #[test]
    fn parses_names_times_values_and_interps() {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let curves = parse_curves(&v).unwrap();
        assert!(curves.sample("LayerArmRight", 0.0).is_some());
        let arm = curves.0.get("LayerArmRight").unwrap();
        assert_eq!(arm.times.len(), 3);
        assert_eq!(arm.interps[1], AlsKeyInterp::Constant);
        let foot = curves.0.get("FootPlanted").unwrap();
        assert_eq!(foot.pre_infinity, AlsExtrapolation::Cycle);
        assert_eq!(foot.post_infinity, AlsExtrapolation::Constant);
    }

    #[test]
    fn rejects_length_mismatch() {
        let bad = r#"{"curves":[{"name":"x","interps":[],"times":[0.0,1.0],"values":[1.0]}]}"#;
        let v: serde_json::Value = serde_json::from_str(bad).unwrap();
        assert!(parse_curves(&v).is_err());
    }

    #[test]
    fn samples_linear_and_constant_segments() {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let curves = parse_curves(&v).unwrap();
        let s = |t| curves.sample("LayerArmRight", t).unwrap();
        assert!((s(0.0)).abs() < 1e-6);
        assert!((s(0.25) - 0.5).abs() < 1e-6);
        assert!((s(0.5) - 1.0).abs() < 1e-6);
        assert!((s(0.7) - 1.0).abs() < 1e-6, "constant holds through segment");
        assert!((s(0.99) - 1.0).abs() < 1e-6);
        assert!((s(1.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn extrapolates_constant_linear_cycle() {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        let curves = parse_curves(&v).unwrap();
        assert!((curves.sample("LayerArmRight", -3.0).unwrap()).abs() < 1e-6);
        let end_lin = curves.sample("LayerArmRight", 1.5).unwrap();
        assert!(
            (end_lin - 0.0).abs() < 1e-6,
            "post-linear continues last segment slope, got {end_lin}"
        );

        let foot_t = |t| curves.sample("FootPlanted", t).unwrap();
        let wrapped_start = foot_t(0.05);
        assert!(
            (wrapped_start - foot_t(0.55)).abs() < 1e-6,
            "pre-cycle wraps by span, got {wrapped_start}"
        );
        assert!(
            (foot_t(1.25) - foot_t(0.75)).abs() < 1e-6,
            "post-cycle wraps back onto domain"
        );
        assert!((foot_t(9.9) - 1.0).abs() < 1e-6, "post-constant clamps");
    }

    #[test]
    fn single_key_curve_is_constant() {
        let one = AlsFloatCurve {
            times: vec![0.4],
            values: vec![0.7],
            interps: vec![],
            pre_infinity: AlsExtrapolation::default(),
            post_infinity: AlsExtrapolation::default(),
        };
        assert!((one.sample(0.0) - 0.7).abs() < 1e-6);
        assert!((one.sample(10.0) - 0.7).abs() < 1e-6);
    }
}
