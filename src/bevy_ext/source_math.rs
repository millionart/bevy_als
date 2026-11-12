use bevy::math::{DQuat, DVec3};

fn source_cross(a: DVec3, b: DVec3) -> DVec3 {
    #[cfg(target_arch = "aarch64")]
    {
        DVec3::new(
            (-a.z).mul_add(b.y, a.y * b.z),
            (-a.x).mul_add(b.z, a.z * b.x),
            (-a.y).mul_add(b.x, a.x * b.y),
        )
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        a.cross(b)
    }
}

pub(crate) fn source_rotate_vector(rotation: DQuat, vector: DVec3) -> DVec3 {
    let source_rotation = DVec3::new(-rotation.x, -rotation.z, -rotation.y);
    let source_vector = DVec3::new(vector.x, vector.z, vector.y);
    let cross = source_cross(source_rotation, source_vector);
    let twice = cross + cross;
    #[cfg(target_arch = "aarch64")]
    let intermediate = DVec3::new(
        rotation.w.mul_add(twice.x, source_vector.x),
        rotation.w.mul_add(twice.y, source_vector.y),
        rotation.w.mul_add(twice.z, source_vector.z),
    );
    #[cfg(not(target_arch = "aarch64"))]
    let intermediate = source_vector + rotation.w * twice;
    let result = intermediate + source_cross(source_rotation, twice);
    DVec3::new(result.x, result.z, result.y)
}
