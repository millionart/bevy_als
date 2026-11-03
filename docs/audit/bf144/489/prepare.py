from pathlib import Path

parent = Path(__file__).resolve().parent.parent
previous = parent/'487/prepare.py'
prepare = previous.read_text()
for old,new in [('tools/tmp/bf144/487','tools/tmp/bf144/489'),('486/replay-restored.json.gz','488/replay-restored.json.gz')]:
    assert prepare.count(old)==1
    prepare = prepare.replace(old,new)
exec(compile(prepare,previous.name,'exec'))

fixture = (parent/'487/replay.rs').read_text()
a = fixture.index('fn project(')
b = fixture.index('fn foot_case(',a)
c = fixture.index('fn main()',b)
fixture = (fixture[:a]+fixture[b:c]).replace('/487/','/489/')
(temporary/'foot.rs').write_text(fixture)
print('Reused qualified fixed-history foot fixture with the D278 world-rotation control',flush=True)
