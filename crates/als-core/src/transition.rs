use bevy::prelude::*;

use crate::state::{Gait, LocomotionState, OverlayState, RotationMode, Stance};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    LocomotionState,
    Stance,
    Gait,
    RotationMode,
    OverlayState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateValue {
    LocomotionState(LocomotionState),
    Stance(Stance),
    Gait(Gait),
    RotationMode(RotationMode),
    OverlayState(OverlayState),
}

impl StateValue {
    pub const fn dimension(self) -> Dimension {
        match self {
            StateValue::LocomotionState(_) => Dimension::LocomotionState,
            StateValue::Stance(_) => Dimension::Stance,
            StateValue::Gait(_) => Dimension::Gait,
            StateValue::RotationMode(_) => Dimension::RotationMode,
            StateValue::OverlayState(_) => Dimension::OverlayState,
        }
    }
}

impl From<LocomotionState> for StateValue {
    fn from(v: LocomotionState) -> Self {
        Self::LocomotionState(v)
    }
}
impl From<Stance> for StateValue {
    fn from(v: Stance) -> Self {
        Self::Stance(v)
    }
}
impl From<Gait> for StateValue {
    fn from(v: Gait) -> Self {
        Self::Gait(v)
    }
}
impl From<RotationMode> for StateValue {
    fn from(v: RotationMode) -> Self {
        Self::RotationMode(v)
    }
}
impl From<OverlayState> for StateValue {
    fn from(v: OverlayState) -> Self {
        Self::OverlayState(v)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum InvalidRequestPolicy {
    #[default]
    Queue,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RejectReason {
    StanceRequiresGrounded,
    GaitRequiresGrounded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    Legal,
    Illegal(RejectReason),
    IllegalQueuable(RejectReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accept,
    Reject(RejectReason),
    Queue,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsStateRequest {
    pub locomotion_state: Option<LocomotionState>,
    pub ragdoll_exit: bool,
    pub stance: Option<Stance>,
    pub gait: Option<Gait>,
    pub rotation_mode: Option<RotationMode>,
    pub overlay_state: Option<OverlayState>,
    pub on_invalid: InvalidRequestPolicy,
}

#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AlsDesiredStance(pub Stance);

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlsPhysicalStanceOverride(pub Stance);

impl AlsStateRequest {
    pub const fn with_locomotion_state(mut self, v: LocomotionState) -> Self {
        self.locomotion_state = Some(v);
        self
    }
    pub const fn with_stance(mut self, v: Stance) -> Self {
        self.stance = Some(v);
        self
    }
    pub const fn with_gait(mut self, v: Gait) -> Self {
        self.gait = Some(v);
        self
    }
    pub const fn with_rotation_mode(mut self, v: RotationMode) -> Self {
        self.rotation_mode = Some(v);
        self
    }
    pub const fn with_overlay_state(mut self, v: OverlayState) -> Self {
        self.overlay_state = Some(v);
        self
    }
    pub const fn with_policy(mut self, p: InvalidRequestPolicy) -> Self {
        self.on_invalid = p;
        self
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsCurrentStates {
    pub locomotion_state: LocomotionState,
    pub stance: Stance,
    pub gait: Gait,
    pub rotation_mode: RotationMode,
    pub overlay_state: OverlayState,
}

pub fn legality(value: StateValue, cur: &AlsCurrentStates) -> Legality {
    match value {
        StateValue::LocomotionState(to) => {
            if to == cur.locomotion_state {
                return Legality::Legal;
            }
            Legality::Legal
        }
        StateValue::Stance(_) => Legality::Legal,
        StateValue::Gait(to) => {
            if to == cur.gait {
                return Legality::Legal;
            }
            match cur.locomotion_state {
                LocomotionState::Grounded => Legality::Legal,
                _ => Legality::IllegalQueuable(RejectReason::GaitRequiresGrounded),
            }
        }
        StateValue::RotationMode(_) | StateValue::OverlayState(_) => Legality::Legal,
    }
}

pub fn validate_one(
    value: impl Into<StateValue>,
    cur: &AlsCurrentStates,
    policy: InvalidRequestPolicy,
) -> Verdict {
    match legality(value.into(), cur) {
        Legality::Legal => Verdict::Accept,
        Legality::Illegal(reason) => Verdict::Reject(reason),
        Legality::IllegalQueuable(reason) => match policy {
            InvalidRequestPolicy::Queue => Verdict::Queue,
            InvalidRequestPolicy::Reject => Verdict::Reject(reason),
        },
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct AlsVerdicts {
    pub locomotion_state: Option<Verdict>,
    pub stance: Option<Verdict>,
    pub gait: Option<Verdict>,
    pub rotation_mode: Option<Verdict>,
    pub overlay_state: Option<Verdict>,
}

impl AlsVerdicts {
    pub fn get(&self, dimension: Dimension) -> Option<Verdict> {
        match dimension {
            Dimension::LocomotionState => self.locomotion_state,
            Dimension::Stance => self.stance,
            Dimension::Gait => self.gait,
            Dimension::RotationMode => self.rotation_mode,
            Dimension::OverlayState => self.overlay_state,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.get(Dimension::LocomotionState).is_none()
            && self.get(Dimension::Stance).is_none()
            && self.get(Dimension::Gait).is_none()
            && self.get(Dimension::RotationMode).is_none()
            && self.get(Dimension::OverlayState).is_none()
    }
}

pub fn validate(req: &AlsStateRequest, cur: &AlsCurrentStates) -> AlsVerdicts {
    AlsVerdicts {
        locomotion_state: req
            .locomotion_state
            .map(|v| validate_one(v, cur, req.on_invalid)),
        stance: req.stance.map(|v| validate_one(v, cur, req.on_invalid)),
        gait: req.gait.map(|v| validate_one(v, cur, req.on_invalid)),
        rotation_mode: req
            .rotation_mode
            .map(|v| validate_one(v, cur, req.on_invalid)),
        overlay_state: req
            .overlay_state
            .map(|v| validate_one(v, cur, req.on_invalid)),
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct StateTransitionEvent {
    pub entity: Entity,
    pub dimension: Dimension,
    pub from: StateValue,
    pub to: StateValue,
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct StateTransitionRejectedEvent {
    pub entity: Entity,
    pub dimension: Dimension,
    pub requested: StateValue,
    pub reason: RejectReason,
}

#[allow(clippy::too_many_arguments)]
fn process_dimension<T: Copy + PartialEq>(
    entity: Entity,
    dimension: Dimension,
    request_slot: &mut Option<T>,
    current: &mut T,
    verdict: Option<Verdict>,
    to_value: impl Fn(T) -> StateValue,
    transitions: &mut MessageWriter<StateTransitionEvent>,
    rejections: &mut MessageWriter<StateTransitionRejectedEvent>,
) {
    let (Some(verdict), Some(requested)) = (verdict, *request_slot) else {
        return;
    };
    match verdict {
        Verdict::Accept => {
            *request_slot = None;
            if requested != *current {
                let from = *current;
                *current = requested;
                transitions.write(StateTransitionEvent {
                    entity,
                    dimension,
                    from: to_value(from),
                    to: to_value(requested),
                });
            }
        }
        Verdict::Reject(reason) => {
            *request_slot = None;
            rejections.write(StateTransitionRejectedEvent {
                entity,
                dimension,
                requested: to_value(requested),
                reason,
            });
        }
        Verdict::Queue => {}
    }
}

fn process_stance_dimension(
    entity: Entity,
    request_slot: &mut Option<Stance>,
    current: &mut Stance,
    desired: Option<Mut<AlsDesiredStance>>,
    verdict: Option<Verdict>,
    transitions: &mut MessageWriter<StateTransitionEvent>,
    rejections: &mut MessageWriter<StateTransitionRejectedEvent>,
) {
    let (Some(verdict), Some(requested)) = (verdict, *request_slot) else {
        return;
    };
    match verdict {
        Verdict::Accept => {
            *request_slot = None;
            if let Some(mut desired) = desired {
                desired.0 = requested;
            } else if requested != *current {
                let from = *current;
                *current = requested;
                transitions.write(StateTransitionEvent {
                    entity,
                    dimension: Dimension::Stance,
                    from: StateValue::Stance(from),
                    to: StateValue::Stance(requested),
                });
            }
        }
        Verdict::Reject(reason) => {
            *request_slot = None;
            rejections.write(StateTransitionRejectedEvent {
                entity,
                dimension: Dimension::Stance,
                requested: StateValue::Stance(requested),
                reason,
            });
        }
        Verdict::Queue => {}
    }
}

pub fn state_transition_system(
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut rejections: MessageWriter<StateTransitionRejectedEvent>,
    mut actors: Query<(
        Entity,
        &mut AlsStateRequest,
        &mut LocomotionState,
        &mut Stance,
        Option<&mut AlsDesiredStance>,
        &mut Gait,
        &mut RotationMode,
        &mut OverlayState,
    )>,
) {
    for (
        entity,
        mut req,
        mut loco,
        mut stance,
        desired_stance,
        mut gait,
        mut rotation,
        mut overlay,
    ) in &mut actors
    {
        let cur = AlsCurrentStates {
            locomotion_state: *loco,
            stance: *stance,
            gait: *gait,
            rotation_mode: *rotation,
            overlay_state: *overlay,
        };
        let verdicts = validate(&req, &cur);

        process_dimension(
            entity,
            Dimension::LocomotionState,
            &mut req.locomotion_state,
            &mut *loco,
            verdicts.locomotion_state,
            StateValue::LocomotionState,
            &mut transitions,
            &mut rejections,
        );
        process_stance_dimension(
            entity,
            &mut req.stance,
            &mut *stance,
            desired_stance,
            verdicts.stance,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            Dimension::Gait,
            &mut req.gait,
            &mut *gait,
            verdicts.gait,
            StateValue::Gait,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            Dimension::RotationMode,
            &mut req.rotation_mode,
            &mut *rotation,
            verdicts.rotation_mode,
            StateValue::RotationMode,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            Dimension::OverlayState,
            &mut req.overlay_state,
            &mut *overlay,
            verdicts.overlay_state,
            StateValue::OverlayState,
            &mut transitions,
            &mut rejections,
        );
    }
}

pub struct AlsStateTransitionPlugin;

impl Plugin for AlsStateTransitionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<StateTransitionEvent>()
            .add_message::<StateTransitionRejectedEvent>()
            .add_systems(Update, state_transition_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::ecs::system::RunSystemOnce;

    fn cur(loco: LocomotionState) -> AlsCurrentStates {
        AlsCurrentStates {
            locomotion_state: loco,
            ..Default::default()
        }
    }

    #[test]
    fn air_crouch_updates_desired_stance_regardless_of_request_policy() {
        let c = cur(LocomotionState::Air);
        let req = AlsStateRequest::default().with_stance(Stance::Crouching);
        assert_eq!(
            validate(&req, &c).stance,
            Some(Verdict::Accept),
            "desired stance updates immediately in air"
        );
        let req = req.with_policy(InvalidRequestPolicy::Reject);
        assert_eq!(
            validate(&req, &c).stance,
            Some(Verdict::Accept),
            "desired stance is not a physical crouch request"
        );
    }

    #[test]
    fn stance_requests_remain_legal_after_landing() {
        let req = AlsStateRequest::default().with_stance(Stance::Crouching);
        let in_air = validate(&req, &cur(LocomotionState::Air));
        assert_eq!(in_air.stance, Some(Verdict::Accept));
        let landed = validate(&req, &cur(LocomotionState::Grounded));
        assert_eq!(landed.stance, Some(Verdict::Accept));
    }

    #[test]
    fn locomotion_transitions_match_als_matrix() {
        let v = |from, to| validate_one(to, &cur(from), InvalidRequestPolicy::Queue);
        assert_eq!(
            v(LocomotionState::Grounded, LocomotionState::Air),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::Air, LocomotionState::Grounded),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::Grounded, LocomotionState::Ragdolling),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::Ragdolling, LocomotionState::Grounded),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::Air, LocomotionState::Ragdolling),
            Verdict::Accept,
            "air to ragdolling matches ALS behavior"
        );
        assert_eq!(
            v(LocomotionState::Ragdolling, LocomotionState::Air),
            Verdict::Accept,
            "ragdolling exit may enter air when the ground trace is clear"
        );
    }

    #[test]
    fn gair_requests_queue_while_crouched_gait_passes_through() {
        let v = |loco, gait: Gait| validate_one(gait, &cur(loco), InvalidRequestPolicy::Queue);
        assert_eq!(
            v(LocomotionState::Air, Gait::Running),
            Verdict::Queue,
            "gait request in air should queue (ALS: sprint resumes after landing)"
        );
        let crouched = AlsCurrentStates {
            locomotion_state: LocomotionState::Grounded,
            stance: Stance::Crouching,
            ..Default::default()
        };
        assert_eq!(
            validate_one(Gait::Walking, &crouched, InvalidRequestPolicy::Queue),
            Verdict::Accept,
            "gait passes through while crouching: actual semantics derived by crouch walk speed table"
        );
    }

    #[test]
    fn overlay_and_rotation_mode_are_always_legal() {
        let c = cur(LocomotionState::Ragdolling);
        let req = AlsStateRequest::default()
            .with_overlay_state(OverlayState::Rifle)
            .with_rotation_mode(RotationMode::Aiming);
        let verdicts = validate(&req, &c);
        assert_eq!(verdicts.overlay_state, Some(Verdict::Accept));
        assert_eq!(verdicts.rotation_mode, Some(Verdict::Accept));
    }

    #[test]
    fn same_value_request_is_idempotent_accept() {
        let c = cur(LocomotionState::Grounded);
        let req = AlsStateRequest::default()
            .with_stance(Stance::Standing)
            .with_gait(Gait::Walking)
            .with_overlay_state(OverlayState::Default);
        let verdicts = validate(&req, &c);
        assert_eq!(verdicts.stance, Some(Verdict::Accept));
        assert_eq!(verdicts.gait, Some(Verdict::Accept));
        assert_eq!(verdicts.overlay_state, Some(Verdict::Accept));
        assert!(verdicts.get(Dimension::LocomotionState).is_none());
        assert!(!verdicts.is_empty());
    }

    #[test]
    fn state_value_reports_own_dimension() {
        assert_eq!(
            StateValue::from(Stance::Crouching).dimension(),
            Dimension::Stance
        );
        assert_eq!(
            StateValue::from(LocomotionState::Air).dimension(),
            Dimension::LocomotionState
        );
    }

    #[test]
    fn airborne_crouch_updates_desired_stance_without_writing_actual_stance() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AlsStateTransitionPlugin);

        let e = app
            .world_mut()
            .spawn((
                LocomotionState::Air,
                Stance::Standing,
                AlsDesiredStance(Stance::Standing),
                Gait::Walking,
                RotationMode::VelocityDirection,
                OverlayState::Default,
                AlsStateRequest::default().with_stance(Stance::Crouching),
            ))
            .id();

        app.update();
        assert_eq!(
            *app.world().get::<Stance>(e).unwrap(),
            Stance::Standing,
            "desired stance must not write the actual stance"
        );
        assert_eq!(
            app.world().get::<AlsDesiredStance>(e).unwrap().0,
            Stance::Crouching,
            "airborne input persists desired crouching"
        );
        assert_eq!(app.world().get::<AlsStateRequest>(e).unwrap().stance, None);
        let events = app
            .world_mut()
            .run_system_once(|mut reader: MessageReader<StateTransitionEvent>| {
                reader.read().copied().collect::<Vec<_>>()
            })
            .unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn airborne_crouch_uses_desired_stance_with_reject_policy_too() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AlsStateTransitionPlugin);

        let e = app
            .world_mut()
            .spawn((
                LocomotionState::Air,
                Stance::Standing,
                AlsDesiredStance(Stance::Standing),
                Gait::Walking,
                RotationMode::VelocityDirection,
                OverlayState::Default,
                AlsStateRequest::default()
                    .with_stance(Stance::Crouching)
                    .with_policy(InvalidRequestPolicy::Reject),
            ))
            .id();

        app.update();
        assert_eq!(*app.world().get::<Stance>(e).unwrap(), Stance::Standing);
        assert_eq!(
            app.world().get::<AlsDesiredStance>(e).unwrap().0,
            Stance::Crouching
        );

        let rejections = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionRejectedEvent>| {
                r.read().copied().collect::<Vec<_>>()
            })
            .unwrap();
        assert!(rejections.is_empty());

        let transitions = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionEvent>| r.read().copied().count())
            .unwrap();
        assert_eq!(
            transitions, 0,
            "desired stance updates do not emit actual-stance transitions"
        );
    }

    #[test]
    fn ragdoll_to_air_transitions_end_to_end() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AlsStateTransitionPlugin);

        let e = app
            .world_mut()
            .spawn((
                LocomotionState::Ragdolling,
                Stance::Standing,
                Gait::Walking,
                RotationMode::VelocityDirection,
                OverlayState::Default,
                AlsStateRequest::default().with_locomotion_state(LocomotionState::Air),
            ))
            .id();

        app.update();
        assert_eq!(
            *app.world().get::<LocomotionState>(e).unwrap(),
            LocomotionState::Air,
            "ragdolling exit may enter air"
        );
        assert_eq!(
            app.world()
                .get::<AlsStateRequest>(e)
                .unwrap()
                .locomotion_state,
            None,
            "accepted request clears after transition"
        );
        let transitions = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionEvent>| {
                r.read().copied().collect::<Vec<_>>()
            })
            .unwrap();
        let rejections = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionRejectedEvent>| {
                r.read().copied().collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(
            transitions,
            vec![StateTransitionEvent {
                entity: e,
                dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Ragdolling),
                to: StateValue::LocomotionState(LocomotionState::Air),
            }],
            "ragdoll-to-air transition must be observable"
        );
        assert!(rejections.is_empty());
    }

    #[test]
    fn ground_to_air_transition_fires_event_and_noop_request_fires_none() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AlsStateTransitionPlugin);

        let e = app
            .world_mut()
            .spawn((
                LocomotionState::Grounded,
                Stance::Standing,
                Gait::Walking,
                RotationMode::VelocityDirection,
                OverlayState::Default,
                AlsStateRequest::default()
                    .with_locomotion_state(LocomotionState::Air)
                    .with_overlay_state(OverlayState::Default),
            ))
            .id();

        app.update();
        assert_eq!(
            *app.world().get::<LocomotionState>(e).unwrap(),
            LocomotionState::Air
        );

        let events = app
            .world_mut()
            .run_system_once(|mut r: MessageReader<StateTransitionEvent>| {
                r.read().copied().collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(
            events,
            vec![StateTransitionEvent {
                entity: e,
                dimension: Dimension::LocomotionState,
                from: StateValue::LocomotionState(LocomotionState::Grounded),
                to: StateValue::LocomotionState(LocomotionState::Air),
            }],
            "only the real LocomotionState change event; same-value Overlay is idempotent no-op"
        );
    }
}
