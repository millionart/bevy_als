from pathlib import Path

root = Path(__file__).resolve().parent
old = root.parent/'484'
temporary = Path('tools/tmp/bf144/495')
temporary.mkdir(parents=True,exist_ok=True)
driver = (old/'playground_observed.rs').read_text().replace('probe.frame >= 240','probe.frame >= 80').replace('probe.frame==240','probe.frame==80')
a=driver.index('        std::fs::write(output,serde_json::to_vec_pretty(&serde_json::json!({"gate":"CAPTURED_REAL_INPUT_CAMERA_AND_DESIRED_DIRECTION240"')
b=driver.index('\n',a)
driver=driver[:a]+'''        let rows=[0,1,2,3,105,158].map(|index| &probe.animation_after[index]);
        std::fs::write(output,serde_json::to_vec(&rows).unwrap()).unwrap();'''+driver[b:]
key='        "head_apply":if before{Vec::new()}else{c458_head_apply(player,&graphs,&skeletons,&assets)},'
assert driver.count(key)==1
driver=driver.replace(key,key+'''
        "scale_nodes":if before||![0,1,2,3,105,158].contains(&probe.animation_after.len()){Vec::new()}else{c495_scales(player,&graphs,&skeletons,&assets)},''')
(temporary/'playground_observed.rs').write_text(driver+'\n'+(root/'scale_observer.rs').read_text())
(temporary/'foot_observer.rs').write_bytes((old/'foot_observer.rs').read_bytes())
print('Prepared existing Playground with six upstream cached-pose observations',flush=True)
