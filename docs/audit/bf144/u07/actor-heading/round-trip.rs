use glam::{Affine3A,DQuat,EulerRot,Quat,Vec3};
fn main() {
    let yaw=-std::f32::consts::FRAC_PI_2;
    let spawn=Quat::from_rotation_y(yaw);
    let matrix=|q| Affine3A::from_scale_rotation_translation(Vec3::ONE,q,Vec3::ZERO).to_scale_rotation_translation().1;
    let rebuild=Quat::from_euler(EulerRot::YXZ,yaw,0.0,0.0);
    let half=std::f64::consts::FRAC_1_SQRT_2;
    let basis=DQuat::from_xyzw(0.,half,0.,half);
    for (name,q) in [("spawn",spawn),("spawn_mat",matrix(spawn)),("two_mats",matrix(matrix(spawn))),
                    ("rebuild",rebuild),("rebuild_mat",matrix(rebuild)),
                    ("exact_f64", DQuat::from_rotation_y(-std::f64::consts::FRAC_PI_2).as_quat())] {
        let source=q.as_dquat()*basis;
        println!("{name} {q:?} norm={} source={source:?}",q.length_squared());
    }
    let wrapped=(yaw+std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)-std::f32::consts::PI;
    println!("yaw {yaw:?} deg={} wrapped {wrapped:?} deg={} bits={} {}", yaw.to_degrees(),wrapped.to_degrees(),yaw.to_bits(),wrapped.to_bits());
}
