use bevy::prelude::*;

use crate::als::state::{AlsNotifyLocomotionAction, Gait, LocomotionState, OverlayState, RotationMode, Stance};

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
    pub ragdoll_start: bool,
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

pub(crate) fn commit_state<T: Copy + PartialEq + Into<StateValue>>(
    entity: Entity,
    mut current: Mut<T>,
    next: T,
    transitions: &mut MessageWriter<StateTransitionEvent>,
) {
    if let Some(previous) = current.replace_if_neq(next) {
        let to = next.into();
        transitions.write(StateTransitionEvent {
            entity,
            dimension: to.dimension(),
            from: previous.into(),
            to,
        });
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq)]
pub struct StateTransitionRejectedEvent {
    pub entity: Entity,
    pub dimension: Dimension,
    pub requested: StateValue,
    pub reason: RejectReason,
}

fn process_dimension<T: Copy + PartialEq + Into<StateValue>>(
    entity: Entity,
    mut request_slot: Mut<Option<T>>,
    current: Mut<T>,
    verdict: Option<Verdict>,
    transitions: &mut MessageWriter<StateTransitionEvent>,
    rejections: &mut MessageWriter<StateTransitionRejectedEvent>,
) {
    let (Some(verdict), Some(requested)) = (verdict, *request_slot) else { return; };
    match verdict {
        Verdict::Accept => {
            *request_slot = None;
            commit_state(entity, current, requested, transitions);
        }
        Verdict::Reject(reason) => {
            *request_slot = None;
            let requested = requested.into();
            rejections.write(StateTransitionRejectedEvent {
                entity,
                dimension: requested.dimension(),
                requested,
                reason,
            });
        }
        Verdict::Queue => {}
    }
}

fn process_stance_dimension(
    entity: Entity,
    mut request_slot: Mut<Option<Stance>>,
    current: Mut<Stance>,
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
                desired.set_if_neq(AlsDesiredStance(requested));
            } else {
                commit_state(entity, current, requested, transitions);
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

pub(crate) fn locomotion_mode_locked(
    mode: LocomotionState,
    mantle: Option<&crate::als::mantle::AlsMantleState>,
    action: Option<&AlsNotifyLocomotionAction>,
) -> bool {
    (mode == LocomotionState::Custom
        && mantle.is_some_and(|state| state.active || state.root_motion_ready))
        || (mode == LocomotionState::None && action == Some(&AlsNotifyLocomotionAction::Ragdolling))
}

pub fn state_transition_system(
    mut commands: Commands,
    mut transitions: MessageWriter<StateTransitionEvent>,
    mut rejections: MessageWriter<StateTransitionRejectedEvent>,
    ragdoll_eligibility: crate::ragdoll_link::AlsRagdollStartEligibility,
    mut actors: Query<(
        Entity,
        Has<crate::als::character::AlsCharacter>,
        &mut AlsStateRequest,
        &mut LocomotionState,
        &mut Stance,
        Option<&mut AlsDesiredStance>,
        &mut Gait,
        &mut RotationMode,
        &mut OverlayState,
        Option<&mut AlsNotifyLocomotionAction>,
        Option<&crate::als::mantle::AlsMantleState>,
    ), Without<crate::als::character::AlsBirthAnimationPending>>,
) {
    for (
        entity,
        is_character,
        mut req,
        loco,
        stance,
        desired_stance,
        gait,
        rotation,
        overlay,
        mut action,
        mantle,
    ) in &mut actors
    {
        let mode_locked = locomotion_mode_locked(*loco, mantle, action.as_deref());
        if mode_locked && req.locomotion_state.is_some() {
            req.locomotion_state = None;
        }
        let ragdoll_start = req.ragdoll_start;
        if ragdoll_start { req.ragdoll_start = false; }
        if ragdoll_start && action.as_deref() != Some(&AlsNotifyLocomotionAction::Ragdolling)
            && (!is_character || ragdoll_eligibility.allows(entity))
        {
            req.ragdoll_exit = false;
            if !mode_locked {
                req.locomotion_state = Some(LocomotionState::None);
            }
            if let Some(action) = action.as_deref_mut() {
                *action = AlsNotifyLocomotionAction::Ragdolling;
            } else {
                commands.entity(entity).insert(AlsNotifyLocomotionAction::Ragdolling);
            }
        }
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
            req.reborrow().map_unchanged(|request| &mut request.locomotion_state),
            loco,
            verdicts.locomotion_state,
            &mut transitions,
            &mut rejections,
        );
        process_stance_dimension(
            entity,
            req.reborrow().map_unchanged(|request| &mut request.stance),
            stance,
            desired_stance,
            verdicts.stance,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            req.reborrow().map_unchanged(|request| &mut request.gait),
            gait,
            verdicts.gait,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            req.reborrow().map_unchanged(|request| &mut request.rotation_mode),
            rotation,
            verdicts.rotation_mode,
            &mut transitions,
            &mut rejections,
        );
        process_dimension(
            entity,
            req.map_unchanged(|request| &mut request.overlay_state),
            overlay,
            verdicts.overlay_state,
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
            v(LocomotionState::Grounded, LocomotionState::None),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::None, LocomotionState::Grounded),
            Verdict::Accept
        );
        assert_eq!(
            v(LocomotionState::Air, LocomotionState::None),
            Verdict::Accept,
            "air to ragdolling matches ALS behavior"
        );
        assert_eq!(
            v(LocomotionState::None, LocomotionState::Air),
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
        let c = cur(LocomotionState::None);
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
    fn state_change_ticks_follow_accepted_value_changes() {
        let mut app = App::new();
        app.add_plugins(AlsStateTransitionPlugin);
        let actor = app.world_mut().spawn((LocomotionState::Air, Stance::Standing, Gait::Walking,
            RotationMode::ViewDirection, OverlayState::Default, AlsDesiredStance(Stance::Standing),
            AlsStateRequest::default())).id();
        let ticks = |world: &World| {
            let actor = world.entity(actor);
            [actor.get_ref::<LocomotionState>().unwrap().last_changed(),
                actor.get_ref::<Stance>().unwrap().last_changed(),
                actor.get_ref::<Gait>().unwrap().last_changed(),
                actor.get_ref::<RotationMode>().unwrap().last_changed(),
                actor.get_ref::<OverlayState>().unwrap().last_changed(),
                actor.get_ref::<AlsDesiredStance>().unwrap().last_changed(),
                actor.get_ref::<AlsStateRequest>().unwrap().last_changed()]
        };
        let mut differences = Vec::new();
        let unchanged = [false; 7];
        let cases = [
            ("empty", AlsStateRequest::default(), unchanged, vec![], false, None),
            ("queued", AlsStateRequest::default().with_gait(Gait::Running), unchanged, vec![], false, Some(Gait::Running)),
            ("rejected", AlsStateRequest::default().with_gait(Gait::Running).with_policy(InvalidRequestPolicy::Reject),
                [false, false, false, false, false, false, true], vec![], true, None),
            ("same", AlsStateRequest::default().with_locomotion_state(LocomotionState::Air).with_stance(Stance::Standing)
                .with_gait(Gait::Walking).with_rotation_mode(RotationMode::ViewDirection).with_overlay_state(OverlayState::Default),
                [false, false, false, false, false, false, true], vec![], false, None),
            ("desired", AlsStateRequest::default().with_stance(Stance::Crouching),
                [false, false, false, false, false, true, true], vec![], false, None),
            ("commit", AlsStateRequest::default().with_locomotion_state(LocomotionState::Grounded)
                .with_rotation_mode(RotationMode::VelocityDirection).with_overlay_state(OverlayState::Rifle),
                [true, false, false, true, true, false, true],
                vec![Dimension::LocomotionState, Dimension::RotationMode, Dimension::OverlayState], false, None),
            ("gait", AlsStateRequest::default().with_gait(Gait::Running),
                [false, false, true, false, false, false, true], vec![Dimension::Gait], false, None),
        ];
        for (label, request, changes, dimensions, rejected, pending_gait) in cases {
            *app.world_mut().get_mut::<AlsStateRequest>(actor).unwrap() = request;
            let before = ticks(app.world());
            app.update();
            let after = ticks(app.world());
            let transitions = app.world_mut().resource_mut::<Messages<StateTransitionEvent>>().drain()
                .map(|event| event.dimension).collect::<Vec<_>>();
            let rejections = app.world_mut().resource_mut::<Messages<StateTransitionRejectedEvent>>().drain().count();
            let pending = app.world().get::<AlsStateRequest>(actor).unwrap().gait;
            let actual = std::array::from_fn::<_, 7, _>(|i| before[i] != after[i]);
            if actual != changes {
                differences.push((label, actual, changes));
            }
            assert_eq!(transitions, dimensions);
            assert_eq!(rejections, usize::from(rejected));
            assert_eq!(pending, pending_gait);
        }
        assert!(differences.is_empty(), "Unexpected component change ticks: {differences:?}");
    }

    #[test]
    fn named_locomotion_requests_keep_identity_and_emit_only_changes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin));
        let actor = app.world_mut().spawn((LocomotionState::Grounded, Stance::Standing, Gait::Walking,
            RotationMode::ViewDirection, OverlayState::Default, AlsStateRequest::default())).id();
        let climbing = LocomotionState::new("Als.LocomotionMode.Climbing").unwrap();
        let sliding = LocomotionState::new("Als.LocomotionMode.Sliding").unwrap();
        let mut previous = LocomotionState::Grounded;
        for requested in [climbing, climbing, sliding, LocomotionState::Air, LocomotionState::Grounded] {
            app.world_mut().get_mut::<AlsStateRequest>(actor).unwrap().locomotion_state = Some(requested);
            app.update();
            assert_eq!(*app.world().get::<LocomotionState>(actor).unwrap(), requested);
            assert!(app.world().get::<AlsStateRequest>(actor).unwrap().locomotion_state.is_none());
            let events: Vec<_> = app.world_mut().resource_mut::<Messages<StateTransitionEvent>>().drain().collect();
            assert_eq!(events.len(), usize::from(previous != requested));
            if let Some(event) = events.first() {
                assert_eq!(event.from, StateValue::LocomotionState(previous));
                assert_eq!(event.to, StateValue::LocomotionState(requested));
            }
            previous = requested;
        }
    }

    #[test]
    fn ragdoll_request_separates_action_from_locked_movement_mode() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin));
        for (mode, action, expected) in [
            (
                LocomotionState::Grounded,
                AlsNotifyLocomotionAction::None,
                LocomotionState::None,
            ),
            (
                LocomotionState::Air,
                AlsNotifyLocomotionAction::Rolling,
                LocomotionState::None,
            ),
            (
                LocomotionState::Custom,
                AlsNotifyLocomotionAction::Mantling,
                LocomotionState::Custom,
            ),
            (
                LocomotionState::Grounded,
                AlsNotifyLocomotionAction::Ragdolling,
                LocomotionState::Grounded,
            ),
        ] {
            let actor = app
                .world_mut()
                .spawn((
                    mode,
                    action,
                    Stance::Standing,
                    Gait::Walking,
                    RotationMode::ViewDirection,
                    OverlayState::Default,
                    AlsStateRequest {
                        ragdoll_start: true,
                        ..default()
                    },
            ))
                .id();
            if mode == LocomotionState::Custom {
                app.world_mut().entity_mut(actor).insert(crate::als::mantle::AlsMantleState { active: true, root_motion_ready: true, ..default() });
            }
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(actor), Some(&expected));
            assert_eq!(
                app.world().get::<AlsNotifyLocomotionAction>(actor),
                Some(&AlsNotifyLocomotionAction::Ragdolling)
            );
            assert!(
                !app.world()
                    .get::<AlsStateRequest>(actor)
                    .unwrap()
                    .ragdoll_start
            );
            let events: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<StateTransitionEvent>>()
                .drain()
                .collect();
            assert_eq!(events.len(), usize::from(mode != expected));
            if let Some(event) = events.first() {
                assert_eq!(event.from, StateValue::LocomotionState(mode));
                assert_eq!(event.to, StateValue::LocomotionState(expected));
            }
            app.world_mut()
                .get_mut::<AlsStateRequest>(actor)
                .unwrap()
                .ragdoll_start = true;
            app.update();
            assert_eq!(app.world().get::<LocomotionState>(actor), Some(&expected));
            assert_eq!(
                app.world_mut()
                    .resource_mut::<Messages<StateTransitionEvent>>()
                    .drain()
                    .count(),
                0
            );
            if expected != LocomotionState::Grounded {
                app.world_mut()
                    .get_mut::<AlsStateRequest>(actor)
                    .unwrap()
                    .locomotion_state = Some(LocomotionState::Air);
                app.update();
                assert_eq!(app.world().get::<LocomotionState>(actor), Some(&expected));
            }
            app.world_mut().despawn(actor);
        }
    }

    #[test]
    fn active_mantle_locks_ordinary_requests_and_keeps_desired_stance() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AlsStateTransitionPlugin));
        let actor = app
            .world_mut()
            .spawn((
                LocomotionState::Custom,
                Stance::Standing,
                AlsDesiredStance(Stance::Standing),
                Gait::Walking,
                RotationMode::ViewDirection,
                OverlayState::Default,
                AlsStateRequest::default(),
                crate::als::mantle::AlsMantleState { active: true, root_motion_ready: true, ..default() },
            ))
            .id();
        for requested in [LocomotionState::Grounded, LocomotionState::Air] {
            *app.world_mut().get_mut::<AlsStateRequest>(actor).unwrap() = AlsStateRequest::default()
                .with_locomotion_state(requested)
                .with_stance(Stance::Crouching);
            app.update();
            assert_eq!(
                app.world().get::<LocomotionState>(actor),
                Some(&LocomotionState::Custom)
            );
            assert_eq!(
                app.world()
                    .get::<AlsStateRequest>(actor)
                    .unwrap()
                    .locomotion_state,
                None
            );
            assert_eq!(app.world().get::<Stance>(actor), Some(&Stance::Standing));
            assert_eq!(
                app.world().get::<AlsDesiredStance>(actor).unwrap().0,
                Stance::Crouching
            );
        }
        app.world_mut()
            .get_mut::<AlsStateRequest>(actor)
            .unwrap()
            .ragdoll_start = true;
        app.update();
        assert_eq!(
            app.world().get::<LocomotionState>(actor),
            Some(&LocomotionState::Custom)
        );
        assert_eq!(
            app.world().get::<AlsNotifyLocomotionAction>(actor),
            Some(&AlsNotifyLocomotionAction::Ragdolling)
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
    fn unlocked_none_to_air_transitions_end_to_end() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AlsStateTransitionPlugin);

        let e = app
            .world_mut()
            .spawn((
                LocomotionState::None,
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
                from: StateValue::LocomotionState(LocomotionState::None),
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
