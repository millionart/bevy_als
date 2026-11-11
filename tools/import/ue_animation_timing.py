import struct


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def native_notify_times(document):
    events = {}
    for case in document["notify_clock_probe"]["cases"]:
        for event in case["events"]:
            times = event["trigger_time"], event["end_trigger_time"]
            if events.setdefault(event["event_index"], times) != times:
                raise ValueError("Native notify clock samples disagree")
    return events
