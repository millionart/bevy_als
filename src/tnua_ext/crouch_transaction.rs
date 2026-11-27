use bevy::prelude::Component;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleStance {
    Expanded,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleStancePhase {
    Expanded,
    EnteringCompact,
    Compact,
    TryingToExpand,
    BlockedFromExpanding,
    ExitingCompact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleStanceCommand {
    None,
    ApplyCompact,
    TryExpand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsuleStanceResult {
    None,
    CompactApplied,
    ExpandBlocked,
    ExpandedApplied,
}

/// Standalone capsule stance helper for host controllers. ALS characters use
/// [`crate::als::motion::AlsCrouchTransaction`] for the movement and animation lifecycle.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapsuleStanceTransaction {
    pub desired: CapsuleStance,
    pub actual: CapsuleStance,
    pub phase: CapsuleStancePhase,
    pub last_command: CapsuleStanceCommand,
    pub last_result: CapsuleStanceResult,
    pub retry_count: u32,
    pub revision: u64,
}

impl Default for CapsuleStanceTransaction {
    fn default() -> Self {
        Self {
            desired: CapsuleStance::Expanded,
            actual: CapsuleStance::Expanded,
            phase: CapsuleStancePhase::Expanded,
            last_command: CapsuleStanceCommand::None,
            last_result: CapsuleStanceResult::None,
            retry_count: 0,
            revision: 0,
        }
    }
}

impl CapsuleStanceTransaction {
    pub fn synchronize(&mut self, desired: CapsuleStance, actual: CapsuleStance) {
        if self.desired != desired {
            self.revision = self.revision.wrapping_add(1);
            self.retry_count = 0;
        }
        self.desired = desired;
        self.actual = actual;
        if desired == actual {
            self.phase = match actual {
                CapsuleStance::Expanded => CapsuleStancePhase::Expanded,
                CapsuleStance::Compact => CapsuleStancePhase::Compact,
            };
        } else if desired == CapsuleStance::Compact {
            self.phase = CapsuleStancePhase::EnteringCompact;
        } else if self.phase != CapsuleStancePhase::BlockedFromExpanding {
            self.phase = CapsuleStancePhase::TryingToExpand;
        }
    }

    pub fn command(&mut self) -> CapsuleStanceCommand {
        let command = match (self.desired, self.actual) {
            (CapsuleStance::Compact, CapsuleStance::Expanded) => {
                self.phase = CapsuleStancePhase::EnteringCompact;
                CapsuleStanceCommand::ApplyCompact
            }
            (CapsuleStance::Expanded, CapsuleStance::Compact) => {
                self.phase = CapsuleStancePhase::TryingToExpand;
                CapsuleStanceCommand::TryExpand
            }
            _ => CapsuleStanceCommand::None,
        };
        self.last_command = command;
        command
    }

    pub fn compact_applied(&mut self) {
        self.actual = CapsuleStance::Compact;
        self.phase = CapsuleStancePhase::Compact;
        self.last_result = CapsuleStanceResult::CompactApplied;
    }

    pub fn expand_blocked(&mut self) {
        self.actual = CapsuleStance::Compact;
        self.phase = CapsuleStancePhase::BlockedFromExpanding;
        self.last_result = CapsuleStanceResult::ExpandBlocked;
        self.retry_count = self.retry_count.saturating_add(1);
    }

    pub fn expanded_applied(&mut self) {
        self.actual = CapsuleStance::Expanded;
        self.phase = CapsuleStancePhase::Expanded;
        self.last_result = CapsuleStanceResult::ExpandedApplied;
        self.retry_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_expand_keeps_intent_and_retries() {
        let mut transaction = CapsuleStanceTransaction::default();
        transaction.synchronize(CapsuleStance::Compact, CapsuleStance::Expanded);
        assert_eq!(transaction.command(), CapsuleStanceCommand::ApplyCompact);
        transaction.compact_applied();
        transaction.synchronize(CapsuleStance::Expanded, CapsuleStance::Compact);
        assert_eq!(transaction.command(), CapsuleStanceCommand::TryExpand);
        transaction.expand_blocked();
        assert_eq!(transaction.desired, CapsuleStance::Expanded);
        assert_eq!(transaction.actual, CapsuleStance::Compact);
        assert_eq!(transaction.phase, CapsuleStancePhase::BlockedFromExpanding);
        transaction.synchronize(CapsuleStance::Expanded, CapsuleStance::Compact);
        assert_eq!(transaction.command(), CapsuleStanceCommand::TryExpand);
        transaction.expanded_applied();
        assert_eq!(transaction.actual, CapsuleStance::Expanded);
        assert_eq!(transaction.retry_count, 0);
    }

    #[test]
    fn repeated_pairs_and_instances_remain_isolated() {
        let mut first = CapsuleStanceTransaction::default();
        let mut second = CapsuleStanceTransaction::default();
        for _ in 0..100 {
            first.synchronize(CapsuleStance::Compact, CapsuleStance::Expanded);
            assert_eq!(first.command(), CapsuleStanceCommand::ApplyCompact);
            first.compact_applied();
            first.synchronize(CapsuleStance::Expanded, CapsuleStance::Compact);
            assert_eq!(first.command(), CapsuleStanceCommand::TryExpand);
            first.expanded_applied();
        }
        second.synchronize(CapsuleStance::Compact, CapsuleStance::Expanded);
        assert_eq!(second.command(), CapsuleStanceCommand::ApplyCompact);
        second.compact_applied();
        second.synchronize(CapsuleStance::Expanded, CapsuleStance::Compact);
        second.expand_blocked();
        assert_eq!(first.actual, CapsuleStance::Expanded);
        assert_eq!(first.retry_count, 0);
        assert_eq!(second.actual, CapsuleStance::Compact);
        assert_eq!(second.retry_count, 1);
    }
}
