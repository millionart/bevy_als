use bevy::{
    asset::{Asset, AssetEvent, AssetId, Assets},
    ecs::{message::MessageCursor, system::SystemParam},
    prelude::*,
};

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum AssetRevision {
    #[default]
    Stable,
    Pending,
    Invalidated,
}

#[derive(SystemParam)]
pub(crate) struct AssetRevisions<'w, 's, A: Asset> {
    messages: Option<Res<'w, Messages<AssetEvent<A>>>>,
    cursor: Local<'s, MessageCursor<AssetEvent<A>>>,
    pending: Local<'s, bool>,
}

impl<A: Asset> AssetRevisions<'_, '_, A> {
    pub(crate) fn read(
        &mut self,
        assets: &Res<Assets<A>>,
        added_may_affect_pose: impl Fn(AssetId<A>) -> bool,
    ) -> AssetRevision {
        use AssetRevision::*;

        let Some(messages) = &self.messages else {
            let unknown = assets.is_changed() || *self.pending;
            *self.pending = false;
            return if unknown { Invalidated } else { Stable };
        };
        let mut invalidated = self.cursor.missed_messages(messages) > 0;
        let mut seen = false;
        for event in self.cursor.read(messages) {
            seen = true;
            invalidated |= match event {
                AssetEvent::Modified { .. } | AssetEvent::Removed { .. } => true,
                AssetEvent::Added { id } => added_may_affect_pose(*id),
                _ => false,
            };
        }
        if invalidated {
            *self.pending = false;
            return Invalidated;
        }

        // Bevy publishes the batch and drains the asset queue in the same system run.
        let published = seen && assets.last_changed() == messages.last_changed();
        let was_pending = *self.pending;
        *self.pending = assets.is_changed() && !published;
        if *self.pending {
            Pending
        } else if was_pending && !published {
            Invalidated
        } else {
            Stable
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::{AssetApp, AssetPlugin};

    use super::*;

    #[derive(Asset, TypePath)]
    struct Sample(u32);

    fn observe(app: &mut App) -> AssetRevision {
        app.world_mut()
            .run_system_cached(
                |assets: Res<Assets<Sample>>, mut changes: AssetRevisions<Sample>| {
                    changes.read(&assets, |_| false)
                },
            )
            .unwrap()
    }

    #[test]
    fn additions_can_settle_without_invalidating_the_cohort() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Sample>();

        let handle = app
            .world_mut()
            .resource_mut::<Assets<Sample>>()
            .add(Sample(0));
        assert_eq!(observe(&mut app), AssetRevision::Pending);
        app.update();
        assert_eq!(observe(&mut app), AssetRevision::Stable);

        app.world_mut()
            .resource_mut::<Assets<Sample>>()
            .get_mut(&handle)
            .unwrap()
            .0 += 1;
        assert_eq!(observe(&mut app), AssetRevision::Pending);
        app.update();
        assert_eq!(observe(&mut app), AssetRevision::Invalidated);
    }

    #[test]
    fn a_published_addition_does_not_cover_a_later_write() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Sample>();

        let handle = app
            .world_mut()
            .resource_mut::<Assets<Sample>>()
            .add(Sample(0));
        assert_eq!(observe(&mut app), AssetRevision::Pending);
        app.update();
        app.world_mut()
            .resource_mut::<Assets<Sample>>()
            .get_mut_untracked(handle.id())
            .unwrap()
            .0 += 1;
        assert_eq!(observe(&mut app), AssetRevision::Pending);
        app.update();
        assert_eq!(observe(&mut app), AssetRevision::Invalidated);
    }
}
