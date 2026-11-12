use std::collections::HashMap;

use bevy::asset::{Asset, LoadContext};
use bevy::prelude::*;
use bevy_gltf::extensions::{
    ErasedGltfExtensionHandler, GltfExtensionHandler, GltfExtensionHandlers,
};
use bevy_gltf::gltf::Animation;
use serde::{Deserialize, Serialize};

use crate::extensions::rich_curve::{cubic_bezier, unweighted_cubic};

pub const ALS_CURVES_EXTENSION: &str = "ALS_float_curves";
pub const ALS_CURVES_LABEL: &str = "AlsCurves";

#[derive(Reflect, Serialize, Deserialize, Debug, Default, Clone)]
#[reflect(opaque, Default, Serialize, Deserialize, Hash, PartialEq)]
#[serde(transparent)]
pub struct SourceCurveName(String);

impl SourceCurveName {
    pub fn as_str(&self) -> &str { &self.0 }
}

impl<S: AsRef<str> + ?Sized> PartialEq<S> for SourceCurveName {
    fn eq(&self, other: &S) -> bool { self.0.eq_ignore_ascii_case(other.as_ref()) }
}
impl Eq for SourceCurveName {}
impl std::hash::Hash for SourceCurveName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.0.len());
        for byte in self.0.bytes() { state.write_u8(byte.to_ascii_lowercase()); }
    }
}
impl AsRef<str> for SourceCurveName {
    fn as_ref(&self) -> &str { &self.0 }
}
impl std::ops::Deref for SourceCurveName {
    type Target = str;
    fn deref(&self) -> &str { &self.0 }
}
impl std::fmt::Display for SourceCurveName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.0.fmt(f) }
}
impl From<String> for SourceCurveName {
    fn from(value: String) -> Self { Self(value) }
}
impl From<&str> for SourceCurveName {
    fn from(value: &str) -> Self { Self(value.to_owned()) }
}
impl From<&String> for SourceCurveName {
    fn from(value: &String) -> Self { Self(value.clone()) }
}
impl From<Box<str>> for SourceCurveName {
    fn from(value: Box<str>) -> Self { Self(value.into()) }
}
impl From<SourceCurveName> for String {
    fn from(value: SourceCurveName) -> Self { value.0 }
}

pub type SourceCurveValues = bevy::platform::collections::HashMap<SourceCurveName, f32>;

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

#[derive(Reflect, Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
#[reflect(opaque, Default, Serialize, Deserialize)]
pub struct AlsPoseCurves(pub HashMap<SourceCurveName, AlsFloatCurve>);

impl AlsPoseCurves {
    pub fn sample(&self, curve: &str, t: f32) -> Option<f32> {
        self.0.get(&SourceCurveName::from(curve)).map(|c| c.sample(t))
    }

    pub fn sample_all(&self, time: f32) -> SourceCurveValues {
        self.0
            .iter()
            .map(|(name, curve)| (name.clone(), curve.sample(time)))
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsExtrapolation {
    #[default]
    Constant,
    Linear,
    Cycle,
    CycleWithOffset,
    Oscillate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct AlsFloatCurve {
    pub times: Vec<f32>,
    pub values: Vec<f32>,
    pub interps: Vec<AlsKeyInterp>,
    pub tangent_modes: Vec<u8>,
    pub weight_modes: Vec<u8>,
    pub arrive_tangents: Vec<f32>,
    pub leave_tangents: Vec<f32>,
    pub arrive_weights: Vec<f32>,
    pub leave_weights: Vec<f32>,
    pub pre_infinity: AlsExtrapolation,
    pub post_infinity: AlsExtrapolation,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlsKeyInterp {
    #[default]
    Linear,
    Constant,
    Cubic,
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
    #[serde(default)]
    tangent_modes: Vec<u8>,
    #[serde(default)]
    weight_modes: Vec<u8>,
    #[serde(default)]
    arrive_tangents: Vec<f32>,
    #[serde(default)]
    leave_tangents: Vec<f32>,
    #[serde(default)]
    arrive_weights: Vec<f32>,
    #[serde(default)]
    leave_weights: Vec<f32>,
}

fn parse_extrapolation(raw: Option<&String>) -> AlsExtrapolation {
    let Some(s) = raw else {
        return AlsExtrapolation::Constant;
    };
    match s.as_str() {
        "RCCE_Linear" => AlsExtrapolation::Linear,
        "RCCE_Cycle" => AlsExtrapolation::Cycle,
        "RCCE_CycleWithOffset" => AlsExtrapolation::CycleWithOffset,
        "RCCE_Oscillate" => AlsExtrapolation::Oscillate,
        _ => AlsExtrapolation::Constant,
    }
}

fn parse_interp(raw: &str) -> AlsKeyInterp {
    match raw {
        "CONSTANT" => AlsKeyInterp::Constant,
        "CUBIC" => AlsKeyInterp::Cubic,
        _ => AlsKeyInterp::Linear,
    }
}

fn resize_values<T: Clone>(mut values: Vec<T>, length: usize, default: T) -> Vec<T> {
    values.resize(length, default);
    values
}

pub fn parse_curves(
    extension_value: &serde_json::Value,
) -> Result<AlsPoseCurves, AlsCurvesParseError> {
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
        let mut interps: Vec<AlsKeyInterp> = c.interps.iter().map(|s| parse_interp(s)).collect();
        interps.resize(c.times.len(), AlsKeyInterp::Linear);
        let length = c.times.len();
        let curve = AlsFloatCurve {
            times: c.times,
            values: c.values,
            interps,
            tangent_modes: resize_values(c.tangent_modes, length, 0),
            weight_modes: resize_values(c.weight_modes, length, 0),
            arrive_tangents: resize_values(c.arrive_tangents, length, 0.0),
            leave_tangents: resize_values(c.leave_tangents, length, 0.0),
            arrive_weights: resize_values(c.arrive_weights, length, 0.0),
            leave_weights: resize_values(c.leave_weights, length, 0.0),
            pre_infinity: parse_extrapolation(c.pre_infinity.as_ref()),
            post_infinity: parse_extrapolation(c.post_infinity.as_ref()),
        };
        map.insert(c.name.into(), curve);
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
        let mut t = t;
        let mut cycle_value_offset = 0.0;
        if last - first <= 0.0 {
            return self.values[0];
        }
        if t < first {
            match self.pre_infinity {
                AlsExtrapolation::Constant => return self.values[0],
                AlsExtrapolation::Linear => {}
                extrapolation => {
                    let (cycled_time, cycle_count) = cycle_time(first, last, t);
                    t = cycled_time;
                    match extrapolation {
                        AlsExtrapolation::CycleWithOffset => {
                            cycle_value_offset =
                                (self.values[0] - self.values[n - 1]) * cycle_count as f32;
                        }
                        AlsExtrapolation::Oscillate if cycle_count % 2 == 1 => {
                            t = first + (last - t);
                        }
                        _ => {}
                    }
                }
            }
        } else if t > last {
            match self.post_infinity {
                AlsExtrapolation::Constant => return self.values[n - 1],
                AlsExtrapolation::Linear => {}
                extrapolation => {
                    let (cycled_time, cycle_count) = cycle_time(first, last, t);
                    t = cycled_time;
                    match extrapolation {
                        AlsExtrapolation::CycleWithOffset => {
                            cycle_value_offset =
                                (self.values[n - 1] - self.values[0]) * cycle_count as f32;
                        }
                        AlsExtrapolation::Oscillate if cycle_count % 2 == 1 => {
                            t = first + (last - t);
                        }
                        _ => {}
                    }
                }
            }
        }

        if t <= first {
            return self.extrapolate_start(t) + cycle_value_offset;
        }
        if t >= last {
            return self.extrapolate_end(t) + cycle_value_offset;
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
        let value = match self.interps.get(lo).copied().unwrap_or_default() {
            AlsKeyInterp::Constant => v0,
            AlsKeyInterp::Linear => {
                let a = (t - t0) / (t1 - t0);
                v0 + (v1 - v0) * a
            }
            AlsKeyInterp::Cubic => self.sample_cubic(lo, hi, t, t0, t1, v0, v1),
        };
        value + cycle_value_offset
    }

    fn sample_cubic(
        &self,
        lo: usize,
        hi: usize,
        t: f32,
        t0: f32,
        t1: f32,
        v0: f32,
        v1: f32,
    ) -> f32 {
        let duration = t1 - t0;
        if duration <= f32::EPSILON {
            return v0;
        }
        let leave_weighted = self.weight_modes.get(lo).copied().unwrap_or(0) & 2 != 0;
        let arrive_weighted = self.weight_modes.get(hi).copied().unwrap_or(0) & 1 != 0;
        if !leave_weighted && !arrive_weighted {
            let alpha = ((t - t0) / duration).clamp(0.0, 1.0);
            return unweighted_cubic(v0, v1, self.leave_tangent(lo), self.arrive_tangent(hi), duration, alpha);
        }
        let leave_tangent = self.leave_tangent(lo);
        let arrive_tangent = self.arrive_tangent(hi);
        let leave_weight = if leave_weighted {
            self.leave_weights.get(lo).copied().unwrap_or(0.0)
        } else {
            tangent_weight(duration, leave_tangent)
        };
        let arrive_weight = if arrive_weighted {
            self.arrive_weights.get(hi).copied().unwrap_or(0.0)
        } else {
            tangent_weight(duration, arrive_tangent)
        };
        let (leave_sine, leave_cosine) = leave_tangent.atan().sin_cos();
        let (arrive_sine, arrive_cosine) = arrive_tangent.atan().sin_cos();
        let control_time_1 = t0 + leave_cosine * leave_weight;
        let control_time_2 = t1 - arrive_cosine * arrive_weight;
        let control_value_1 = v0 + leave_sine * leave_weight;
        let control_value_2 = v1 - arrive_sine * arrive_weight;
        let parameter = cubic_parameter_for_time(t, t0, control_time_1, control_time_2, t1);
        cubic_bezier(v0, control_value_1, control_value_2, v1, parameter)
    }

    fn arrive_tangent(&self, index: usize) -> f32 {
        self.arrive_tangents.get(index).copied().unwrap_or(0.0)
    }

    fn leave_tangent(&self, index: usize) -> f32 {
        self.leave_tangents.get(index).copied().unwrap_or(0.0)
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

fn tangent_weight(duration: f32, tangent: f32) -> f32 {
    let value_delta = tangent * duration;
    (duration * duration + value_delta * value_delta).sqrt() / 3.0
}

fn cycle_time(min_time: f32, max_time: f32, time: f32) -> (f32, u64) {
    let duration = max_time - min_time;
    if duration <= f32::EPSILON {
        return (min_time, 0);
    }
    let mut remapped_time = time;
    let cycle_count = if time > max_time {
        let count = ((max_time - time) / duration).floor() as i64;
        remapped_time += duration * count as f32;
        count
    } else if time < min_time {
        let count = ((time - min_time) / duration).floor() as i64;
        remapped_time -= duration * count as f32;
        count
    } else {
        0
    };
    if remapped_time == max_time && time < min_time {
        remapped_time = min_time;
    }
    if remapped_time == min_time && time > max_time {
        remapped_time = max_time;
    }
    (remapped_time, cycle_count.unsigned_abs())
}

fn cubic_parameter_for_time(time: f32, t0: f32, t1: f32, t2: f32, t3: f32) -> f32 {
    let range = t3 - t0;
    if range.abs() <= f32::EPSILON {
        return 0.0;
    }
    let control_1 = ((t1 - t0) / range) as f64;
    let control_2 = ((t2 - t0) / range) as f64;
    let alpha = ((time - t0) / range) as f64;
    let cubic = 1.0 - 3.0 * control_2 + 3.0 * control_1;
    let quadratic = 3.0 * control_2 - 6.0 * control_1;
    let linear = 3.0 * control_1;
    let roots = cubic_roots(cubic, quadratic, linear, -alpha);
    roots
        .into_iter()
        .filter(|root| *root >= -1e-9 && *root <= 1.0 + 1e-9)
        .map(|root| root.clamp(0.0, 1.0))
        .max_by(f64::total_cmp)
        .unwrap_or(0.0) as f32
}

fn cubic_roots(cubic: f64, quadratic: f64, linear: f64, constant: f64) -> Vec<f64> {
    if cubic.abs() <= 1e-12 {
        return quadratic_roots(quadratic, linear, constant);
    }
    let normalized_quadratic = quadratic / cubic;
    let normalized_linear = linear / cubic;
    let normalized_constant = constant / cubic;
    let p = normalized_linear - normalized_quadratic * normalized_quadratic / 3.0;
    let q = 2.0 * normalized_quadratic.powi(3) / 27.0
        - normalized_quadratic * normalized_linear / 3.0
        + normalized_constant;
    let discriminant = (q * 0.5).powi(2) + (p / 3.0).powi(3);
    let offset = normalized_quadratic / 3.0;
    if discriminant > 1e-12 {
        let root =
            (-q * 0.5 + discriminant.sqrt()).cbrt() + (-q * 0.5 - discriminant.sqrt()).cbrt();
        return vec![root - offset];
    }
    if discriminant >= -1e-12 {
        let root = (-q * 0.5).cbrt();
        return vec![2.0 * root - offset, -root - offset];
    }
    let radius = 2.0 * (-p / 3.0).sqrt();
    let cosine = (-q * 0.5 / (-p / 3.0).sqrt().powi(3)).clamp(-1.0, 1.0);
    let angle = cosine.acos();
    (0..3)
        .map(|index| radius * ((angle + std::f64::consts::TAU * index as f64) / 3.0).cos() - offset)
        .collect()
}

fn quadratic_roots(quadratic: f64, linear: f64, constant: f64) -> Vec<f64> {
    if quadratic.abs() <= 1e-12 {
        return if linear.abs() <= 1e-12 {
            Vec::new()
        } else {
            vec![-constant / linear]
        };
    }
    let discriminant = linear * linear - 4.0 * quadratic * constant;
    if discriminant < -1e-12 {
        return Vec::new();
    }
    if discriminant <= 1e-12 {
        return vec![-linear / (2.0 * quadratic)];
    }
    let square_root = discriminant.sqrt();
    vec![
        (-linear + square_root) / (2.0 * quadratic),
        (-linear - square_root) / (2.0 * quadratic),
    ]
}

#[derive(Default, Clone)]
pub struct AlsCurvesHandler {
    pending: Vec<(String, AlsPoseCurves)>,
}

impl AlsCurvesHandler {
    pub fn register(app: &mut App) {
        if app
            .world()
            .get_resource::<GltfExtensionHandlers>()
            .is_none()
        {
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

    #[test]
    fn source_curve_name_keeps_spelling_and_ascii_comparison() {
        let name = SourceCurveName::from("FixtureCurve");
        let alias = SourceCurveName::from("fixturecurve");
        let mut values = HashMap::from([(name.clone(), 1)]);
        values.insert(alias.clone(), 2);
        assert_eq!(values.len(), 1);
        assert_eq!(values.get(&name), Some(&2));
        assert_eq!(values.keys().next().unwrap().as_str(), "FixtureCurve");
        let encoded = serde_json::to_string(&name).unwrap();
        assert_eq!(encoded, "\"FixtureCurve\"");
        assert_eq!(serde_json::from_str::<SourceCurveName>(&encoded).unwrap(), name);
        assert_ne!(SourceCurveName::from("\u{00C4}"), SourceCurveName::from("\u{00E4}"));
        assert_ne!(SourceCurveName::from("\u{212A}ey"), SourceCurveName::from("key"));
        assert_ne!(SourceCurveName::from("Curve_Name"), SourceCurveName::from("CurveName"));
    }

    #[test]
    fn source_curves_preserve_fname_case_identity() {
        let curves = parse_curves(&serde_json::json!({"curves":[{
            "name":"FixtureCurve", "times":[0.0,1.0], "values":[2.0,6.0], "interps":["LINEAR","LINEAR"]
        }]})).unwrap();
        for name in ["FixtureCurve", "fixturecurve", "FIXTURECURVE"] {
            assert_eq!(curves.sample(name, 0.5), Some(4.0), "curve name: {name}");
        }
    }

    #[test]
    fn source_graph_samples_curves_at_pose_time_and_blends_actual_weights() {
        use bevy::animation::animated_field;
        use crate::extensions::{
            AnimationContextState, NormalizedBlendSpaceNode, NormalizedBlendSpacePoint,
            SourceApplyAdditiveNode, SourceCurveModifyNode, SourceLayeredBoneBlendNode,
            SourceSequenceEvaluatorNode, SourceSequencePlayerNode, SourceTwoWayBlendNode,
        };
        use bevy::ecs::system::RunSystemOnce;
        use bevy::platform::collections::HashMap as GraphMap;
        use bevy_animation_graph::core::{
            animation_clip::{EntityPath, GraphClip},
            animation_graph::{AnimationGraph, GraphInputPin, TimeUpdate},
            animation_node::AnimationNode,
            context::{
                deferred_gizmos::DeferredGizmos, graph_context::QueryOutputTime,
                graph_context_arena::GraphContextArena, new_context::GraphContext,
                system_resources::SystemResources,
            },
            edge_data::{DataSpec, DataValue},
            pose::Pose,
            skeleton::Skeleton,
            state_machine::high_level::StateMachine,
            utils::testing::TestIoEnv,
        };
        fn query(
            In((handle, explicit_time)): In<(Handle<AnimationGraph>, f32)>,
            resources: SystemResources,
        ) -> Vec<AnimationContextState> {
            let graph = resources.animation_graph_assets.get(&handle).unwrap();
            let mut arena = GraphContextArena::new(handle.id());
            let mut gizmos = DeferredGizmos::default();
            let entities = GraphMap::new();
            let env = TestIoEnv::default();
            let inputs = GraphMap::from([("draw_amount".to_owned(), DataValue::F32(explicit_time))]);
            let context = GraphContext::new(
                arena.get_toplevel_id(),
                graph,
                &mut arena,
                &resources,
                &env,
                Entity::PLACEHOLDER,
                &entities,
                &mut gizmos,
                &inputs,
            );
            let output = graph
                .query_with_context(
                    QueryOutputTime::Forced(TimeUpdate::Delta(0.02)),
                    context.clone(),
                )
                .unwrap();
            assert_eq!(output["evaluator"].as_pose().unwrap().timestamp, 1.0);
            let root = EntityPath::default().child("root").id();
            let expected_time = explicit_time.clamp(0.0, 1.0);
            let time_pose = output["time_evaluator"].as_pose().unwrap();
            assert_eq!(time_pose.timestamp, expected_time);
            assert_eq!(time_pose.get_bone(root).unwrap().translation, Some(Vec3::Z * (2.75 * expected_time)));
            let time_context = crate::extensions::animation_context::graph_output_context_state(graph, &context, "time_evaluator").unwrap();
            assert_eq!(time_context.curves[&crate::extensions::SourceCurveName::from("SourceClock")], expected_time);
            let modified = crate::extensions::animation_context::graph_output_context_state(graph, &context, "case_modified").unwrap();
            assert_eq!(modified.curves.len(), 1);
            assert_eq!(modified.curves[&crate::extensions::SourceCurveName::from("SourceClock")], expected_time + 2.0);
            for name in ["sequence", "evaluator"] {
                assert_eq!(output[name].as_pose().unwrap().get_bone(root).unwrap().translation, Some(Vec3::Z * 0.2));
            }
            [
                "sequence",
                "evaluator",
                "blendspace",
                "two_way",
                "additive",
                "layered",
                "inactive_additive",
                "scaled_missing",
            ]
            .map(|output| {
                crate::extensions::animation_context::graph_output_context_state(graph, &context, output)
                    .unwrap_or_default()
            })
            .to_vec()
        }
        let curves = parse_curves(
            &serde_json::from_str(include_str!(
                "../../tools/animgraph_authoring/fixtures/ue_standing_walk_forward_curves.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let mut clips = Assets::<GraphClip>::default();
        let mut skeletons = Assets::<Skeleton>::default();
        let root_path = EntityPath::default().child("root");
        let mut skeleton = Skeleton::default();
        skeleton.set_root(root_path.id());
        skeleton.add_bone(root_path.clone(), Transform::from_translation(Vec3::Z * 0.2), Transform::from_translation(Vec3::Z * 0.2));
        let mut animation = bevy::animation::AnimationClip::default();
        animation.add_curve_to_target(root_path.id().animation_target_id(), AnimatableCurve::new(
            animated_field!(Transform::translation),
            UnevenSampleAutoCurve::new([(0.0, Vec3::ZERO), (1.0, Vec3::Z * 2.75)]).unwrap(),
        ));
        let clip = clips.add(GraphClip::from_bevy_clip(animation, skeletons.add(skeleton), default(), None));
        let sequence = AnimationNode::new(
            "sequence",
            SourceSequencePlayerNode {
                clip: clip.clone(),
                curves: curves.clone(),
                root_lock: Some(crate::extensions::source_sequence_player::SourceRootLock::RefPose),
                start_position: 0.1,
                expose_play_rate_pin: false,
                expose_start_position_pin: false,
                ..default()
            },
        );
        let evaluator = AnimationNode::new(
            "evaluator",
            SourceSequenceEvaluatorNode {
                clip: clip.clone(),
                curves: curves.clone(),
                root_lock: Some(crate::extensions::source_sequence_player::SourceRootLock::RefPose),
                explicit_value: 30.0,
                use_explicit_frame: true,
                ..default()
            },
        );
        let time_evaluator = AnimationNode::new("time_evaluator", SourceSequenceEvaluatorNode {
            clip,
            curves: parse_curves(&serde_json::json!({"curves":[{"name":"SourceClock", "times":[0.0,1.0], "values":[0.0,1.0], "interps":["LINEAR","LINEAR"]}]})).unwrap(),
            explicit_value_key: "draw_amount".to_owned(),
            frames_per_second: 60.0,
            ..default()
        });
        let blendspace = AnimationNode::new(
            "blendspace",
            NormalizedBlendSpaceNode {
                looped: true,
                points: vec![
                    NormalizedBlendSpacePoint {
                        id: "a".into(),
                        duration: 1.0,
                        curves,
                        ..default()
                    },
                    NormalizedBlendSpacePoint {
                        id: "b".into(),
                        duration: 1.0,
                        point: Vec2::X,
                        ..default()
                    },
                ],
                ..default()
            },
        );
        let two_way = AnimationNode::new(
            "two_way",
            SourceTwoWayBlendNode {
                pin_default: 0.25,
                ..default()
            },
        );
        let additive = AnimationNode::new(
            "additive",
            SourceApplyAdditiveNode {
                use_constant_alpha: true,
                constant_alpha: 0.5,
                ..default()
            },
        );
        let layered = AnimationNode::new(
            "layered",
            SourceLayeredBoneBlendNode {
                blend_weight: 0.5,
                ..default()
            },
        );
        let inactive_additive = AnimationNode::new(
            "inactive_additive",
            SourceApplyAdditiveNode {
                use_constant_alpha: true,
                constant_alpha: 0.0,
                ..default()
            },
        );
        let scaled_missing = AnimationNode::new(
            "scaled_missing",
            SourceCurveModifyNode {
                curve_name: "Missing".into(),
                pin_default: 2.0,
                alpha: 1.0,
                apply_mode: "Scale".into(),
                ..default()
            },
        );
        let case_modified = AnimationNode::new("case_modified", SourceCurveModifyNode {
            curve_name: "sOuRcEcLoCk".into(), pin_default: 2.0, alpha: 1.0, apply_mode: "Add".into(), ..default()
        });
        let mut graph = AnimationGraph::new();
        for (name, value) in [
            ("position", DataValue::Vec2(Vec2::new(0.5, 0.0))),
            ("phase", DataValue::F32(0.12)),
            ("sample", DataValue::Pose(Pose::default())),
            ("alpha", DataValue::F32(0.25)),
        ] {
            graph.set_default_data(GraphInputPin::passthrough(name), value);
        }
        for (node, output) in [
            (&sequence, "Pose"),
            (&evaluator, "Pose"),
            (&time_evaluator, "Pose"),
            (&blendspace, "pose"),
            (&two_way, "Pose"),
            (&additive, "Pose"),
            (&layered, "Pose"),
            (&inactive_additive, "Pose"),
            (&scaled_missing, "Pose"),
            (&case_modified, "Pose"),
        ] {
            graph.add_node(node.clone());
            graph.add_output_data(node.name.clone(), DataSpec::Pose);
            graph.add_output_data_edge(node.id, output, node.name.clone());
        }
        for (input, pin) in [
            ("position", "position"),
            ("phase", "phase"),
            ("sample", "pose_a"),
            ("sample", "pose_b"),
        ] {
            graph.add_input_data_edge(GraphInputPin::passthrough(input), blendspace.id, pin);
        }
        graph.add_node_parameter_edge(sequence.id, "Pose", two_way.id, "A");
        graph.add_node_parameter_edge(time_evaluator.id, "Pose", case_modified.id, "Source Pose");
        graph.add_input_data_edge(GraphInputPin::passthrough("alpha"), two_way.id, "Alpha");
        graph.add_node_parameter_edge(evaluator.id, "Pose", two_way.id, "B");
        graph.add_node_parameter_edge(two_way.id, "Pose", additive.id, "Base");
        graph.add_node_parameter_edge(blendspace.id, "pose", additive.id, "Additive");
        graph.add_node_parameter_edge(sequence.id, "Pose", layered.id, "BasePose");
        graph.add_node_parameter_edge(evaluator.id, "Pose", layered.id, "BlendPoses_0");
        graph.add_node_parameter_edge(sequence.id, "Pose", inactive_additive.id, "Base");
        graph.add_node_parameter_edge(sequence.id, "Pose", scaled_missing.id, "Source Pose");
        let mut graphs = Assets::<AnimationGraph>::default();
        let handle = graphs.add(graph);
        let mut world = World::new();
        world.insert_resource(graphs);
        world.insert_resource(clips);
        world.insert_resource(skeletons);
        world.insert_resource(Assets::<StateMachine>::default());
        for time in [-0.5, 0.25, 0.75, 1.5] {
            world.run_system_once_with(query, (handle.clone(), time)).unwrap();
        }
        let actual = world.run_system_once_with(query, (handle, 0.5)).unwrap();
        for (context, (foot, gait)) in actual.iter().zip([
            (-0.7, 1.0),
            (-0.25, 1.0),
            (-0.35, 0.5),
            (-0.5875, 1.0),
            (-0.7625, 1.25),
            (-0.25, 1.0),
            (-0.7, 1.0),
            (-0.7, 1.0),
        ]) {
            assert!(context.curves_are_authoritative);
            assert!(
                (context.curves[&crate::extensions::SourceCurveName::from("FootPlanted")] - foot).abs() < 2e-5,
                "{:?}",
                context.curves
            );
            assert!((context.curves[&crate::extensions::SourceCurveName::from("PoseGait")] - gait).abs() < 1e-6);
        }
        assert_eq!(actual[7].curves[&crate::extensions::SourceCurveName::from("Missing")], 0.0);
    }

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
        let arm = curves.0.get(&crate::extensions::SourceCurveName::from("LayerArmRight")).unwrap();
        assert_eq!(arm.times.len(), 3);
        assert_eq!(arm.interps[1], AlsKeyInterp::Constant);
        let foot = curves.0.get(&crate::extensions::SourceCurveName::from("FootPlanted")).unwrap();
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
        assert!(
            (s(0.7) - 1.0).abs() < 1e-6,
            "constant holds through segment"
        );
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
    fn post_cycle_wraps_inside_the_first_out_of_range_period() {
        let curve = AlsFloatCurve {
            times: vec![0.0, 1.0],
            values: vec![0.0, 1.0],
            interps: vec![AlsKeyInterp::Linear, AlsKeyInterp::Linear],
            pre_infinity: AlsExtrapolation::Cycle,
            post_infinity: AlsExtrapolation::Cycle,
            ..Default::default()
        };
        assert!((curve.sample(1.25) - 0.25).abs() < 1e-6);
        assert!((curve.sample(2.25) - 0.25).abs() < 1e-6);
        assert!((curve.sample(-0.25) - 0.75).abs() < 1e-6);
        assert!((curve.sample(-1.25) - 0.75).abs() < 1e-6);
        assert!((curve.sample(2.0) - 1.0).abs() < 1e-6);
        assert!((curve.sample(-1.0)).abs() < 1e-6);
    }

    #[test]
    fn cycle_with_offset_accumulates_endpoint_delta() {
        let curve = AlsFloatCurve {
            times: vec![0.0, 1.0],
            values: vec![0.0, 1.0],
            interps: vec![AlsKeyInterp::Linear, AlsKeyInterp::Linear],
            pre_infinity: AlsExtrapolation::CycleWithOffset,
            post_infinity: AlsExtrapolation::CycleWithOffset,
            ..Default::default()
        };
        assert!((curve.sample(1.25) - 1.25).abs() < 1e-6);
        assert!((curve.sample(2.25) - 2.25).abs() < 1e-6);
        assert!((curve.sample(-0.25) + 0.25).abs() < 1e-6);
        assert!((curve.sample(-1.25) + 1.25).abs() < 1e-6);
    }

    #[test]
    fn oscillate_reverses_every_other_period() {
        let curve = AlsFloatCurve {
            times: vec![0.0, 1.0],
            values: vec![0.0, 1.0],
            interps: vec![AlsKeyInterp::Linear, AlsKeyInterp::Linear],
            pre_infinity: AlsExtrapolation::Oscillate,
            post_infinity: AlsExtrapolation::Oscillate,
            ..Default::default()
        };
        assert!((curve.sample(1.25) - 0.75).abs() < 1e-6);
        assert!((curve.sample(2.25) - 0.25).abs() < 1e-6);
        assert!((curve.sample(-0.25) - 0.25).abs() < 1e-6);
        assert!((curve.sample(-1.25) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn parser_distinguishes_extended_extrapolation_modes() {
        let offset = "RCCE_CycleWithOffset".to_owned();
        let oscillate = "RCCE_Oscillate".to_owned();
        assert_eq!(
            parse_extrapolation(Some(&offset)),
            AlsExtrapolation::CycleWithOffset
        );
        assert_eq!(
            parse_extrapolation(Some(&oscillate)),
            AlsExtrapolation::Oscillate
        );
    }

    #[test]
    fn single_key_curve_is_constant() {
        let one = AlsFloatCurve {
            times: vec![0.4],
            values: vec![0.7],
            interps: vec![],
            pre_infinity: AlsExtrapolation::default(),
            post_infinity: AlsExtrapolation::default(),
            ..Default::default()
        };
        assert!((one.sample(0.0) - 0.7).abs() < 1e-6);
        assert!((one.sample(10.0) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn cubic_segments_preserve_exported_tangents() {
        let curve = AlsFloatCurve {
            times: vec![0.0, 1.0],
            values: vec![0.0, 1.0],
            interps: vec![AlsKeyInterp::Cubic, AlsKeyInterp::Cubic],
            arrive_tangents: vec![0.0, 2.0],
            leave_tangents: vec![2.0, 0.0],
            ..Default::default()
        };
        assert!((curve.sample(0.25) - 0.34375).abs() < 1e-6);
        assert!((curve.sample(0.75) - 0.65625).abs() < 1e-6);
    }

    #[test]
    fn cubic_segments_follow_weighted_tangent_handles() {
        let curve = AlsFloatCurve {
            times: vec![0.0, 1.0],
            values: vec![0.0, 1.0],
            interps: vec![AlsKeyInterp::Cubic, AlsKeyInterp::Cubic],
            weight_modes: vec![2, 1],
            arrive_tangents: vec![0.0, 0.0],
            leave_tangents: vec![1.0, 0.0],
            arrive_weights: vec![0.0, 0.75],
            leave_weights: vec![1.0606602, 0.0],
            ..Default::default()
        };
        assert!((curve.sample(0.5) - 0.78125).abs() < 1e-5);
    }

    #[test]
    fn parser_keeps_cubic_interpolation_and_tangent_arrays() {
        let value: serde_json::Value = serde_json::json!({
            "curves": [{
                "name": "Cubic",
                "interps": ["CUBIC", "CUBIC"],
                "times": [0.0, 1.0],
                "values": [0.0, 1.0],
                "tangent_modes": [1, 2],
                "weight_modes": [0, 0],
                "arrive_tangents": [0.0, 2.0],
                "leave_tangents": [2.0, 0.0],
                "arrive_weights": [0.0, 0.0],
                "leave_weights": [0.0, 0.0]
            }]
        });
        let curves = parse_curves(&value).unwrap();
        let curve = curves.0.get(&crate::extensions::SourceCurveName::from("Cubic")).unwrap();
        assert_eq!(curve.interps[0], AlsKeyInterp::Cubic);
        assert_eq!(curve.tangent_modes, vec![1, 2]);
        assert_eq!(curve.weight_modes, vec![0, 0]);
        assert_eq!(curve.arrive_tangents, vec![0.0, 2.0]);
        assert_eq!(curve.leave_tangents, vec![2.0, 0.0]);
        assert_eq!(curve.arrive_weights, vec![0.0, 0.0]);
        assert_eq!(curve.leave_weights, vec![0.0, 0.0]);
        assert!((curve.sample(0.25) - 0.34375).abs() < 1e-6);
    }
}
