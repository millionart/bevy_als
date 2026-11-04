from pathlib import Path

root = Path(__file__).resolve().parent
previous = root.parent/'516'/'prepare_reader.py'
script = previous.read_text().replace('tools/tmp/bf144/516','tools/tmp/bf144/519')
anchor = 'body = text.replace(anchor,replacement).encode()'
assert script.count(anchor)==1
wall = (root/'native_wall.inc').read_text()
script = script.replace(anchor,
    "text = text.replace(anchor,replacement)\n"
    "wall_anchor = '        Box->RegisterComponent();\\n        FActorSpawnParameters Spawn;'\n"
    "assert text.count(wall_anchor)==1\n"
    "text = text.replace(wall_anchor, '        Box->RegisterComponent();\\n' + "
    + repr(wall) + " + '        FActorSpawnParameters Spawn;')\n"
    "body = text.encode()")
exec(compile(script,previous.name,'exec'))
