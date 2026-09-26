use bevy_als::extensions::AlsExtPlugin;
use bevy::prelude::*;
use bevy_animation_graph_editor::AnimationGraphEditorPlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(AnimationGraphEditorPlugin);
    app.add_plugins(AlsExtPlugin);
    app.run();
}
