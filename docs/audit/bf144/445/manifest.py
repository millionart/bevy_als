import gzip
import hashlib
import json
import re
from pathlib import Path

r=Path(__file__).resolve().parent
native=json.loads(gzip.decompress(Path('docs/audit/bf144/445a/restored.native.json.gz').read_bytes()))['character_animation_probe']['cases'][0]
decl={}
frames=[]
for frame in native['frames']:
    players=[]
    for instance in frame['linked_instances']:
        for player in instance['asset_players']:
            key=player['node_guid']
            row=dict(owner=instance['class'],**player)
            signature={k:row[k]for k in ['node_guid','owner','node_type','asset']}
            if key in decl:assert decl[key]==signature
            else:decl[key]=signature
            players.append(row)
    frames.append(dict(frame=frame['frame'],players=players))
assert len(decl)==85
matches={k:[]for k in decl}
for p in sorted(Path('assets/ALS/Character').rglob('*.animgraph.ron')):
    for line in p.read_text().splitlines():
        m=re.search(r'\(id: "([0-9a-f-]{36})", name: "((?:\\.|[^"\\])*)", inner: \{ "([^"]+)":',line)
        if m and m[1]in matches:
            matches[m[1]].append(dict(path=str(p.relative_to('assets')),name=m[2],target_type=m[3],file_sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
for key,items in matches.items():assert len(items)==1,(key,items)
result=dict(source='docs/audit/bf144/445a/restored.native.json.gz',source_declarations=decl,target_declarations={k:v[0]for k,v in matches.items()})
(r/'declarations.json').write_text(json.dumps(result,indent=2)+'\n')
raw=(json.dumps(dict(frames=frames),separators=(',',':'))+'\n').encode()
packed=gzip.compress(raw,mtime=0)
assert gzip.decompress(packed)==raw
(r/'native-players.json.gz').write_bytes(packed)
(r/'native-player-compression.json').write_text(json.dumps(dict(raw_bytes=len(raw),raw_sha256=hashlib.sha256(raw).hexdigest(),gzip_bytes=len(packed),gzip_sha256=hashlib.sha256(packed).hexdigest(),round_trip_byte_exact=True),indent=2)+'\n')
print('Mapped all 85 native player GUIDs to current target declarations',flush=True)
