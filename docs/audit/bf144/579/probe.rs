use approx::relative_eq;
use num_traits::Bounded;
use parry3d::math::{self, ComplexField, Pose, Real, Rotation, Vector, DIM};
use parry3d::query::{self, Ray, ShapeCastOptions};
use parry3d::query::gjk::{eps_tol, CsoPoint, VoronoiSimplex};
use parry3d::shape::{Capsule, Cuboid, SupportMap};
use serde_json::{json, Value};

fn number(row: &Value) -> f32 {
    row.as_f64().unwrap() as f32
}

fn vector(row: &Value) -> Vector {
    Vector::new(number(&row[0]),number(&row[1]),number(&row[2]))
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args.next().unwrap();
    let output = args.next().unwrap();
    let inputs: Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let mut rows = Vec::new();
    for row in inputs.as_array().unwrap() {
        let center = vector(&row["box_center"]);
        let floor = Cuboid::new(vector(&row["box_extents"]));
        let radius = number(&row["radius"]);
        let segment_half = number(&row["half_height"]) - radius;
        let capsule = Capsule::new(Vector::Y * segment_half, Vector::NEG_Y * segment_half, radius);
        let q = &row["orientation"];
        let rotation = Rotation::from_xyzw(number(&q[0]),number(&q[1]),number(&q[2]),number(&q[3]));
        let start = vector(&row["start"]);
        let direction = vector(&row["direction"]);
        let length = number(&row["length"]);
        let world_box = Pose::from_translation(center);
        let world_capsule = Pose::from_parts(start,rotation);
        let relative = world_box.inverse() * world_capsule;
        let options = ShapeCastOptions { max_time_of_impact:length,stop_at_penetration:false,..Default::default() };
        let baseline = query::cast_shapes(&world_box,Vector::ZERO,&floor,&world_capsule,direction,&capsule,options)
            .unwrap().unwrap();
        let mut trace = Vec::new();
        let replay = traced_ray_cast(&relative,&floor,&capsule,&Ray::new(Vector::ZERO,direction),
            Real::MAX,&mut VoronoiSimplex::new(),&mut trace,false).unwrap();
        let mut ablated_trace = Vec::new();
        let ablated = traced_ray_cast(&relative,&floor,&capsule,&Ray::new(Vector::ZERO,direction),
            Real::MAX,&mut VoronoiSimplex::new(),&mut ablated_trace,true);
        let ablated_toi = ablated.map(|value|value.0);
        rows.push(json!({"name":row["name"],"expected":row["expected"],"public_toi":baseline.time_of_impact,
            "public_normal":baseline.normal1.to_array(),"replay_toi":replay.0,"replay_normal":replay.1.to_array(),
            "trace":trace,"ablated_toi":ablated_toi,"ablated_trace":ablated_trace}));
        println!("Measured {}",row["name"]);
    }
    std::fs::write(output,serde_json::to_vec_pretty(&rows).unwrap()).unwrap();
}
