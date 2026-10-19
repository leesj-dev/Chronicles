import json
import tempfile
import unittest
from collections import defaultdict
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from zoneinfo import ZoneInfo

from adapters.base import ScanResult, event_id
from server import Collector, demo_report


class HourlyTests(unittest.TestCase):
    def test_dedup_and_legacy_import_preserve_daily_totals_and_export_timestamps(self):
        values = dict(input=10, output=5, cacheRead=2, cacheWrite=0)
        key = ('claude', 'shared')
        scan = ScanResult(events=[(key, 'claude', '2026-10-18T15:30:00Z', 'claude-haiku-5-5', values)])
        collector = Collector(roots=[])
        collector.adapters = [SimpleNamespace(scan=lambda: scan)]
        with tempfile.TemporaryDirectory() as directory:
            collector.import_path = Path(directory)/'import.json'
            collector.import_path.write_text(json.dumps(dict(events=[
                dict(id=event_id(key), provider='claude', date='2026-10-18', model='claude-haiku-5-5', **values),
                dict(id='old', provider='claude', date='2026-10-19', model='claude-haiku-5-5', **values),
            ])))
            with patch('server.KST', ZoneInfo('Asia/Seoul')), patch('adapters.base.KST', ZoneInfo('Asia/Seoul')):
                report = collector.collect()
        self.assertEqual(report['events'], 2)
        self.assertEqual(sum(row['requests'] for row in report['hourlyRows']), 2)
        self.assertEqual(sum(row['total'] for row in report['hourlyRows']), sum(row['total'] for row in report['rows']))
        self.assertEqual({row['hour'] for row in report['hourlyRows']}, {None, '2026-10-19T00:00:00+09:00'})
        self.assertEqual(collector.local_events[0]['timestamp'], '2026-10-18T15:30:00+00:00')
        self.assertTrue(all(row['date'] == '2026-10-19' for row in report['rows']))
        self.assertEqual(values['output'], 5)

    def test_fall_back_hours_do_not_merge(self):
        collector = Collector(roots=[])
        values = dict(input=10, output=0, cacheRead=0, cacheWrite=0)
        collector.adapters = [SimpleNamespace(scan=lambda: ScanResult(events=[
            (('a',), 'claude', '2026-11-01T05:30:00Z', 'model', values),
            (('b',), 'claude', '2026-11-01T06:30:00Z', 'model', values),
        ]))]
        with patch('server.KST', ZoneInfo('America/New_York')), patch('adapters.base.KST', ZoneInfo('America/New_York')):
            report = collector.collect()
        self.assertEqual(len(report['rows']), 1)
        self.assertEqual(len(report['hourlyRows']), 2)
        self.assertEqual({row['hour'] for row in report['hourlyRows']}, {'2026-11-01T01:00:00-04:00', '2026-11-01T01:00:00-05:00'})

    def test_fictional_demo_hours_sum_to_original_daily_records(self):
        report = demo_report()
        def grouped(rows):
            result = defaultdict(lambda: defaultdict(int))
            for row in rows:
                for field in ['requests', 'input', 'output', 'cacheRead', 'cacheWrite', 'total']:
                    result[(row['date'], row['provider'], row['model'], row['device'])][field] += row[field]
            return result
        self.assertEqual(grouped(report['rows']), grouped(report['hourlyRows']))
