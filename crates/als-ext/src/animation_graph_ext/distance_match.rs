use bevy::prelude::*;
use bevy_animation_graph::core::animation_graph::TimeUpdate;
use bevy_animation_graph::core::animation_node::{NodeLike, ReflectNodeLike};
use bevy_animation_graph::core::context::{new_context::NodeContext, spec_context::SpecContext};
use bevy_animation_graph::core::edge_data::{DataSpec, DataValue};
use bevy_animation_graph::core::errors::GraphError;

use crate::curve::CurveKey;

pub const STOP_IN_POSE: &str = "pose";
pub const STOP_IN_TIME: &str = "time";
pub const STOP_OUT_POSE: &str = "pose";
pub const IN_REMAINING: &str = "remaining";
pub const IN_TOTAL: &str = "total";

fn key(t: f32, v: f32) -> CurveKey {
    CurveKey { t, v }
}

pub fn default_stop_distance_curve() -> Vec<CurveKey> {
    vec![
        key(0.00, 0.00),
        key(0.30, 0.50),
        key(0.60, 0.85),
        key(0.85, 1.00),
        key(1.00, 1.00),
    ]
}

pub fn sample_distance_curve(keys: &[CurveKey], time_norm: f32) -> f32 {
    if keys.is_empty() {
        return time_norm.clamp(0.0, 1.0);
    }
    let t = time_norm.clamp(0.0, 1.0);
    if t <= keys[0].t {
        return keys[0].v;
    }
    let last = keys[keys.len() - 1];
    if t >= last.t {
        return last.v;
    }
    for w in keys.windows(2) {
        let a = &w[0];
        let b = &w[1];
        if t >= a.t && t <= b.t {
            let span = (b.t - a.t).max(1e-6);
            let frac = (t - a.t) / span;
            return a.v + (b.v - a.v) * frac;
        }
    }
    last.v
}

pub fn invert_distance_curve(keys: &[CurveKey], walked: f32) -> f32 {
    if keys.is_empty() {
        return walked.clamp(0.0, 1.0);
    }
    let lo = keys[0].v;
    let hi = keys[keys.len() - 1].v.max(lo + 1e-6);
    let target = walked.clamp(lo, hi);
    if target <= keys[0].v {
        return keys[0].t;
    }
    for w in keys.windows(2) {
        let a = &w[0];
        let b = &w[1];
        if target <= b.v {
            let span_v = (b.v - a.v).max(1e-6);
            let frac = ((target - a.v) / span_v).clamp(0.0, 1.0);
            return a.t + (b.t - a.t) * frac;
        }
    }
    keys[keys.len() - 1].t
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct DistanceMatchNode {
    pub keys: Vec<CurveKey>,
    pub duration_fallback: f32,
}

impl DistanceMatchNode {
    pub fn with_default_curve(duration_fallback: f32) -> Self {
        Self {
            keys: default_stop_distance_curve(),
            duration_fallback,
        }
    }
}

impl NodeLike for DistanceMatchNode {
    fn display_name(&self) -> String {
        "ALS Distance Match".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let remaining = ctx.data_back(IN_REMAINING)?.as_f32()?;
        let total = ctx.data_back(IN_TOTAL)?.as_f32()?.max(1e-4);
        let walked = (1.0 - remaining / total).clamp(0.0, 1.0);
        let t_norm = invert_distance_curve(&self.keys, walked);
        let duration = ctx
            .duration_back(STOP_IN_TIME)?
            .unwrap_or(self.duration_fallback)
            .max(1e-4);
        let t_abs = t_norm * duration;

        ctx.set_time_update_back(STOP_IN_TIME, TimeUpdate::Absolute(t_abs));

        let mut in_pose = ctx.data_back(STOP_IN_POSE)?.into_pose()?;
        in_pose.timestamp = t_abs;
        ctx.set_time(t_abs);
        ctx.set_data_fwd(STOP_OUT_POSE, in_pose);

        let _ = DataValue::from(t_abs);
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(IN_REMAINING, DataSpec::F32)
            .add_input_data(IN_TOTAL, DataSpec::F32)
            .add_input_data(STOP_IN_POSE, DataSpec::Pose)
            .add_input_time(STOP_IN_TIME);
        ctx.add_output_data(STOP_OUT_POSE, DataSpec::Pose)
            .add_output_time();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve() -> Vec<CurveKey> {
        default_stop_distance_curve()
    }

    #[test]
    fn samples_monotonic_s_curve() {
        let c = curve();
        let mut prev = -1.0;
        for i in 0..=20 {
            let v = sample_distance_curve(&c, i as f32 / 20.0);
            assert!(v >= prev - 1e-6, "distance curve must be monotonic");
            assert!((0.0..=1.0).contains(&v));
            prev = v;
        }
    }

    #[test]
    fn inversion_round_trips_and_clamps_ends() {
        let c = curve();
        for i in 0..=8 {
            let t = i as f32 / 10.0;
            let d = sample_distance_curve(&c, t);
            let t_back = invert_distance_curve(&c, d);
            assert!((t_back - t).abs() < 2e-3, "invert(sample({t}))={t_back}");
        }
        assert!(invert_distance_curve(&c, -5.0) <= 1e-4);
        let plant = invert_distance_curve(&c, 1.0);
        assert!((plant - 0.85).abs() < 1e-4, "plateau inverts to plant time");
    }

    #[test]
    fn plant_point_reached_before_clip_end() {
        let c = curve();
        let plant_time = invert_distance_curve(&c, 1.0);
        assert!(
            plant_time < 1.0,
            "foot plants before clip end, got {plant_time}"
        );
        assert!(plant_time > 0.5, "plant happens late in the clip");
        let tail = sample_distance_curve(&c, (plant_time + 1.0) * 0.5);
        assert!(
            (tail - 1.0).abs() < 1e-5,
            "distance stays at max through recovery tail"
        );
    }

    #[test]
    fn empty_curve_falls_back_to_identity() {
        assert!((sample_distance_curve(&[], 0.7) - 0.7).abs() < 1e-6);
        assert!((invert_distance_curve(&[], 0.3) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::{GetTypeRegistration, TypeRegistry};
        let mut registry = TypeRegistry::default();
        registry.register::<DistanceMatchNode>();
        let reg = registry
            .get(std::any::TypeId::of::<DistanceMatchNode>())
            .unwrap();
        assert!(
            reg.data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
        assert_eq!(
            <DistanceMatchNode as bevy::reflect::TypePath>::type_path(),
            "als_ext::DistanceMatchNode"
        );
    }
}
