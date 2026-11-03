from pathlib import Path

previous = Path(__file__).resolve().parent.parent/'495/observer.py'
text = previous.read_text()
for old,new in [
    ('tools/tmp/bf144/495','tools/tmp/bf144/497'),
    ('[0,1,2,3,105,158].map(|index| &probe.animation_after[index])','(95..=110).map(|index| &probe.animation_after[index]).collect::<Vec<_>>()'),
    ('![0,1,2,3,105,158].contains(&probe.animation_after.len())','!(95..=110).contains(&probe.animation_after.len())'),
    ('with six upstream cached-pose observations','with 16 consecutive Lean observations'),
]:
    assert text.count(old)==1
    text=text.replace(old,new)
exec(compile(text,previous.name,'exec'))
