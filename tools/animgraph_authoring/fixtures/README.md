# Native animation fixtures

`ue_standing_walk_run_tick.json` is the unmodified `blendspace_probe` section exported on 2026-09-08 from `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.BS_Als_WalkRun_Forward`. It calls UE's `UBlendSpace::TickAssetPlayer` with one asset in a marker sync group. It covers start position 0.2, a walk-to-run input change and a stride change; it does not represent the complete AnimBP or Actor.

Recreate the request and export a new source record without overwriting this fixture:

```powershell
python -c "import json,pathlib; r=json.load(open('tools/animgraph_authoring/fixtures/ue_standing_walk_run_tick.json')); pathlib.Path('target/standing_tick.request.json').write_text(json.dumps({'start_position':r['start_position'],'looped':r['looped'],'frames':[f['input'] for f in r['frames']]}),encoding='utf-8')"
pwsh -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.BS_Als_WalkRun_Forward -Output target/standing_tick.ue.json -BlendSpaceProbe target/standing_tick.request.json
```
