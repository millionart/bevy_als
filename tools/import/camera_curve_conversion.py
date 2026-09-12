from coordinate_basis import SOURCE_LENGTH_TO_RUNTIME_SCALE


CAMERA_CURVE_TARGETS = {
    'CameraOffsetX': ('CameraOffsetZ', -SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'CameraOffsetY': ('CameraOffsetX', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'CameraOffsetZ': ('CameraOffsetY', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'PivotOffsetX': ('PivotOffsetX', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'PivotOffsetY': ('PivotOffsetZ', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'PivotOffsetZ': ('PivotOffsetY', SOURCE_LENGTH_TO_RUNTIME_SCALE),
    'LocationLagX': ('LocationLagZ', 1.0),
    'LocationLagY': ('LocationLagX', 1.0),
    'LocationLagZ': ('LocationLagY', 1.0),
}


def camera_curve_target(name, apply_mode='Blend'):
    target, scale = CAMERA_CURVE_TARGETS.get(name, (name, 1.0))
    return target, 1.0 if apply_mode == 'Scale' else scale


def self_test():
    for prefix, source, expected in [
        ('CameraOffset', [100.0, 200.0, 300.0], [2.0, 3.0, -1.0]),
        ('PivotOffset', [100.0, 200.0, 300.0], [1.0, 3.0, 2.0]),
        ('LocationLag', [0.2, 0.4, 0.8], [0.4, 0.8, 0.2]),
    ]:
        values = {}
        for axis, value in zip('XYZ', source):
            target, scale = camera_curve_target(prefix + axis)
            values[target] = value * scale
        assert [values[prefix + axis] for axis in 'XYZ'] == expected
    for name in CAMERA_CURVE_TARGETS:
        assert camera_curve_target(name, 'Scale')[1] == 1.0
    assert camera_curve_target('FovOffset') == ('FovOffset', 1.0)
    print('Camera curve conversion self-test PASS', flush=True)


if __name__ == '__main__':
    self_test()
