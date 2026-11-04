import json
from pathlib import Path

root=Path(__file__).resolve().parent
cases=json.loads((root/'wall.native.json').read_bytes())['character_animation_probe']['cases']
case=next(row for row in cases if row['name']=='grazing')
frame=case['frames'][40]
forward,down=frame['blocking_hits']
assert forward['component']=='AuditStepFin' and down['walkable_hit']
floor=down['reused_down_hit_floor']
assert floor['walkable'] and 1<floor['distance_cm']<1.1
data=dict(gap_m=floor['distance_cm']/100,move_stop_cm=down['move_stop_cm'],
          current_floor_gap_m=frame['actual_current_floor']['distance_cm']/100)
(root/'source-gap.json').write_text(json.dumps(data,indent=2)+'\n')
print('Prepared the actual native pre-adjustment gap as the target control',flush=True)
