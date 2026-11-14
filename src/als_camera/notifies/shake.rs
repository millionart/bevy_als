use std::{collections::hash_map::RandomState, hash::BuildHasher};

use bevy::prelude::*;

use super::{
    AlsCameraAnimationFrame, AlsCameraDebugInfo, AlsCameraDefinition, AlsCameraDefinitionHandle,
    AlsCameraFieldOfView, AlsCameraShakeRequest, AlsCameraViewInfo, AlsCameraViewSource,
    AlsCharacterCamera, CameraShakeDefinition, set_field_of_view,
};

#[derive(Component, Debug)]
pub struct AlsCameraShakes {
    active: Vec<CameraShakeInstance>,
    nonce: u64,
}

impl Default for AlsCameraShakes {
    fn default() -> Self {
        Self {
            active: Vec::new(),
            nonce: RandomState::new().hash_one(0_u8).max(1),
        }
    }
}

#[derive(Debug)]
struct CameraShakeInstance {
    definition: CameraShakeDefinition,
    elapsed: f32,
    blend_in_offset: f32,
    scale: f32,
    phases: [f32; 7],
}

impl AlsCameraShakes {
    fn start(&mut self, definition: &CameraShakeDefinition, scale: f32) {
        if definition.single_instance
            && let Some(instance) = self.active.first_mut()
        {
            let settings = &instance.definition;
            instance.blend_in_offset = if settings.blend_out <= 0.0 {
                settings.blend_in
            } else if settings.duration > 0.0
                && instance.elapsed > settings.duration - settings.blend_out
            {
                settings.blend_in * (settings.duration - instance.elapsed) / settings.blend_out
            } else {
                0.0
            };
            instance.elapsed = 0.0;
            instance.scale = scale;
            return;
        }
        let [x, y, z] = definition.location;
        let [yaw, pitch, roll] = definition.rotation_yxz;
        let waves = [x, y, z, yaw, pitch, roll, definition.field_of_view];
        let mut phases = [0.0; 7];
        for index in [2, 0, 1, 4, 3, 5, 6] {
            if waves[index].random_start {
                self.nonce ^= self.nonce << 13;
                self.nonce ^= self.nonce >> 7;
                self.nonce ^= self.nonce << 17;
                phases[index] =
                    (self.nonce as f64 / u64::MAX as f64) as f32 * std::f32::consts::TAU;
            }
        }
        self.active.push(CameraShakeInstance {
            definition: definition.clone(),
            elapsed: 0.0,
            blend_in_offset: 0.0,
            scale,
            phases,
        });
    }
}

impl CameraShakeInstance {
    fn update(&mut self, delta_seconds: f32) -> Option<(Vec3, Vec3, f32)> {
        let settings = &self.definition;
        let mut values = [0.0; 7];
        for (index, wave) in settings
            .location
            .iter()
            .chain(&settings.rotation_yxz)
            .chain(std::iter::once(&settings.field_of_view))
            .enumerate()
        {
            if wave.amplitude != 0.0 {
                self.phases[index] += delta_seconds * wave.frequency_hz * std::f32::consts::TAU;
                values[index] = wave.amplitude * self.phases[index].sin();
            }
        }
        self.elapsed += delta_seconds;
        if settings.duration > 0.0 && (self.elapsed < 0.0 || self.elapsed >= settings.duration) {
            return None;
        }
        let blend_in = if settings.blend_in > 0.0 {
            ((self.elapsed + self.blend_in_offset) / settings.blend_in).min(1.0)
        } else {
            1.0
        };
        let blend_out = if settings.duration > 0.0 && settings.blend_out > 0.0 {
            ((settings.duration - self.elapsed) / settings.blend_out).min(1.0)
        } else {
            1.0
        };
        let scale = blend_in * blend_out * self.scale;
        Some((
            Vec3::new(values[0], values[1], values[2]) * scale,
            Vec3::new(values[3], values[4], values[5]) * scale,
            values[6] * scale,
        ))
    }
}

fn normalized_angle(angle: f32) -> f32 {
    let angle = angle.rem_euclid(std::f32::consts::TAU);
    if angle > std::f32::consts::PI {
        angle - std::f32::consts::TAU
    } else {
        angle
    }
}

pub(super) fn apply_camera_shakes(
    mut commands: Commands,
    transform_helper: bevy::transform::helper::TransformHelper,
    mut requests: MessageReader<AlsCameraShakeRequest>,
    time: Res<Time<Virtual>>,
    definitions: Res<Assets<AlsCameraDefinition>>,
    owners: Query<&AlsCameraDefinitionHandle>,
    valid_view_owners: Query<
        (),
        (
            With<GlobalTransform>,
            With<crate::als::motion::AlsViewYaw>,
            With<crate::als::state::Stance>,
        ),
    >,
    frames: Query<&AlsCameraAnimationFrame>,
    component_views: Query<&AlsCameraViewInfo>,
    mut views: Query<
        (
            Entity,
            &AlsCharacterCamera,
            &mut AlsCameraShakes,
            &Transform,
            &mut Projection,
            &mut AlsCameraFieldOfView,
            Option<&AlsCameraViewSource>,
            Option<&ChildOf>,
        ),
        With<Camera3d>,
    >,
    mut diagnostics: Query<&mut AlsCameraDebugInfo>,
) {
    let requests: Vec<_> = requests.read().copied().collect();
    for (
        entity,
        owner,
        mut shakes,
        current_transform,
        mut projection,
        mut field_of_view,
        source,
        parent,
    ) in &mut views
    {
        let Ok(handle) = owners.get(owner.character) else {
            continue;
        };
        let Some(definition) = definitions.get(&handle.0) else {
            continue;
        };
        let mut transform = *current_transform;
        for request in requests
            .iter()
            .filter(|request| request.target == owner.character)
        {
            shakes.start(&definition.sprint_shake, request.scale);
        }
        let before = transform;
        let mut fov = field_of_view.0;
        shakes.active.retain_mut(|instance| {
            let Some((location, mut rotation, fov_offset)) = instance.update(time.delta_secs())
            else {
                return false;
            };
            let (_, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            let pitch = normalized_angle(pitch);
            let limit = 89.9_f32.to_radians();
            rotation.y =
                normalized_angle(pitch + normalized_angle(rotation.y)).clamp(-limit, limit) - pitch;
            let orientation = transform.rotation;
            transform.translation += orientation * location;
            transform.rotation =
                orientation * Quat::from_euler(EulerRot::YXZ, rotation.x, rotation.y, rotation.z);
            fov += fov_offset;
            true
        });
        set_field_of_view(
            &mut projection,
            &mut field_of_view,
            fov,
            definition.view_aspect_ratio,
        );
        if let Some(source) = source
            && let Ok(mut debug) = diagnostics.get_mut(source.0)
        {
            debug.shake_location_offset = transform.translation - before.translation;
            let (yaw, pitch, roll) =
                (before.rotation.inverse() * transform.rotation).to_euler(EulerRot::YXZ);
            debug.shake_rotation_degrees =
                Vec3::new(pitch.to_degrees(), -yaw.to_degrees(), -roll.to_degrees());
        }
        if let Some(parent) = parent
            && valid_view_owners.contains(owner.character)
            && source.is_some_and(|source| {
                frames.get(source.0).is_ok_and(|frame| {
                    !frame.active || component_views.get(source.0).is_ok_and(|view| view.valid)
                })
            })
        {
            let Ok(world) = transform_helper.compute_global_transform(parent.parent()) else {
                continue;
            };
            transform = GlobalTransform::from(transform).reparented_to(&world);
        }
        commands.entity(entity).insert(transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> CameraShakeDefinition {
        let definition: AlsCameraDefinition = ron::from_str(include_str!(
            "../../../assets/ALSCamera/Data/als_camera.camera_component.ron"
        ))
        .unwrap();
        definition.sprint_shake
    }

    #[test]
    fn source_shake_weights_multiply_and_restart_preserves_wave_phase() {
        let mut settings = definition();
        settings.single_instance = true;
        settings.duration = 0.15;
        settings.field_of_view.amplitude = 3.0;
        settings.field_of_view.frequency_hz = 0.25;
        for wave in settings
            .location
            .iter_mut()
            .chain(&mut settings.rotation_yxz)
            .chain(std::iter::once(&mut settings.field_of_view))
        {
            wave.random_start = false;
        }
        let mut shakes = AlsCameraShakes::default();
        shakes.start(&settings, 1.0);
        let (location, _, fov) = shakes.active[0].update(0.075).unwrap();
        assert!((location.x - 0.01 * (std::f32::consts::TAU * 0.075).sin() * 0.5625).abs() < 1e-8);
        assert!((fov - 3.0 * (std::f32::consts::TAU * 0.25 * 0.075).sin() * 0.5625).abs() < 1e-7);
        let phases = shakes.active[0].phases;
        shakes.start(&settings, 0.5);
        assert_eq!(shakes.active.len(), 1);
        assert_eq!(shakes.active[0].phases, phases);
        assert_eq!(shakes.active[0].elapsed, 0.0);
        let (location, _, _) = shakes.active[0].update(0.025).unwrap();
        assert!((location.x - 0.01 * (std::f32::consts::TAU * 0.1).sin() * 0.5).abs() < 1e-8);
        assert!(shakes.active[0].update(0.2).is_none());
        settings.duration = 0.0;
        settings.blend_out = 0.0;
        let mut infinite = AlsCameraShakes::default();
        infinite.start(&settings, 1.0);
        assert!(infinite.active[0].update(3.0).is_some());
        infinite.start(&settings, 1.0);
        assert_eq!(infinite.active[0].blend_in_offset, settings.blend_in);
    }

    #[test]
    fn source_shake_initial_offsets_are_independent_and_zero_amplitude_keeps_its_phase() {
        let settings = definition();
        let mut shakes = AlsCameraShakes {
            active: Vec::new(),
            nonce: 1,
        };
        shakes.start(&settings, 0.4);
        shakes.start(&settings, 0.7);
        assert_eq!(shakes.active.len(), 2);
        assert_ne!(shakes.active[0].phases, shakes.active[1].phases);
        assert!(
            shakes.active[0]
                .phases
                .iter()
                .all(|value| (0.0..=std::f32::consts::TAU).contains(value))
        );
        let phases = shakes.active[0].phases;
        shakes.active[0].update(0.03).unwrap();
        assert_ne!(shakes.active[0].phases[0], phases[0]);
        assert_eq!(shakes.active[0].phases[6], phases[6]);
    }

    #[test]
    fn source_shake_pitch_limit_preserves_the_view_boundary_and_applies_fov() {
        use bevy::ecs::system::RunSystemOnce;
        let mut world = World::new();
        world.init_resource::<Assets<AlsCameraDefinition>>();
        world.init_resource::<Messages<AlsCameraShakeRequest>>();
        world.init_resource::<Time<Virtual>>();
        world
            .resource_mut::<Time<Virtual>>()
            .advance_by(std::time::Duration::from_millis(20));
        let definition_handle = world.resource_mut::<Assets<AlsCameraDefinition>>().add(
            ron::from_str::<AlsCameraDefinition>(include_str!(
                "../../../assets/ALSCamera/Data/als_camera.camera_component.ron"
            ))
            .unwrap(),
        );
        let character = world
            .spawn(AlsCameraDefinitionHandle(definition_handle))
            .id();
        let mut settings = definition();
        settings.blend_in = 0.0;
        settings.blend_out = 0.0;
        for wave in &mut settings.location {
            wave.amplitude = 0.0;
        }
        for wave in &mut settings.rotation_yxz {
            wave.amplitude = 0.0;
            wave.frequency_hz = 0.0;
        }
        settings.rotation_yxz[1].amplitude = 0.1;
        settings.field_of_view.amplitude = 4.0;
        settings.field_of_view.frequency_hz = 0.0;
        let shakes = AlsCameraShakes {
            nonce: 1,
            active: vec![CameraShakeInstance {
                definition: settings,
                elapsed: 0.0,
                blend_in_offset: 0.0,
                scale: 1.0,
                phases: [std::f32::consts::FRAC_PI_2; 7],
            }],
        };
        let orientation = Quat::from_euler(EulerRot::YXZ, 0.3, 89.9_f32.to_radians(), 0.0);
        let view = world
            .spawn((
                Camera3d::default(),
                Transform::from_rotation(orientation),
                AlsCharacterCamera { character },
                AlsCameraFieldOfView(90.0),
                shakes,
            ))
            .id();
        world.run_system_once(apply_camera_shakes).unwrap();
        let actual = world.get::<Transform>(view).unwrap().rotation;
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            assert!(
                (actual * axis).distance(orientation * axis) < 2e-7,
                "{orientation:?} -> {actual:?}"
            );
        }
        assert_eq!(world.get::<AlsCameraFieldOfView>(view).unwrap().0, 94.0);
    }
}
