import json
from pathlib import Path


class Reader:
    def __init__(self, path, chunk_size=4194304):
        self.file = Path(path).open(encoding='utf-8')
        self.chunk_size = chunk_size
        self.buffer = ''
        self.position = 0
        self.eof = False
        self.decoder = json.JSONDecoder()

    def fill(self):
        self.buffer = self.buffer[self.position:]
        self.position = 0
        value = self.file.read(self.chunk_size)
        self.buffer += value
        self.eof = not value
        return bool(value)

    def peek(self):
        while True:
            while self.position < len(self.buffer) and self.buffer[self.position].isspace():
                self.position += 1
            if self.position < len(self.buffer):
                return self.buffer[self.position]
            if not self.fill():
                return ''

    def take(self, expected):
        if self.peek() != expected:
            raise ValueError(f'Expected JSON token {expected}')
        self.position += 1

    def value(self):
        self.peek()
        while True:
            try:
                value, end = self.decoder.raw_decode(self.buffer, self.position)
            except json.JSONDecodeError:
                if self.eof or not self.fill():
                    raise
                continue
            if end == len(self.buffer) and not self.eof:
                self.fill()
                continue
            if end < len(self.buffer) and self.buffer[end] not in ',]} \t\r\n:':
                if self.eof or not self.fill():
                    raise ValueError('Invalid JSON value delimiter')
                continue
            self.position = end
            return value


def members(path, chunk_size=4194304):
    reader = Reader(path, chunk_size)
    try:
        reader.take('[')
        reader.take('{')
        seen = set()
        frame_count = 0
        while reader.peek() != '}':
            key = reader.value()
            if not isinstance(key, str) or key in seen:
                raise ValueError('Invalid or duplicate case member')
            seen.add(key)
            reader.take(':')
            if key == 'frames':
                reader.take('[')
                while reader.peek() != ']':
                    value = reader.value()
                    yield ('frame', frame_count, value)
                    frame_count += 1
                    if reader.peek() == ']':
                        break
                    reader.take(',')
                    if reader.peek()==']':
                        raise ValueError('Trailing frame separator')
                reader.take(']')
            else:
                yield ('metadata', key, reader.value())
            if reader.peek() == '}':
                break
            reader.take(',')
            if reader.peek()=='}':
                raise ValueError('Trailing case separator')
        reader.take('}')
        reader.take(']')
        if reader.peek() or 'frames' not in seen:
            raise ValueError('Invalid case boundary or missing frames')
    finally:
        reader.file.close()
