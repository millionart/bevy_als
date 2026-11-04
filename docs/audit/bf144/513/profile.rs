fn c513_profile(
    ready: Res<ManualPlaygroundStartupReady>,
    admissions: Query<&AlsPlayerAdmissionComplete>,
    mut handles: Query<&mut bevy_als::als::character::AlsMovementParamsHandle>,
    mut assets: ResMut<Assets<AlsMovementParams>>,
    mut recorded: Local<bool>,
) {
    if *recorded || !ready.passed {
        return;
    }
    let Some(admission) = admissions.iter().next() else { return; };
    let Ok(mut handle) = handles.get_mut(admission.character) else { return; };
    let original = assets.get(&handle.0).unwrap().clone();
    let mut effective = original.clone();
    let original_curves = &original.acceleration.grounded;
    let selected = std::env::var("ALS_C513_PROFILE").ok();
    match selected.as_deref() {
        Some("normal") => effective.acceleration.grounded.responsive = original_curves.normal.clone(),
        Some("acceleration-profile") => {
            effective.acceleration.grounded.responsive.acceleration = original_curves.normal.acceleration.clone();
        }
        None => {}
        _ => panic!("Unknown dynamics profile control"),
    }
    let mut compared = effective.clone();
    compared.acceleration.grounded.responsive = original_curves.responsive.clone();
    assert_eq!(compared, original);
    if selected.is_some() {
        let previous = handle.0.clone();
        handle.0 = assets.add(effective.clone());
        assert_eq!(assets.get(&previous), Some(&original));
    }
    let output = std::path::PathBuf::from(std::env::var("ALS_C458_OUTPUT").unwrap()).with_file_name("profile.json");
    std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({
        "mode":selected,"only_responsive_slot_changed":true,
        "original":original_curves,
        "effective":effective.acceleration.grounded
    })).unwrap()).unwrap();
    *recorded = true;
}
