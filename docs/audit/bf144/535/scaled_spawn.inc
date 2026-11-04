fn c535_scale_spawn(
    event: On<Add, AlsCharacter>,
    mut actors: Query<(&mut Position, &mut Transform, &mut Collider)>,
) {
    let (mut position,mut transform,mut collider)=actors.get_mut(event.entity).unwrap();
    let old_half=collider.shape_scaled().compute_local_aabb().half_extents().y;
    transform.scale=Vec3::splat(0.3);
    collider.set_scale(transform.scale,16);
    let new_half=collider.shape_scaled().compute_local_aabb().half_extents().y;
    position.0.y += new_half-old_half;
    transform.translation.y += new_half-old_half;
}
