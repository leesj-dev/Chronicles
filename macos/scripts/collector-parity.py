"""Compare Swift with the existing Python reference using fictional histories only."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
os.environ['CHRONICLES_TIMEZONE'] = 'Asia/Seoul'
sys.path.insert(0, str(ROOT / 'src'))
from server import Collector

with tempfile.TemporaryDirectory(prefix='chronicles-collector-parity-') as temporary:
    home = Path(temporary)
    sessions, archived, projects = home/'codex/sessions', home/'codex/archived_sessions', home/'claude/projects/p'
    for path in [sessions, archived, projects]:
        path.mkdir(parents=True)
    def codex(date, total, model=None, last=None):
        info = dict(total_token_usage=total)
        if model:
            info['model'] = model
        if last:
            info['last_token_usage'] = last
        return dict(timestamp=date, payload=dict(type='token_count', info=info))
    first = dict(input_tokens=100, cached_input_tokens=20, cache_write_input_tokens=5, output_tokens=10, total_tokens=110)
    second = dict(input_tokens=130, cached_input_tokens=25, cache_write_input_tokens=5, output_tokens=20, total_tokens=150)
    logs = [dict(type='turn_context', payload=dict(model='gpt-6-sol')),
            codex('2026-10-09T15:00:00Z', first), codex('2026-10-09T15:00:01Z', first),
            codex('2026-10-09T15:00:02Z', second, last=dict(input_tokens=999, output_tokens=999)),
            codex('2026-10-09T15:00:03Z', dict(input_tokens=8, output_tokens=2, total_tokens=10), 'gpt-6-astra', dict(input_tokens=8, output_tokens=2))]
    content = '\n'.join(json.dumps(record) for record in logs) + '\nmalformed\n'
    (sessions/'usage.jsonl').write_text(content)
    (archived/'copy.jsonl').write_text(content)
    def claude(output, model='claude-sonnet-4.5'):
        return dict(timestamp='2026-01-01T15:00:00Z', requestId='r', message=dict(id='m', model=model,
                    usage=dict(input_tokens=10, output_tokens=output, cache_read_input_tokens=2, cache_creation_input_tokens=3)))
    (projects/'usage.jsonl').write_text('\n'.join(json.dumps(record) for record in [claude(5), claude(20), claude(999, '<synthetic>')]))
    reference = Collector(roots=[('codex', sessions), ('codex', archived), ('claude', projects)]).collect()
    actual = json.loads(subprocess.check_output([str(ROOT/'macos/.build/debug/chronicles-inspect'),
                         '--codex-home', str(home/'codex'), '--claude-home', str(home/'claude'), '--timezone', 'Asia/Seoul']))
    fields = ['date', 'provider', 'model', 'device', 'requests', 'input', 'output', 'cacheRead', 'cacheWrite', 'total']
    def rows(report):
        return sorted([{key: row[key] for key in fields} for row in report['rows']], key=lambda row: (row['date'], row['provider'], row['model']))
    assert rows(actual) == rows(reference), (rows(actual), rows(reference))
    assert actual['files'] == reference['files'] == 3
    assert actual['skipped'] == reference['skipped'] == 2
    assert actual['timezone'] == reference['timezone'] == 'Asia/Seoul'
    print('Native/Python collector parity passed: counters, deduplication, cumulative deltas, reset, cache buckets, skipped lines and timezone.')
