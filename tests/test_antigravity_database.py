import sqlite3
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from adapters.antigravity import read_database


class AntigravityDatabaseTests(unittest.TestCase):
    def test_snapshot_preserves_uncheckpointed_wal_and_cleans_up(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'conversation.db'
            writer = sqlite3.connect(path)
            self.addCleanup(writer.close)
            writer.execute('PRAGMA journal_mode=WAL')
            writer.execute('CREATE TABLE gen_metadata (idx INTEGER, data BLOB)')
            writer.commit()
            writer.execute('PRAGMA wal_checkpoint(TRUNCATE)')
            writer.execute("INSERT INTO gen_metadata VALUES (1, X'0102')")
            writer.commit()
            originals = {suffix: Path(str(path)+suffix).read_bytes()
                         for suffix in ('', '-wal', '-shm')}
            cantopen = sqlite3.OperationalError('unable to open database file')
            cantopen.sqlite_errorcode = sqlite3.SQLITE_CANTOPEN
            connect = sqlite3.connect
            snapshots = []

            def open_database(database, **kwargs):
                if database == path.as_uri()+'?mode=ro':
                    raise cantopen
                connection = connect(database, **kwargs)
                snapshots.append(Path(connection.execute('PRAGMA database_list').fetchone()[2]))
                return connection

            with patch('adapters.antigravity.sqlite3.connect', side_effect=open_database):
                with read_database(path) as reader:
                    self.assertEqual(reader.execute('SELECT idx, data FROM gen_metadata').fetchall(), [(1, b'\x01\x02')])
            self.assertEqual(len(snapshots), 1)
            self.assertFalse(snapshots[0].parent.exists())
            for suffix, original in originals.items():
                self.assertEqual(Path(str(path)+suffix).read_bytes(), original)
            writer.close()

    def test_other_sqlite_errors_are_not_retried_as_snapshots(self):
        busy = sqlite3.OperationalError('database is locked')
        busy.sqlite_errorcode = sqlite3.SQLITE_BUSY
        with patch('adapters.antigravity.sqlite3.connect', side_effect=busy) as connect:
            with self.assertRaisesRegex(sqlite3.OperationalError, 'locked'):
                with read_database(Path('/unused.db')):
                    self.fail('unexpected connection')
            self.assertEqual(connect.call_count, 1)
