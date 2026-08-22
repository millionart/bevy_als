use bevy_tnua::builtins::{
    TnuaBuiltinCrouch, TnuaBuiltinCrouchConfig, TnuaBuiltinJump, TnuaBuiltinJumpConfig,
    TnuaBuiltinWalk, TnuaBuiltinWalkConfig,
};
use bevy_tnua::TnuaScheme;

#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum AlsControlScheme {
    Jump(TnuaBuiltinJump),
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
