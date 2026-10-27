import base64
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
from zoneinfo import ZoneInfo

from adapters import jsonl
from server import Collector


# A genuine Zstandard frame: Astra model context and three token reports (one repeat).
ARCHIVE = base64.b64decode(
    'KLUv/QRYlQYAskslIGBn2wYI2tob101G9ednva1+Q5Ar0Y6P+IMJlFkUYgADCaBgQDI1U5Gn3TZH9ORx9GNHlAWlMEGyyRFlJtS7fm0xvKx7qtlmzQYWsx5L1OIC1gIIRBGIGlJASiOTcTjyOOqbs+3YYfqIvjhH9WIDJjzPN9FujYPRbTnqG1aY8wXqlWWNYtsRvQ+4F0vUSrw+6rkp+osXABBAlYMTCqAQQAdoBmgB9iWYDE17XjNHQiQEeMEABBfnwf/sC4zOeXXQKQM7YrCGSbzAY6DiwRb2epMEKA=='
)


class CompressedCodexTests(unittest.TestCase):
    @unittest.skipUnless(jsonl.zstd is not None or jsonl.shutil.which('zstd'), 'Zstandard decoder unavailable')
    def test_fresh_scan_restores_original_dates_and_deduplicates_plain_archive_and_import(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root/'old.jsonl.zst'
            archive.write_bytes(ARCHIVE)
            original = archive.read_bytes()
            # Both modules bind the report timezone at import time. Keep the
            # parser and hourly collector in the same zone on UTC CI runners.
            with patch('adapters.base.KST', ZoneInfo('Asia/Seoul')), \
                    patch('server.KST', ZoneInfo('Asia/Seoul')):
                collector = Collector([('codex', root)])
                report = collector.collect()
                self.assertEqual(report['errors'], [])
                self.assertEqual(report['events'], 2)
                rows = report['rows']
                self.assertEqual([(r['date'], r['model'], r['total']) for r in rows],
                                 [('2026-10-01', 'gpt-6-astra', 120), ('2026-10-02', 'gpt-6-astra', 60)])
                self.assertEqual(sum(r['input'] for r in rows), 110)
                self.assertEqual(sum(r['cacheRead'] for r in rows), 40)
                with jsonl.open_log(archive) as stream:
                    plain = stream.read()
                (root/'copy.jsonl').write_text(plain)
                self.assertEqual(collector.collect()['rows'], rows)
                imported = root/'import.json'
                import json
                imported.write_text(json.dumps(dict(events=collector.local_events)))
                collector.import_path = imported
                merged = collector.collect()
                self.assertEqual(merged['events'], 2)
                self.assertEqual(sum(r['total'] for r in merged['rows']), 180)
                self.assertTrue(all(r['device'] == 'shared' for r in merged['rows']))
                self.assertEqual(archive.read_bytes(), original)

    @unittest.skipUnless(jsonl.zstd is not None or jsonl.shutil.which('zstd'), 'Zstandard decoder unavailable')
    def test_archive_cache_is_reused_and_removed_files_are_pruned(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = root/'old.jsonl.zst'
            path.write_bytes(ARCHIVE)
            adapter = jsonl.CodexAdapter([root])
            with patch('adapters.jsonl.parse_log', wraps=jsonl.parse_log) as parse:
                self.assertEqual(len(adapter.scan().events), 2)
                adapter.scan()
                self.assertEqual(parse.call_count, 1)
                path.unlink()
                self.assertEqual(adapter.scan().files, 0)
                self.assertEqual(adapter.cache, {})

    def test_missing_decoder_reports_partial_scan_instead_of_silent_omission(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'old.jsonl.zst').write_bytes(ARCHIVE)
            with patch('adapters.jsonl.zstd', None), patch('adapters.jsonl.shutil.which', return_value=None):
                result = jsonl.CodexAdapter([root]).scan()
            self.assertEqual(result.files, 1)
            self.assertEqual(result.events, [])
            self.assertIn('requires Python 3.14+ or zstd', ' '.join(result.errors))

    @unittest.skipUnless(jsonl.zstd is not None or jsonl.shutil.which('zstd'), 'Zstandard decoder unavailable')
    def test_invalid_archive_is_reported_and_not_cached(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'broken.jsonl.zst').write_bytes(b'not a zstd frame')
            adapter = jsonl.CodexAdapter([root])
            result = adapter.scan()
            self.assertEqual(result.events, [])
            self.assertTrue(result.errors)
            self.assertEqual(adapter.cache, {})
