from pathlib import Path

previous = Path(__file__).resolve().parent.parent / '508' / 'prepare_reader.py'
script = previous.read_text()
for old, new in (
    ('bf144/508', 'bf144/509'),
    ('velocity_blend_half_life', 'lean_half_life'),
    ('bOverrideVelocityBlend', 'bOverrideLean'),
    ('VelocityBlendHalfLife', 'LeanHalfLife'),
    ('VelocityBlendInterpolationHalfLife', 'LeanInterpolationHalfLife'),
    ('Grounded', 'General'),
):
    assert old in script
    script = script.replace(old, new)
marker = "if mode == 'omit-write':"
assert script.count(marker) == 1
script = script.replace(marker, '''capture = 'TEXT("GroundedState"), TEXT("LocomotionState")'
assert function.count(capture) == 1
function = function.replace(capture, 'TEXT("LeanState"), ' + capture)
''' + marker)
exec(compile(script, previous.name, 'exec'))
