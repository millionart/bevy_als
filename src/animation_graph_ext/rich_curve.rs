use bevy::prelude::*;
use bevy_animation_graph::core::{
    animation_node::{NodeLike, ReflectNodeLike},
    context::{new_context::NodeContext, spec_context::SpecContext},
    edge_data::DataSpec,
    errors::GraphError,
};
use serde::{Deserialize, Serialize};

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum RichCurveInterpolation {
    Constant,
    #[default]
    Linear,
    Cubic,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum RichCurveTangentMode {
    #[default]
    Auto,
    User,
    Break,
    None,
    SmartAuto,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[reflect(Default, Serialize)]
pub enum RichCurveTangentWeightMode {
    #[default]
    None,
    Arrive,
    Leave,
    Both,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Default, Serialize)]
pub struct RichCurveKey {
    pub interpolation: RichCurveInterpolation,
    pub tangent_mode: RichCurveTangentMode,
    pub tangent_weight_mode: RichCurveTangentWeightMode,
    pub t: f32,
    pub v: f32,
    pub arrive_tangent: f32,
    pub arrive_weight: f32,
    pub leave_tangent: f32,
    pub leave_weight: f32,
}

pub(crate) fn cubic_bezier(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let lerp = |left: f32, right: f32| left + (right - left) * t;
    let p01 = lerp(p0, p1);
    let p12 = lerp(p1, p2);
    let p23 = lerp(p2, p3);
    lerp(lerp(p01, p12), lerp(p12, p23))
}

pub(crate) fn unweighted_cubic(
    start: f32,
    end: f32,
    leave_tangent: f32,
    arrive_tangent: f32,
    span: f32,
    alpha: f32,
) -> f32 {
    let one_third = 1.0 / 3.0;
    let first_control = start + leave_tangent * span * one_third;
    let second_control = end - arrive_tangent * span * one_third;
    cubic_bezier(start, first_control, second_control, end, alpha)
}

fn cubic_roots(coefficients: [f64; 4]) -> Vec<f64> {
    let denominator = if coefficients[3].abs() > f64::EPSILON {
        coefficients[3]
    } else {
        f64::EPSILON * f64::EPSILON
    };
    let a = coefficients[2] / denominator;
    let b = coefficients[1] / denominator;
    let c = coefficients[0] / denominator;
    let a_squared = a * a;
    let p = (-a_squared / 3.0 + b) / 3.0;
    let q = (2.0 * a * a_squared / 27.0 - a * b / 3.0 + c) / 2.0;
    let cube_of_p = p * p * p;
    let discriminant = q * q + cube_of_p;
    let mut roots = Vec::new();
    if discriminant.abs() <= 1e-12 {
        if q.abs() <= 1e-12 {
            roots.push(0.0);
        } else {
            let u = (-q).cbrt();
            roots.push(2.0 * u);
            roots.push(-u);
        }
    } else if discriminant < 0.0 {
        let phi = (-q / (-cube_of_p).sqrt()).acos() / 3.0;
        let t = 2.0 * (-p).sqrt();
        roots.push(t * phi.cos());
        roots.push(-t * (phi + std::f64::consts::PI / 3.0).cos());
        roots.push(-t * (phi - std::f64::consts::PI / 3.0).cos());
    } else {
        let sqrt = discriminant.sqrt();
        roots.push((sqrt - q).cbrt() - (sqrt + q).cbrt());
    }
    let shift = a / 3.0;
    for root in &mut roots {
        *root -= shift;
    }
    roots
}

fn uses_weighted_tangents(left: RichCurveKey, right: RichCurveKey) -> bool {
    !matches!(
        left.tangent_weight_mode,
        RichCurveTangentWeightMode::None | RichCurveTangentWeightMode::Arrive
    ) || !matches!(
        right.tangent_weight_mode,
        RichCurveTangentWeightMode::None | RichCurveTangentWeightMode::Leave
    )
}

fn tangent_weight(
    mode: RichCurveTangentWeightMode,
    leave: bool,
    tangent: f32,
    span: f32,
    value: f32,
) -> f32 {
    let weighted = if leave {
        matches!(
            mode,
            RichCurveTangentWeightMode::Leave | RichCurveTangentWeightMode::Both
        )
    } else {
        matches!(
            mode,
            RichCurveTangentWeightMode::Arrive | RichCurveTangentWeightMode::Both
        )
    };
    if weighted {
        value
    } else {
        (span * span + (tangent * span) * (tangent * span)).sqrt() / 3.0
    }
}

fn weighted_cubic(left: RichCurveKey, right: RichCurveKey, time: f32) -> f32 {
    let span = right.t - left.t;
    let alpha = ((time - left.t) / span).clamp(0.0, 1.0);
    let leave_angle = left.leave_tangent.atan();
    let arrive_angle = right.arrive_tangent.atan();
    let leave_weight = tangent_weight(
        left.tangent_weight_mode,
        true,
        left.leave_tangent,
        span,
        left.leave_weight,
    );
    let arrive_weight = tangent_weight(
        right.tangent_weight_mode,
        false,
        right.arrive_tangent,
        span,
        right.arrive_weight,
    );
    let first_x = left.t + leave_angle.cos() * leave_weight;
    let first_y = left.v + leave_angle.sin() * leave_weight;
    let second_x = right.t - arrive_angle.cos() * arrive_weight;
    let second_y = right.v - arrive_angle.sin() * arrive_weight;
    let normalized_first_x = (first_x - left.t) / span;
    let normalized_second_x = (second_x - left.t) / span;
    let a = normalized_first_x;
    let b = normalized_second_x - normalized_first_x;
    let c = 1.0 - normalized_second_x;
    let d = b - a;
    let coefficients = [
        -(alpha as f64),
        3.0 * a as f64,
        3.0 * d as f64,
        (c - b - d) as f64,
    ];
    let interpolation = cubic_roots(coefficients)
        .into_iter()
        .filter(|root| *root >= -1e-6 && *root <= 1.0 + 1e-6)
        .fold(None, |best, root| match best {
            Some(best) if best >= root => Some(best),
            _ => Some(root),
        })
        .unwrap_or(0.0);
    cubic_bezier(left.v, first_y, second_y, right.v, interpolation as f32)
}

pub fn sample_rich_curve(keys: &[RichCurveKey], time: f32) -> f32 {
    let Some(first) = keys.first().copied() else {
        return 0.0;
    };
    let Some(last) = keys.last().copied() else {
        return 0.0;
    };
    if time <= first.t {
        return first.v;
    }
    if time >= last.t {
        return last.v;
    }
    let upper = keys.partition_point(|key| key.t <= time);
    let left = keys[upper - 1];
    let right = keys[upper];
    let span = right.t - left.t;
    if span <= f32::EPSILON {
        return left.v;
    }
    let alpha = ((time - left.t) / span).clamp(0.0, 1.0);
    match left.interpolation {
        RichCurveInterpolation::Constant => left.v,
        RichCurveInterpolation::Linear => left.v + (right.v - left.v) * alpha,
        RichCurveInterpolation::Cubic if uses_weighted_tangents(left, right) => {
            weighted_cubic(left, right, time)
        }
        RichCurveInterpolation::Cubic => unweighted_cubic(
            left.v, right.v, left.leave_tangent, right.arrive_tangent, span, alpha,
        ),
    }
}

#[derive(Reflect, Clone, Debug, Default)]
#[reflect(Default, NodeLike)]
#[type_path = "als_ext"]
pub struct RichCurveSampleNode {
    pub keys: Vec<RichCurveKey>,
}

impl RichCurveSampleNode {
    pub const IN_POSITION: &'static str = "position";
    pub const OUT_VALUE: &'static str = "value";
}

impl NodeLike for RichCurveSampleNode {
    fn display_name(&self) -> String {
        "ALS Rich Curve Sample".into()
    }

    fn update(&self, mut ctx: NodeContext) -> Result<(), GraphError> {
        let position = ctx.data_back(Self::IN_POSITION)?.as_f32()?;
        ctx.set_data_fwd(Self::OUT_VALUE, sample_rich_curve(&self.keys, position));
        Ok(())
    }

    fn spec(&self, mut ctx: SpecContext) -> Result<(), GraphError> {
        ctx.add_input_data(Self::IN_POSITION, DataSpec::F32)
            .add_output_data(Self::OUT_VALUE, DataSpec::F32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(t: f32, v: f32) -> RichCurveKey {
        RichCurveKey { t, v, ..default() }
    }

    #[test]
    fn linear_curve_clamps_and_interpolates() {
        let keys = [key(0.2, 0.25), key(1.0, 1.0)];
        assert!((sample_rich_curve(&keys, 0.0) - 0.25).abs() < 1e-6);
        assert!((sample_rich_curve(&keys, 0.6) - 0.625).abs() < 1e-6);
        assert!((sample_rich_curve(&keys, 2.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cubic_curve_matches_unweighted_unreal_control_points() {
        let keys = [
            RichCurveKey {
                interpolation: RichCurveInterpolation::Cubic,
                tangent_mode: RichCurveTangentMode::User,
                t: 0.2,
                v: 0.25,
                arrive_tangent: 2.5235951,
                arrive_weight: 0.9048326,
                leave_tangent: 2.5235951,
                leave_weight: 0.9048368,
                ..default()
            },
            key(1.0, 1.0),
        ];
        assert!((sample_rich_curve(&keys, 0.2) - 0.25).abs() < 1e-6);
        assert!((sample_rich_curve(&keys, 1.0) - 1.0).abs() < 1e-6);
        assert!((sample_rich_curve(&keys, 0.6) - 0.8773595).abs() < 1e-5);
    }

    #[test]
    fn weighted_curve_solves_time_parameter_before_value() {
        let keys = [
            RichCurveKey {
                interpolation: RichCurveInterpolation::Cubic,
                tangent_mode: RichCurveTangentMode::User,
                tangent_weight_mode: RichCurveTangentWeightMode::Leave,
                t: 0.0,
                v: 0.0,
                leave_tangent: 1.0,
                leave_weight: 0.1,
                ..default()
            },
            RichCurveKey {
                t: 1.0,
                v: 1.0,
                ..default()
            },
        ];
        let sampled = sample_rich_curve(&keys, 0.5);
        assert!(sampled > 0.5);
        assert!(sampled < 1.0);
    }

    #[test]
    fn node_registers_node_like_dispatch_key() {
        use bevy::reflect::TypeRegistry;

        let mut registry = TypeRegistry::default();
        registry.register::<RichCurveSampleNode>();
        let registration = registry
            .get(std::any::TypeId::of::<RichCurveSampleNode>())
            .unwrap();
        assert!(
            registration
                .data::<bevy_animation_graph::core::animation_node::ReflectNodeLike>()
                .is_some()
        );
    }
}
