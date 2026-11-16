use bevy::ecs::system::Commands;
use bevy::prelude::{ChildOf, Component, Name, Transform};

pub use bevy_control_rig::ik::{
    BasicTwoBoneSolve, PoleVector, StableTwoBonePole, TwoBoneSolve, blend_ik_rotation,
    calculate_pole_vector, child_world_rotation_after_parent_rotation, scaled_initial_bone_length,
    solve_basic_two_bone_ik, solve_two_bone_ik, world_rotation_to_local,
};

#[derive(Component, Debug, Default, Clone, Copy)]
pub struct IkHandTarget;

pub const IK_HAND_GUN_NAME: &str = "ik_hand_gun";

pub fn spawn_ik_hand_gun(
    cmds: &mut Commands,
    parent_bone: bevy::prelude::Entity,
) -> bevy::prelude::Entity {
    cmds.spawn((
        IkHandTarget,
        Name::new(IK_HAND_GUN_NAME),
        Transform::from_xyz(0.10, 0.02, 0.25),
        ChildOf(parent_bone),
    ))
    .id()
}

#[cfg(test)]
mod hand_target_tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::prelude::*;

    #[test]
    fn spawn_parents_named_attach_point_under_hand_bone() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let hand = app
            .world_mut()
            .spawn((Name::new("hand_r"), Transform::IDENTITY))
            .id();
        let target = {
            let mut cmds = app.world_mut().commands();
            spawn_ik_hand_gun(&mut cmds, hand)
        };
        app.update();

        assert_eq!(
            app.world().get::<Name>(target).unwrap().as_str(),
            IK_HAND_GUN_NAME
        );
        assert!(app.world().get::<IkHandTarget>(target).is_some());
        let child_of = app.world().get::<ChildOf>(target).unwrap();
        assert_eq!(child_of.0, hand, "attach point parented to hand bone");
        assert!(
            app.world()
                .get::<Transform>(target)
                .unwrap()
                .translation
                .distance(Vec3::new(0.10, 0.02, 0.25))
                < 1e-5
        );
    }
}
