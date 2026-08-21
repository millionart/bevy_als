//! als-ext：bevy_animation_graph / bevy-tnua 基座插件的扩展层。
//!
//! 能力扩展优先经插件公开扩展点在本 crate 实现（curve 采样节点、
//! ControlRig 等价分析式 IK 等）；不修改 fork。M0.1 为引导占位。

pub(crate) mod placeholder {
    /// 占位：M2 起将落地 CurveAsset / CurveSampleNode 等。
    pub const CRATE_ROLE: &str = "als-ext: plugin extensions & analytic IK";
}

pub use placeholder::CRATE_ROLE;
