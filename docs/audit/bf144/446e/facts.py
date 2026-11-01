import hashlib
import json
from pathlib import Path

r=Path(__file__).resolve().parent
units=[(Path('src/als/character/animation_instances/animation_context_projection.rs'),'src/als/character/animation_instances/animation_context_projection.rs',[(21,34),(40,51),(77,110)]),(Path('src/als/character/animation_instances/ab_als_provider.rs'),'src/als/character/animation_instances/ab_als_provider.rs',[(139,155),(417,424)]),(Path('src/animation_graph_ext/source_instance.rs'),'src/animation_graph_ext/source_instance.rs',[(141,170),(180,198)])]
dep=next(Path('target_game/play/deps').glob('bevy_diagnostic-*.d')).read_text().split()
p=Path(next(v for v in dep if v.endswith('/frame_count.rs')))
units.append((p,'dependency:bevy_diagnostic/frame_count.rs',[(10,39)]))
result=[]
for p,key,ranges in units:
    lines=p.read_text().splitlines()
    result.append(dict(key=key,sha256=hashlib.sha256(p.read_bytes()).hexdigest(),excerpts=[dict(start=a,end=b,text='\n'.join(lines[a-1:b]))for a,b in ranges]))
(r/'source-facts.json').write_text(json.dumps(result,indent=2)+'\n')
print('Captured actual target prior curve, property resolution and application frame contracts',flush=True)
