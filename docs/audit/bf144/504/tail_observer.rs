fn c504_write(output: String, probe: &C458Probe) {
    assert_eq!(probe.animation_after.len(), 199);
    let rows=probe.animation_after.iter().enumerate().skip(95).map(|(query,row)| {
        let rig=&row["rig_snapshot"];
        let history=&row["history_input"];
        let calls=rig["calls"].as_array().unwrap().iter()
            .filter(|call| matches!(call["instruction"].as_u64(),Some(89|119)))
            .collect::<Vec<_>>();
        let registers=&rig["registers"];
        let readouts=row["scale_nodes"].as_array().unwrap().iter().filter_map(|node| {
            if node["pin"]!="Pose" {return None;}
            let audit=&node["c503"];
            (!audit.is_null()).then(||serde_json::json!({"guid":node["guid"],"right":audit["right"]}))
        }).collect::<Vec<_>>();
        serde_json::json!({"query":query,"ordinal":row["ordinal"],"delta":row["frame_delta"],
            "locomotion":history["locomotion"],"gait":history["gait"],"effective_move":history["effective_move"],
            "position":row["position"],"velocity":row["velocity"],"provider_speed":row["provider_speed"],
            "rig_input":rig["externals"]["RigInput"],"to_world":rig["to_world"],"init":rig["initialization_count"],
            "rig_source":rig["poses"]["rig_source"]["pose"],"root":rig["poses"]["root_result"]["pose"],
            "root_gait":rig["upstream_context_curves"]["PoseGait"],"traces":rig["traces"],
            "pole_states":[registers[281],registers[331]],"pole_init":[registers[282],registers[332]],
            "pole_calls":calls,"audited_inputs":readouts})
    }).collect::<Vec<_>>();
    std::fs::write(output, serde_json::to_vec(&rows).unwrap()).unwrap();
}
