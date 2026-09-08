# Native animation fixtures

`ue_standing_walk_run_tick.json` is the unmodified `blendspace_probe` section exported on 2026-09-08 from `/ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.BS_Als_WalkRun_Forward`. It calls UE's `UBlendSpace::TickAssetPlayer` with one asset in a marker sync group. It covers start position 0.2, a walk-to-run input change and a stride change; it does not represent the complete AnimBP or Actor.

`ue_standing_walk_run_weights.json` is a second unmodified native probe from the same asset and date. Its 160 frames cover equal weights and interior positions. The same native asset record exposes `BlendSpaceData` triangles `[1, 0, 3]` and `[0, 2, 3]`; the runtime fixture retains that topology and UE's small-array sample ordering. Resetting the triangle cache near a shared edge and removing the native triangulation are controlled failing ablations. Together the two timelines check 310 frames against native filtered coordinates, sample weights and sample times.

Recreate the request and export a new source record without overwriting this fixture:

```powershell
python -c "import json,pathlib; r=json.load(open('tools/animgraph_authoring/fixtures/ue_standing_walk_run_tick.json')); pathlib.Path('target/standing_tick.request.json').write_text(json.dumps({'start_position':r['start_position'],'looped':r['looped'],'frames':[f['input'] for f in r['frames']]}),encoding='utf-8')"
pwsh -File tools/run_ue_source_reader.ps1 -Asset /ALS/ALS/Animations/Grounded/WalkRun/BS_Als_WalkRun_Forward.BS_Als_WalkRun_Forward -Output target/standing_tick.ue.json -BlendSpaceProbe target/standing_tick.request.json
```

For the second timeline, use `ue_standing_walk_run_weights.json` as the request source and a different output prefix.
