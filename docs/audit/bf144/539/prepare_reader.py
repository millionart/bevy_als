import hashlib
import json
from pathlib import Path

root=Path(__file__).resolve().parent
previous=root.parent/'537'/'prepare_reader.py'
script=previous.read_text().replace('tools/tmp/bf144/537','tools/tmp/bf144/539')
for name in ('native_fin.inc','down_miss.inc'):
    script=script.replace(f"(root/'{name}')",f"(root.parent/'537'/'{name}')")
exec(compile(script,previous.name,'exec'))
path=Path('tools/tmp/bf144/539/reader-source')/relative
text=path.read_text()
anchor='FVector(SpawnXY.X + 114.5, SpawnXY.Y + 27, 8)'
assert text.count(anchor)==1
data=text.replace(anchor,'FVector(SpawnXY.X + 114.5, SpawnXY.Y + 27, 7.3)').encode()
path.write_bytes(data)
assert (source/relative).read_bytes()==original
(root/'reader-source.json').write_text(json.dumps(dict(
    canonical_sha256=hashlib.sha256(original).hexdigest(),diagnostic_sha256=hashlib.sha256(data).hexdigest(),
    diagnostic_header_sha256=hashlib.sha256(header.read_bytes()).hexdigest()),indent=2)+'\n')
print('Prepared the native 17.3 cm grazing obstacle',flush=True)
