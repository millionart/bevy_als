import gzip
import json
from pathlib import Path

previous = Path(__file__).resolve().parent.parent/'498/observer.py'
text = previous.read_text()
assert text.count('tools/tmp/bf144/498')==1
exec(compile(text.replace('tools/tmp/bf144/498','tools/tmp/bf144/503'),previous.name,'exec'))
root = Path(__file__).resolve().parent
temporary = Path('tools/tmp/bf144/503')
driver = (temporary/'playground_observed.rs').read_text()
key='    app.add_systems(First,c458_drive);'
assert driver.count(key)==1
driver=driver.replace(key,key+'''
    app.add_systems(PostUpdate,c503_bind_and_publish
        .after(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Inputs)
        .before(bevy_als::extensions::post_physics_animation::PostPhysicsAnimationSet::Evaluate));
''')
with gzip.open(root.parent/'502/replay-restored.json.gz','rt') as stream:
    controls=json.load(stream)
witness=next(row for row in controls if row['query']==101)
assert witness['actual_target'][3]!=witness['actual_target_widened'][3]
value=witness['actual_target_widened'][3]
(temporary/'playground_observed.rs').write_text(driver+f'\nconst C503_RIGHT: f32 = {value}_f32;\n'+(root/'controlled_input.rs').read_text())
print('Prepared two diagnostic readers with an exact one-frame D282 operand',flush=True)
