use crate::character_assembly::AlsCharacterAvatar;
use als_core::state::Stance;
use bevy::prelude::ChildOf;
use bevy::prelude::*;

pub const STANCE_BLEND_SPEED: f32 = 2.5;

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct AlsStanceBlend {
    pub weight: f32,
    pub trans_weight: f32,
    pub trans_up: bool,
}

impl Default for AlsStanceBlend {
    fn default() -> Self {
        Self {
            weight: 0.0,
            trans_weight: 0.0,
            trans_up: false,
        }
    }
}

impl AlsStanceBlend {
    pub fn with_weight(weight: f32) -> Self {
        Self {
            weight,
            ..Self::default()
        }
    }

    fn advance_transition(&mut self, crouching_changed: bool, up: bool, dt: f32) {
        if crouching_changed && self.trans_weight <= 0.0 {
            self.trans_weight = 0.001;
            self.trans_up = up;
        }
        if self.trans_weight > 0.0 {
            self.trans_weight += dt / TRANS_CLIP_SECS;
            if self.trans_weight >= 1.0 {
                self.trans_weight = 0.0;
            }
        }
    }
}

const TRANS_CLIP_SECS: f32 = 0.8;

pub fn advance_stance_weight(current: f32, crouching: bool, dt: f32) -> f32 {
    let target = if crouching { 1.0 } else { 0.0 };
    move_toward_f32(current, target, STANCE_BLEND_SPEED * dt)
}

fn move_toward_f32(current: f32, target: f32, max_delta: f32) -> f32 {
    if (current - target).abs() <= max_delta {
        target
    } else if current < target {
        current + max_delta
    } else {
        current - max_delta
    }
}

pub struct StanceLayerPlugin;

impl Plugin for StanceLayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, stance_blend_driver)
            .add_systems(PostUpdate, crate::character_assembly::sync_avatar_stance_offset
                .before(bevy_animation_graph::core::plugin::AnimationGraphSet::PrePhysics)
                .before(bevy::transform::TransformSystems::Propagate));
    }
}

type StanceDriverQuery = (
    Entity,
    &'static ChildOf,
    Option<&'static mut AlsStanceBlend>,
);

fn stance_blend_driver(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<StanceDriverQuery, With<AlsCharacterAvatar>>,
    stances: Query<&Stance>,
) {
    let dt = time.delta_secs();
    for (entity, parent, blend) in q.iter_mut() {
        let Ok(stance) = stances.get(parent.parent()) else {
            continue;
        };
        let crouching = *stance == Stance::Crouching;
        match blend {
            Some(mut state) => {
                let changed = (state.weight > 0.5) != crouching;
                let up = !crouching;
                state.advance_transition(changed, up, dt);
                state.weight = advance_stance_weight(state.weight, crouching, dt);
            }
            None => {
                let weight = if crouching { 1.0 } else { 0.0 };
                commands
                    .entity(entity)
                    .insert(AlsStanceBlend::with_weight(weight));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_moves_toward_crouch_target() {
        assert!((advance_stance_weight(0.0, true, 0.1) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn weight_snaps_at_target_without_overshoot() {
        assert_eq!(advance_stance_weight(0.9, true, 1.0), 1.0);
        assert_eq!(advance_stance_weight(0.1, false, 1.0), 0.0);
    }

    #[test]
    fn standing_holds_zero() {
        assert_eq!(advance_stance_weight(0.0, false, 0.5), 0.0);
    }
}
