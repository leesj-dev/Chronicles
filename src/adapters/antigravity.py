import json
import sqlite3
import shutil
import tempfile
from contextlib import contextmanager
from datetime import datetime, timezone
from pathlib import Path
from .base import ScanResult, event_id


@contextmanager
def read_database(path):
    """Read WAL databases even when source sidecars cannot be created."""
    connection = None
    try:
        connection = sqlite3.connect(path.as_uri()+'?mode=ro', uri=True)
        connection.execute('SELECT name FROM sqlite_master LIMIT 1').fetchall()
    except sqlite3.Error as error:
        if connection is not None:
            connection.close()
            connection = None
        if not isinstance(error, sqlite3.OperationalError) or (getattr(error, 'sqlite_errorcode', 0) & 255) != sqlite3.SQLITE_CANTOPEN:
            raise
        with path.open('rb') as source:
            header = source.read(20)
        if header[:16] != b'SQLite format 3\x00' or header[18:20] != b'\x02\x02':
            raise
    else:
        try:
            yield connection
        finally:
            connection.close()
        return

    def signature():
        result = []
        for suffix in ('', '-wal'):
            source = Path(str(path)+suffix)
            try:
                stat = source.stat()
                result.append((stat.st_ino, stat.st_size, stat.st_mtime_ns))
            except FileNotFoundError:
                result.append(None)
        return tuple(result)

    with tempfile.TemporaryDirectory(prefix='chronicles-antigravity-') as directory:
        snapshot = Path(directory)/path.name
        before = signature()
        shutil.copyfile(path, snapshot)
        if before[1] is not None:
            shutil.copyfile(Path(str(path)+'-wal'), Path(str(snapshot)+'-wal'))
        if signature() != before:
            raise sqlite3.OperationalError('Antigravity database changed while copying; retry scan')
        # Preserve pending WAL records; SQLite creates its own SHM in this folder.
        connection = sqlite3.connect(snapshot.as_uri()+'?mode=ro', uri=True)
        try:
            yield connection
        finally:
            connection.close()


def protobuf(blob):
    fields = {}
    offset = 0
    def varint():
        nonlocal offset
        value = 0
        for shift in range(0, 70, 7):
            if offset >= len(blob):
                raise ValueError('truncated varint')
            byte = blob[offset]
            offset += 1
            value |= (byte & 127) << shift
            if byte < 128:
                return value
        raise ValueError('oversized varint')
    while offset < len(blob):
        tag = varint()
        field, wire = tag >> 3, tag & 7
        if field == 0:
            raise ValueError('invalid protobuf field')
        if wire == 0:
            value = varint()
        elif wire == 2:
            length = varint()
            if offset + length > len(blob):
                raise ValueError('truncated field')
            value = blob[offset:offset+length]
            offset += length
        elif wire in (1, 5):
            length = 8 if wire == 1 else 4
            if offset+length > len(blob):
                raise ValueError('truncated fixed field')
            offset += length
            continue
        else:
            raise ValueError('unsupported protobuf wire')
        fields.setdefault(field, value)
    return fields


def generation(blob, metadata=None):
    wrapped = protobuf(protobuf(blob).get(1, b''))
    usage = protobuf(wrapped.get(4, b''))
    raw = wrapped.get(19, b'').decode('utf-8').strip()
    label = wrapped.get(21, b'').decode('utf-8').strip()
    values = dict(input=usage.get(1, 0)+usage.get(2, 0), output=usage.get(3, 0), cacheRead=usage.get(5, 0), cacheWrite=0)
    if not raw and not label and not any(usage.get(k,0) for k in [2,3,5]):
        return None
    if not any(values.values()):
        return None
    timing = protobuf(wrapped.get(9, b''))
    timestamp = protobuf(timing.get(4, b'')).get(1)
    if not timestamp and metadata:
        timestamp = protobuf(protobuf(metadata).get(1,b'')).get(1)
    if not isinstance(timestamp, int) or timestamp <= 0:
        return None
    model = label if raw.endswith('-default') and label else raw or label or 'unknown'
    if model.endswith('-tiered'):
        model = model[:-7]
    return datetime.fromtimestamp(timestamp, timezone.utc).isoformat(), model, values


class AntigravityAdapter:
    provider = 'antigravity'
    def __init__(self, home=None):
        self.home = home or Path.home()/'.gemini'
        self.cache = {}

    def scan(self) -> ScanResult:
        events, errors, files, active = [], [], 0, set()
        self.legacy_ids = sorted({p.stem for p in self.home.glob('antigravity*/conversations/*.pb')})
        for directory in self.home.glob('antigravity*/conversations'):
            for path in directory.glob('*.db'):
                active.add(path)
                files += 1
                try:
                    wal = Path(str(path)+'-wal')
                    signature = (path.stat().st_mtime_ns, path.stat().st_size, wal.stat().st_mtime_ns if wal.exists() else 0)
                    if path not in self.cache or self.cache[path][0] != signature:
                        parsed = []
                        bad = 0
                        with read_database(path) as connection:
                            tables={r[0] for r in connection.execute("SELECT name FROM sqlite_master WHERE type='table'")}
                            if 'gen_metadata' not in tables:
                                self.cache[path]=(signature, [], 0)
                                continue
                            correlated = 'steps' in tables and 'metadata' in {r[1] for r in connection.execute('PRAGMA table_info(steps)')}
                            query = 'SELECT g.idx,g.data,s.metadata FROM gen_metadata g LEFT JOIN steps s ON s.idx=g.idx WHERE length(g.data)<=33554432' if correlated else 'SELECT idx,data,NULL FROM gen_metadata WHERE length(data)<=33554432'
                            for idx, blob, metadata in connection.execute(query):
                                try:
                                    event=generation(blob,metadata)
                                    if event:
                                        timestamp,model,values=event
                                        parsed.append((('antigravity',path.stem,idx,timestamp),timestamp,model,values))
                                except (ValueError, TypeError, UnicodeError, OverflowError):
                                    bad += 1
                            bad += connection.execute('SELECT count(*) FROM gen_metadata WHERE length(data)>33554432').fetchone()[0]
                        self.cache[path]=(signature, parsed, bad)
                    events.extend(self.cache[path][1])
                    if self.cache[path][2]:errors.append(f'Antigravity: {self.cache[path][2]} generation records skipped')
                except (sqlite3.Error, OSError) as error:
                    errors.append('Antigravity: '+type(error).__name__)
        self.cache={p:v for p,v in self.cache.items() if p in active}
        return ScanResult(events=[(key, self.provider, ts, model, values) for key, ts, model, values in events], files=files, errors=errors, legacy_ids=self.legacy_ids)


