fn c517_align(
    probe: Res<C458Probe>,
    state: Res<C516Control>,
    mut actors: Query<(&mut Position,&mut Transform)>,
) {
    if probe.frame!=state.frame {
        return;
    }
    let Ok(label)=std::env::var("ALS_C517_ALIGN") else {return;};
    let key=match label.as_str() {
        "start-aligned" => "start_shift",
        "step-aligned" => "first_step_shift",
        _ => panic!("Unknown ledge alignment control"),
    };
    let input:serde_json::Value=serde_json::from_slice(
        &std::fs::read("docs/audit/bf144/517/alignment-input.json").unwrap()).unwrap();
    let shift=input[key].as_f64().unwrap() as f32;
    assert!(shift>0.0 && shift<0.03);
    let (mut position,mut transform)=actors.get_mut(probe.actor.unwrap()).unwrap();
    let before=position.0;
    assert!((transform.translation-before).length()<0.001);
    position.0.x+=shift;
    transform.translation.x=position.0.x;
    c516_write("alignment.json",serde_json::json!({"frame":state.frame,"label":label,
        "requested":shift,"before":before,"after":position.0}));
}
