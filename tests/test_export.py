import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class ExportTests(unittest.TestCase):
    def test_demo_export_embeds_data_styles_and_module_without_dependencies(self):
        root = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory) / 'snapshot.html'
            subprocess.run([sys.executable, str(root/'src/server.py'), '--demo', '--export', str(target)], check=True, capture_output=True, cwd=directory)
            page = target.read_text()
            self.assertNotIn('src="/app.js"', page)
            self.assertNotIn('src="/combobox.js"', page)
            self.assertIn('window.ChroniclesCombobox', page)
            self.assertNotIn('src="/calendar.js"', page)
            self.assertIn('window.ChroniclesCalendar', page)
            self.assertNotIn('href="/style.css"', page)
            self.assertNotIn('href="/shadcn.css"', page)
            self.assertIn('--primary-foreground:', page)
            self.assertNotIn('from "./analytics.mjs"', page)
            self.assertNotIn('from "./pricing.mjs"', page)
            self.assertIn('const TOKEN_PRICES', page)
            self.assertIn('gemini-4-argon', page)
            self.assertIn('<style>', page)
            self.assertIn('data:image/svg+xml,', page)
            self.assertIn('data:font/woff2;base64,', page)
            self.assertNotIn('/Geist.woff2', page)
            payload = re.search(r'window.CHRONICLES_DATA=(.*?);</script>', page, re.S).group(1)
            report = json.loads(payload)
            self.assertTrue(report['demo'])
            self.assertTrue(report['hourlyRows'])
            self.assertEqual(sum(row['requests'] for row in report['rows']), sum(row['requests'] for row in report['hourlyRows']))
