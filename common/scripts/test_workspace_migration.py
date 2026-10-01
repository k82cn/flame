# Copyright 2026 The Flame Authors.
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#     http://www.apache.org/licenses/LICENSE-2.0
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
"""Regression tests for package URLs in the offline SQLite migration."""
from pathlib import Path
import sqlite3
import unittest


class PackageMigrationTests(unittest.TestCase):
    def test_legacy_cache_packages_keep_authority_and_gain_default_workspace(self):
        connection = sqlite3.connect(':memory:')
        scripts = sorted((Path(__file__).resolve().parents[1] / 'migrations/sqlite').glob('*.sql'))
        for script in scripts[:-1]:
            connection.executescript(script.read_text())
        urls = ['grpc://cache:9090/app/pkg/archive?version=4',
                'grpcs://cache:9443/app/pkg/archive',
                'grpc+tls://cache:9443/app/pkg/archive',
                'grpcs-proxy://gateway:9443/app/pkg/archive',
                'https://example.com/archive', 'file:///tmp/archive', None]
        for index, url in enumerate(urls):
            connection.execute('INSERT INTO applications (name,shim,max_instances,delay_release,creation_time,state,url) VALUES (?,0,1,0,0,0,?)', (str(index), url))
        connection.executescript(scripts[-1].read_text())
        for index, original in enumerate(urls):
            actual = connection.execute('SELECT url FROM applications WHERE name=?', (str(index),)).fetchone()[0]
            expected = original.replace('/app/pkg/', '/default/app/pkg/') if original and index < 4 else original
            self.assertEqual(actual, expected)
        self.assertEqual(connection.execute('PRAGMA foreign_key_check').fetchall(), [])


if __name__ == '__main__':
    unittest.main()
