use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinCrouchConfig, TnuaBuiltinJump, TnuaBuiltinJumpConfig,
    TnuaBuiltinWalk, TnuaBuiltinWalkConfig,
};
use bevy_tnua::{
    TnuaAction, TnuaActionContext, TnuaActionInitiationDirective, TnuaActionLifecycleDirective,
    TnuaActionLifecycleStatus, TnuaBasis, TnuaBasisContext, TnuaMotor, TnuaScheme,
};

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum AlsControlScheme {
    Jump(AlsBuiltinJump),
    Crouch(TnuaBuiltinCrouch),
}

#[allow(clippy::derivable_impls)]
impl Default for AlsControlSchemeConfig {
    fn default() -> Self {
        Self {
            basis: TnuaBuiltinWalkConfig::default(),
            jump: TnuaBuiltinJumpConfig::default(),
            crouch: TnuaBuiltinCrouchConfig::default(),
        }
    }
}

#[derive(Default)]
pub struct AlsBuiltinJump {
    pub builtin: TnuaBuiltinJump,
    pub max_hold_time: f32,
    pub jump_z_velocity: f32,
    pub delta_seconds: f32,
}

#[derive(Default)]
pub struct AlsBuiltinJumpMemory {
    pub builtin: bevy_tnua::builtins::TnuaBuiltinJumpMemory,
    pub hold_elapsed: f32,
}

impl TnuaAction<TnuaBuiltinWalk> for AlsBuiltinJump {
    type Config = TnuaBuiltinJumpConfig;
    type Memory = AlsBuiltinJumpMemory;

    fn initiation_decision(
        &self,
        config: &Self::Config,
        sensors: &<TnuaBuiltinWalk as TnuaBasis>::Sensors<'_>,
        ctx: TnuaActionContext<TnuaBuiltinWalk>,
        being_fed_for: &bevy::time::Stopwatch,
    ) -> TnuaActionInitiationDirective {
        self.builtin
            .initiation_decision(config, sensors, ctx, being_fed_for)
    }

    fn apply(
        &self,
        config: &Self::Config,
        memory: &mut Self::Memory,
        sensors: &<TnuaBuiltinWalk as TnuaBasis>::Sensors<'_>,
        ctx: TnuaActionContext<TnuaBuiltinWalk>,
        lifecycle_status: TnuaActionLifecycleStatus,
        motor: &mut TnuaMotor,
    ) -> TnuaActionLifecycleDirective {
        if lifecycle_status.just_started() {
            memory.hold_elapsed = 0.0;
        }
        if !lifecycle_status.is_active() {
            memory.hold_elapsed = f32::INFINITY;
        }
        let holding = memory.hold_elapsed < self.max_hold_time;
        let up = ctx.up_direction.as_vec3();
        let velocity = ctx.tracker.velocity.dot(up);
        let directive = self.builtin.apply(
            config,
            &mut memory.builtin,
            sensors,
            ctx,
            lifecycle_status,
            motor,
        );
        if lifecycle_status.just_started() {
            memory.builtin = bevy_tnua::builtins::TnuaBuiltinJumpMemory::MaintainingJump {
                wait_one_frame_before_updating_velocity_boundary: true,
                velocity_boundary: None,
            };
        }
        if holding || lifecycle_status.just_started() {
            motor.lin.boost += up
                * (velocity.max(self.jump_z_velocity) - velocity - motor.lin.boost.dot(up))
                    .max(0.0);
        }
        memory.hold_elapsed += self.delta_seconds;
        directive
    }

    fn influence_basis(
        &self,
        config: &Self::Config,
        memory: &Self::Memory,
        ctx: TnuaBasisContext,
        basis_input: &TnuaBuiltinWalk,
        basis_config: &TnuaBuiltinWalkConfig,
        basis_memory: &mut <TnuaBuiltinWalk as TnuaBasis>::Memory,
    ) {
        self.builtin.influence_basis(
            config,
            &memory.builtin,
            ctx,
            basis_input,
            basis_config,
            basis_memory,
        );
    }
}
