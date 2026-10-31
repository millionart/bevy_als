import hashlib
import json
from pathlib import Path
r=Path(__file__).resolve().parent
source=Path('src/als_extras/levels/manual_playground.rs').read_text();needle='    if let AppExit::Error(code) = app.run() {';assert source.count(needle)==1
injection='''    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(1.0 / 30.0)));
    app.init_resource::<C441Probe>();
    app.add_systems(First, c441_drive);
    app.add_systems(Update, c441_sample.after(bevy_als::als::dynamics::grounded_dynamics_motor_system).before(bevy_tnua::TnuaPipelineSystems::Motors));
'''
result=source.replace(needle,injection+needle)+(r/'observer.rs').read_text();(r/'playground_observed.rs').write_text(result)
report={'method':'Original manual_playground source with a passive parameter observer and explicit controlled input/time inserted before app.run','original_source_sha256':hashlib.sha256(source.encode()).hexdigest(),'original_scene_setup_removed_or_replaced':False,'no_claim_as_unmodified_public_runner_gate':True,'controlled_case':'Standing ViewDirection Running;240 frames;forward input20..59 and120..159;view yaw-90deg','source_parameter_values_injected':False}
(r/'observer-provenance.json').write_text(json.dumps(report,indent=2)+'\n');print('Prepared original Playground entry plus actual parameter observer; source code untouched',flush=True)
