use std::{num::NonZeroU16, time::Duration};

use bevy::prelude::*;
use bevy_animation_graph::core::{
    animated_scene::AnimatedSceneInstance,
    animation_graph_player::AnimationGraphPlayer,
    plugin::AnimationGraphSet,
    pose::{BonePose, Pose},
};

use crate::{
    AlsAnimationLod,
    animation_lod::AlsAnimationLodSet,
    animation_visibility::AlsAnimationVisibilitySet,
    character_assembly::{AlsCharacterAvatar, AlsPlayerCharacter},
    extensions::post_physics_animation::AnimationPoseUpdateGate,
};

/// Distance and graph evaluation interval for a visible character.
#[derive(Clone, Copy, Debug)]
pub struct AlsAnimationRateTier {
    pub minimum_distance: f32,
    pub update_interval: NonZeroU16,
}

/// Update rate and an optional adaptive frame budget for animation evaluation work.
#[derive(Resource, Clone, Debug)]
pub struct AlsAnimationRateSettings {
    pub enabled: bool,
    pub tiers: Vec<AlsAnimationRateTier>,
    pub interpolate: bool,
    pub cpu_budget_ms: Option<f32>,
    /// Permits the CPU budget to bypass the rig on throttled background characters.
    pub reduce_rig_under_budget: bool,
    pub maximum_budget_interval: NonZeroU16,
}

impl Default for AlsAnimationRateSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            tiers: vec![
                AlsAnimationRateTier {
                    minimum_distance: 32.0,
                    update_interval: NonZeroU16::new(3).unwrap(),
                },
                AlsAnimationRateTier {
                    minimum_distance: 72.0,
                    update_interval: NonZeroU16::new(6).unwrap(),
                },
            ],
            interpolate: true,
            cpu_budget_ms: None,
            reduce_rig_under_budget: false,
            maximum_budget_interval: NonZeroU16::new(12).unwrap(),
        }
    }
}

impl AlsAnimationRateSettings {
    pub(crate) fn distance_interval(&self, distance: f32) -> u16 {
        self.tiers
            .iter()
            .rev()
            .find(|tier| distance >= tier.minimum_distance)
            .map_or(1, |tier| tier.update_interval.get())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BudgetEntry {
    entity: Entity,
    significance: f32,
    pub(crate) interval: u16,
    pub(crate) estimated_ms: f32,
    interpolation_ms: f32,
    interpolate: bool,
    pub(crate) mandatory: bool,
}

impl BudgetEntry {
    fn average_ms(&self) -> f32 {
        let skipped_ms = if self.interpolate {
            self.interpolation_ms
        } else {
            0.0
        };
        let frequency = 1.0 / f32::from(self.interval);
        self.estimated_ms * frequency + skipped_ms * (1.0 - frequency)
    }

    fn stretch(&mut self, excess_ms: f32, maximum: u16) {
        if self.interval >= maximum {
            return;
        }
        let interpolation = if self.interpolate {
            self.interpolation_ms
        } else {
            0.0
        };
        let per_update = (self.estimated_ms - interpolation).max(0.0);
        let target =
            (self.average_ms() - interpolation - excess_ms).max(per_update / f32::from(maximum));
        if target > 0.0 {
            self.interval = ((per_update / target).ceil() as u16).clamp(self.interval, maximum);
        }
    }
}

/// Overrides camera distance as the budget priority for an owner or animation player.
#[derive(Component, Clone, Copy, Debug)]
pub struct AlsAnimationSignificance(pub f32);

/// Entries are ordered from highest to lowest significance.
pub(crate) fn adjust_intervals(
    entries: &mut [BudgetEntry],
    budget_ms: Option<f32>,
    maximum: u16,
) -> f32 {
    let Some(budget) = budget_ms.filter(|value| value.is_finite() && *value >= 0.0) else {
        return 0.0;
    };
    let mut estimated: f32 = entries.iter().map(BudgetEntry::average_ms).sum();
    for entry in entries.iter_mut().rev() {
        if estimated <= budget {
            break;
        }
        if entry.mandatory {
            continue;
        }
        let previous = entry.average_ms();
        entry.stretch(estimated - budget, maximum);
        estimated += entry.average_ms() - previous;
    }
    for entry in entries.iter_mut().rev() {
        if estimated <= budget {
            break;
        }
        if !entry.mandatory && entry.interpolate {
            let previous = entry.average_ms();
            entry.interpolate = false;
            let excess = (estimated + entry.average_ms() - previous) - budget;
            if excess > 0.0 {
                entry.stretch(excess, maximum);
            }
            estimated += entry.average_ms() - previous;
        }
    }
    estimated / budget.max(0.0001)
}

#[derive(Component)]
pub struct AlsAnimationRate {
    phase: Option<u64>,
    interval: u16,
    scheduled: bool,
    skipped: u16,
    carried_delta: f32,
    interpolation: bool,
    previous_pose: Option<Pose>,
    displayed_step: u16,
    pub(crate) updated_this_frame: bool,
    pub(crate) measure_cost: bool,
    pub(crate) visible: bool,
    estimated_ms: f32,
    writeback_ms: f32,
    interpolation_ms: f32,
    writeback_sample: Duration,
    reduced_rig: bool,
}

impl Default for AlsAnimationRate {
    fn default() -> Self {
        Self {
            phase: None,
            interval: 1,
            scheduled: true,
            skipped: 0,
            carried_delta: 0.0,
            interpolation: false,
            previous_pose: None,
            displayed_step: 1,
            updated_this_frame: true,
            measure_cost: false,
            visible: false,
            estimated_ms: 0.1,
            writeback_ms: 0.0,
            interpolation_ms: 0.0,
            writeback_sample: Duration::ZERO,
            reduced_rig: false,
        }
    }
}

impl AlsAnimationRate {
    pub fn interval(&self) -> u16 {
        self.interval
    }

    pub(crate) fn estimated_ms(&self) -> f32 {
        self.estimated_ms + self.writeback_ms
    }

    pub(crate) fn record_writeback(&mut self, started: std::time::Instant) {
        self.writeback_sample += started.elapsed();
    }

    fn finish_writeback_sample(&mut self) {
        let measured = std::mem::take(&mut self.writeback_sample).as_secs_f32() * 1000.0;
        if measured > 0.0 {
            let estimate = if self.updated_this_frame {
                &mut self.writeback_ms
            } else {
                &mut self.interpolation_ms
            };
            *estimate += (measured - *estimate) * 0.1;
        }
    }

    pub(crate) fn schedule(
        &mut self,
        frame: u64,
        next_phase: &mut u64,
        interval: u16,
        interpolate: bool,
        measure_cost: bool,
    ) {
        self.interval = interval;
        self.interpolation = interval > 1 && interpolate;
        if !self.interpolation {
            self.previous_pose = None;
        }
        self.measure_cost = measure_cost;
        self.updated_this_frame = false;
        if interval == 1 {
            self.scheduled = true;
        } else {
            let phase = *self.phase.get_or_insert_with(|| {
                let phase = *next_phase;
                *next_phase = next_phase.wrapping_add(1);
                phase
            });
            self.scheduled = frame
                .wrapping_add(phase)
                .is_multiple_of(u64::from(interval))
                || self.skipped >= interval - 1;
        }
    }

    pub(crate) fn cancel_inactive(&mut self) {
        self.skipped = 0;
        self.carried_delta = 0.0;
        self.previous_pose = None;
    }

    pub(crate) fn step(&mut self, delta: f32, initialize: bool) -> Option<f32> {
        if !initialize && !self.scheduled {
            self.skipped += 1;
            self.displayed_step = self.displayed_step.saturating_add(1);
            self.carried_delta += delta;
            return None;
        }
        self.skipped = 0;
        let elapsed = delta + std::mem::take(&mut self.carried_delta);
        Some(elapsed)
    }

    pub(crate) fn needs_previous_pose(&self) -> bool {
        self.interpolation
    }

    pub(crate) fn publish(&mut self, previous_pose: Option<Pose>, elapsed: Option<Duration>) {
        self.previous_pose = previous_pose;
        self.displayed_step = 1;
        self.updated_this_frame = true;
        if let Some(elapsed) = elapsed {
            let measured = elapsed.as_secs_f32() * 1000.0;
            self.estimated_ms += (measured - self.estimated_ms) * 0.1;
        }
    }

    pub(crate) fn pose_to_publish(&self) -> bool {
        self.updated_this_frame || self.interval == 1 || self.interpolation
    }

    pub(crate) fn interpolate_bone(
        &self,
        pose: &Pose,
        bone: bevy_animation_graph::core::id::BoneId,
        target: &BonePose,
    ) -> Option<BonePose> {
        if !self.interpolation || self.displayed_step >= self.interval {
            return None;
        }
        let previous = self
            .previous_pose
            .as_ref()
            .filter(|previous| previous.skeleton == pose.skeleton)?;
        let mut blend = previous.get_bone(bone)?.clone();
        blend.linear_blend_mut(
            target,
            f32::from(self.displayed_step) / f32::from(self.interval),
        );
        Some(blend)
    }
}

pub(crate) struct AlsAnimationRatePlugin;

impl Plugin for AlsAnimationRatePlugin {
    fn build(&self, app: &mut App) {
        crate::extensions::post_physics_animation::require_animation_player_component::<AlsAnimationRate>(app);
        app.init_resource::<AlsAnimationRateSettings>()
            .add_observer(
                |event: On<Discard<AnimationGraphPlayer>>, mut commands: Commands| {
                    commands
                        .entity(event.entity)
                        .try_remove::<AlsAnimationRate>();
                },
            )
            .add_systems(
                PostUpdate,
                select_animation_rates
                    .after(AlsAnimationLodSet)
                    .after(AlsAnimationVisibilitySet)
                    .before(AnimationGraphSet::PrePhysics),
            );
    }
}

#[derive(Default)]
struct AnimationRatePlan {
    frame: u64,
    next_phase: u64,
    entries: Vec<BudgetEntry>,
}

fn select_animation_rates(
    settings: Res<AlsAnimationRateSettings>,
    avatars: Query<(&AlsCharacterAvatar, &AnimatedSceneInstance)>,
    local_players: Query<(), With<AlsPlayerCharacter>>,
    priorities: Query<&AlsAnimationSignificance>,
    mut players: Query<(
        &AnimationGraphPlayer,
        &mut AlsAnimationLod,
        &mut AlsAnimationRate,
        Option<&AnimationPoseUpdateGate>,
    )>,
    mut plan: Local<AnimationRatePlan>,
) {
    let plan = &mut *plan;
    plan.frame = plan.frame.wrapping_add(1);
    plan.entries.clear();
    let budgets = settings
        .cpu_budget_ms
        .filter(|budget| budget.is_finite() && *budget >= 0.0);
    for (avatar, instance) in &avatars {
        let entity = instance.player_entity();
        let Ok((player, lod, mut rate, gate)) = players.get_mut(entity) else {
            continue;
        };
        rate.finish_writeback_sample();
        let distance = lod.camera_distance();
        let root_motion = player.get_root_motion().is_some_and(|delta| {
            delta.translation != Vec3::ZERO || delta.rotation.dot(Quat::IDENTITY).abs() < 1.0
        });
        let critical = lod.requires_continuous_pose() || root_motion;
        if critical {
            rate.cancel_inactive();
        }
        let updates = gate.is_none_or(|gate| gate.update_graph);
        let can_throttle = (settings.enabled || budgets.is_some())
            && rate.visible
            && updates
            && distance.is_some()
            && !critical
            && local_players.get(avatar.character).is_err();
        let interval = if can_throttle && settings.enabled {
            settings.distance_interval(distance.unwrap())
        } else {
            1
        };
        plan.entries.push(BudgetEntry {
            entity,
            significance: priorities
                .get(entity)
                .or_else(|_| priorities.get(avatar.character))
                .ok()
                .map(|value| value.0)
                .filter(|value| value.is_finite())
                .unwrap_or_else(|| distance.map_or(0.0, |distance| 1.0 / (1.0 + distance))),
            interval,
            estimated_ms: if updates { rate.estimated_ms() } else { 0.0 },
            interpolation_ms: rate.interpolation_ms,
            interpolate: settings.interpolate,
            mandatory: !can_throttle,
        });
    }
    plan.entries.sort_unstable_by(|left, right| {
        right
            .significance
            .total_cmp(&left.significance)
            .then_with(|| left.entity.cmp(&right.entity))
    });
    let maximum = settings.maximum_budget_interval.get();
    let pressure = adjust_intervals(&mut plan.entries, budgets, maximum);
    for entry in &plan.entries {
        if let Ok((_, mut lod, mut rate, _)) = players.get_mut(entry.entity) {
            let reduced_rig = settings.reduce_rig_under_budget
                && budgets.is_some()
                && !entry.mandatory
                && (entry.interval >= maximum && pressure > 1.0
                    || rate.reduced_rig && pressure > 0.8);
            rate.reduced_rig = reduced_rig;
            if lod.budget_reduced_rig != reduced_rig {
                lod.budget_reduced_rig = reduced_rig;
            }
            rate.schedule(
                plan.frame,
                &mut plan.next_phase,
                entry.interval,
                entry.interpolate,
                budgets.is_some(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staggered_players_keep_their_elapsed_animation_time() {
        let mut states: Vec<_> = (0..6).map(|_| AlsAnimationRate::default()).collect();
        let mut phase = 0;
        let mut updates = [0; 6];
        let mut advanced = [0.0; 6];
        for frame in 0..60 {
            let mut due = 0;
            for (index, state) in states.iter_mut().enumerate() {
                state.schedule(frame, &mut phase, 3, false, false);
                if let Some(delta) = state.step(1.0, false) {
                    updates[index] += 1;
                    due += 1;
                    advanced[index] += delta;
                }
            }
            assert_eq!(due, 2);
        }
        assert_eq!(updates, [20; 6]);
        for (state, elapsed) in states.iter().zip(advanced) {
            assert_eq!(elapsed + state.carried_delta, 60.0);
        }
    }

    #[test]
    fn interpolation_advances_the_display_without_changing_the_sampled_pose() {
        use bevy_animation_graph::core::animation_clip::EntityPath;
        let bone = EntityPath::default().child("Bone").id();
        let mut earlier = Pose::default();
        earlier.add_bone(BonePose::from_transform(Transform::IDENTITY), bone);
        let mut latest = Pose::default();
        latest.add_bone(
            BonePose::from_transform(Transform::from_xyz(3.0, 0.0, 0.0)),
            bone,
        );
        let mut rate = AlsAnimationRate::default();
        let mut phase = 0;
        rate.schedule(0, &mut phase, 3, true, false);
        rate.publish(Some(earlier), None);
        let target = latest.get_bone(bone).unwrap();
        assert!(
            rate.interpolate_bone(&latest, bone, target)
                .unwrap()
                .translation
                .unwrap()
                .abs_diff_eq(Vec3::X, 1e-6)
        );
        rate.schedule(1, &mut phase, 3, true, false);
        assert!(rate.step(1.0, false).is_none());
        assert!(
            rate.interpolate_bone(&latest, bone, target)
                .unwrap()
                .translation
                .unwrap()
                .abs_diff_eq(Vec3::X * 2.0, 1e-6)
        );
        assert_eq!(
            latest.get_bone(bone).unwrap().translation,
            Some(Vec3::X * 3.0)
        );
    }

    #[test]
    fn visible_distant_character_uses_the_rate_policy_and_local_control_overrides_budget() {
        let (mut app, actor, avatar) = crate::roll_drive::tests::loaded_roll_character();
        let world = app.world_mut();
        world.resource_mut::<AlsAnimationRateSettings>().enabled = true;
        world.entity_mut(actor).remove::<AlsPlayerCharacter>();
        let player = world
            .get::<AnimatedSceneInstance>(avatar)
            .unwrap()
            .player_entity();
        let position = world.get::<GlobalTransform>(actor).unwrap().translation() + Vec3::Z * 40.0;
        let mut cameras = world.query::<&mut Camera>();
        for mut camera in cameras.iter_mut(world) {
            camera.is_active = false;
        }
        let transform = Transform::from_translation(position);
        world.spawn((
            Camera3d::default(),
            transform,
            GlobalTransform::from(transform),
        ));
        let mut pending = vec![avatar];
        let mut meshes = vec![];
        while let Some(entity) = pending.pop() {
            if world.get::<Mesh3d>(entity).is_some() {
                meshes.push(entity);
            }
            if let Some(children) = world.get::<Children>(entity) {
                pending.extend(children.iter());
            }
        }
        assert!(!meshes.is_empty());
        let frame = |app: &mut App| {
            for entity in &meshes {
                app.world_mut()
                    .entity_mut(*entity)
                    .insert((ViewVisibility::VISIBLE, InheritedVisibility::VISIBLE));
            }
            app.update();
            app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_context_arena()
                .unwrap()
                .frame_index()
        };
        for _ in 0..9 {
            frame(&mut app);
        }
        assert_eq!(
            app.world()
                .get::<AlsAnimationRate>(player)
                .unwrap()
                .interval(),
            3
        );
        let start = frame(&mut app);
        let mut end = start;
        for _ in 0..18 {
            end = frame(&mut app);
        }
        assert_eq!(end - start, 6);
        assert!(
            app.world()
                .get::<AnimationGraphPlayer>(player)
                .unwrap()
                .get_error()
                .is_none()
        );

        app.world_mut()
            .resource_mut::<AlsAnimationRateSettings>()
            .cpu_budget_ms = Some(0.001);
        frame(&mut app);
        assert_eq!(
            app.world()
                .get::<AlsAnimationRate>(player)
                .unwrap()
                .interval(),
            12
        );
        app.world_mut().entity_mut(actor).insert(AlsPlayerCharacter);
        frame(&mut app);
        assert_eq!(
            app.world()
                .get::<AlsAnimationRate>(player)
                .unwrap()
                .interval(),
            1
        );
    }

    #[test]
    fn budget_preserves_mandatory_and_nearby_players_first() {
        let entry = BudgetEntry {
            entity: Entity::PLACEHOLDER,
            significance: 0.0,
            interval: 1,
            estimated_ms: 4.0,
            interpolation_ms: 0.0,
            interpolate: false,
            mandatory: false,
        };
        let mut entries = [
            BudgetEntry {
                mandatory: true,
                ..entry
            },
            entry,
            entry,
            entry,
        ];
        adjust_intervals(&mut entries, Some(10.0), 8);
        assert_eq!(entries[0].interval, 1);
        assert_eq!(entries[1].interval, 1);
        assert!(entries[2].interval >= 2 && entries[3].interval >= entries[2].interval);
        let work: f32 = entries.iter().map(BudgetEntry::average_ms).sum();
        assert!(work <= 10.0);
    }

    #[test]
    fn budget_releases_interpolation_work_when_rate_reduction_is_insufficient() {
        let mut entries = [BudgetEntry {
            entity: Entity::PLACEHOLDER,
            significance: 0.0,
            interval: 1,
            estimated_ms: 8.0,
            interpolation_ms: 4.0,
            interpolate: true,
            mandatory: false,
        }];
        let pressure = adjust_intervals(&mut entries, Some(2.0), 8);
        assert!(entries[0].interval >= 4);
        assert!(!entries[0].interpolate);
        assert!(pressure <= 1.0);
    }
}
