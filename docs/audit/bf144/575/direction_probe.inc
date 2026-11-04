fn c575_install(app: &mut App) {
    app.edit_schedule(Update, |schedule| {
        let drive = {
            let matches: Vec<_> = schedule.graph().systems.iter().filter(|(_, system, _)|
                system.name().to_string().ends_with("::locomotion_drive_system")).collect();
            assert_eq!(matches.len(), 1);
            matches[0].1.default_system_sets()[0]
        };
        schedule.add_systems(c575_input.after(drive).before(TnuaPipelineSystems::Sensors));
    });
}

fn c575_input(
    probe: Res<C458Probe>,
    mut actors: Query<(
        &mut TnuaController<AlsControlScheme>, &bevy_als::als::motion::AlsViewYaw,
        Option<&bevy_als::als::view::AlsPlayerViewYaw>, &LocomotionState,
    )>,
) {
    let frame = probe.frame;
    if !(52..=58).contains(&frame) { return; }
    let (mut controller, view, published, mode) = actors.get_mut(probe.actor.unwrap()).unwrap();
    let natural = controller.basis.desired_motion;
    let label = std::env::var("ALS_C575_MODE").unwrap();
    if label != "natural" {
        let side = match label.as_str() {
            "axial" => 0.0,
            "positive" => 1.0,
            "negative" => -1.0,
            _ => panic!("Unknown common input control"),
        };
        controller.basis.desired_motion = if frame == 54 && side != 0.0 {
            Vec3::new(1.0 - 1.0 / 2_097_152.0, 0.0, side / 1024.0)
        } else {
            Vec3::X
        };
    }
    c516_write(&format!("direction-{frame}.json"), serde_json::json!({
        "frame": frame, "mode": mode, "view_yaw": view.0,
        "published_yaw": published.map(|value| value.0),
        "natural": natural, "submitted": controller.basis.desired_motion
    }));
}
