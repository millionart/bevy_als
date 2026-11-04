from pathlib import Path

previous = Path(__file__).resolve().parent.parent / '508' / 'prepare_reader.py'
script = previous.read_text()
for old, new in (
    ('bf144/508', 'bf144/510'),
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
witness = (Path(__file__).resolve().parent / 'native_witness.inc').read_text()
marker = "if mode == 'omit-write':"
assert script.count(marker) == 1
insertion = '        CaseRow->SetArrayField(TEXT("frames"), Rows);'
script = script.replace(marker,
    'assert function.count(' + repr(insertion) + ') == 1\n'
    + 'function = function.replace(' + repr(insertion) + ', ' + repr(witness + insertion) + ')\n'
    + marker)
exec(compile(script, previous.name, 'exec'))
