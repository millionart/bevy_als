from pathlib import Path

root = Path(__file__).resolve().parent
old = root.parent/'484'
temporary = Path('tools/tmp/bf144/501')
temporary.mkdir(parents=True,exist_ok=True)
driver = (old/'playground_observed.rs').read_text().replace('probe.frame >= 240','probe.frame >= 80').replace('probe.frame==240','probe.frame==80')
a = driver.index('        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240"')
b = driver.index('\n',a)
driver = driver[:a]+'''        std::fs::write(output,serde_json::to_vec(&serde_json::json!({
            "before":&probe.animation_before[96..=103],"after":&probe.animation_after[96..=103],
            "physics_before":&probe.rows[17..25],"physics_after":&probe.camera_rows[17..25]
        })).unwrap()).unwrap();'''+driver[b:]
k='        "ordinal":probe.actor.map(|_|probe.frame.saturating_sub(1)),'
assert driver.count(k)==1
driver=driver.replace(k,k+'''
        "velocity_blend_state":c501_blend(player),''')
(temporary/'playground_observed.rs').write_text(driver+'\n'+(root/'velocity_observer.rs').read_text())
(temporary/'foot_observer.rs').write_bytes((old/'foot_observer.rs').read_bytes())
print('Prepared existing Playground with eight actual pre/post VelocityBlend observations',flush=True)
