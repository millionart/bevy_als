import argparse
import math


SOURCE_LENGTH_TO_RUNTIME_SCALE = 0.01


def source_point_to_runtime(values):
    return [
        values[0] * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        values[2] * SOURCE_LENGTH_TO_RUNTIME_SCALE,
        values[1] * SOURCE_LENGTH_TO_RUNTIME_SCALE,
    ]


def runtime_point_to_source(values):
    return [
        values[0] / SOURCE_LENGTH_TO_RUNTIME_SCALE,
        values[2] / SOURCE_LENGTH_TO_RUNTIME_SCALE,
        values[1] / SOURCE_LENGTH_TO_RUNTIME_SCALE,
    ]


def source_direction_to_runtime(values):
    return [values[0], values[2], values[1]]


def runtime_direction_to_source(values):
    return [values[0], values[2], values[1]]


def source_rotation_to_runtime(values):
    return [-values[0], -values[2], -values[1], values[3]]


def runtime_rotation_to_source(values):
    return [-values[0], -values[2], -values[1], values[3]]


def source_rotator_to_runtime_rotation(values):
    pitch, yaw, roll = (math.radians(value) for value in values)
    pitch_sine = math.sin(-pitch * 0.5)
    yaw_sine = math.sin(yaw * 0.5)
    roll_sine = math.sin(-roll * 0.5)
    pitch_rotation = [0.0, pitch_sine, 0.0, math.cos(pitch * 0.5)]
    yaw_rotation = [0.0, 0.0, yaw_sine, math.cos(yaw * 0.5)]
    roll_rotation = [roll_sine, 0.0, 0.0, math.cos(roll * 0.5)]
    source = quaternion_multiply(quaternion_multiply(yaw_rotation, pitch_rotation), roll_rotation)
    return source_rotation_to_runtime(source)


def quaternion_multiply(left, right):
    lx, ly, lz, lw = left
    rx, ry, rz, rw = right
    return [
        lw * rx + lx * rw + ly * rz - lz * ry,
        lw * ry - lx * rz + ly * rw + lz * rx,
        lw * rz + lx * ry - ly * rx + lz * rw,
        lw * rw - lx * rx - ly * ry - lz * rz,
    ]


def self_test():
    if source_point_to_runtime([100.0, 200.0, 300.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("point conversion")
    if runtime_point_to_source([1.0, 3.0, 2.0]) != [100.0, 200.0, 300.0]:
        raise AssertionError("inverse point conversion")
    if source_direction_to_runtime([1.0, 2.0, 3.0]) != [1.0, 3.0, 2.0]:
        raise AssertionError("direction conversion")
    if source_rotation_to_runtime([0.2, -0.3, 0.4, 0.5]) != [-0.2, -0.4, 0.3, 0.5]:
        raise AssertionError("rotation conversion")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    self_test()
    if args.self_test:
        print("Coordinate basis self-test PASS")


if __name__ == "__main__":
    main()
