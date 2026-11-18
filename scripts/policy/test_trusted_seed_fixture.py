#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Alexey Zhokhov
# SPDX-License-Identifier: Apache-2.0
import hashlib
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BUILDER = ROOT / 'tests/build-trusted-seed-fixture.py'
CHECKER = ROOT / 'scripts/check-generated-workflows.sh'
PACK = 'tests/fixtures/trusted-seed-baselines.pack'
DIGEST = '44f010a183cf42d933e61622c00b49c944842c096641063d7057345b951140f7'


class SeedFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'source'
        self.root.mkdir()
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        for name in ['scripts/trusted-seed.commit', 'scripts/trusted-seed-metadata.pack', 'scripts/trusted-seed-metadata.objects', PACK]:
            target = self.root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, target)
        self.builder = Path(self.temp.name) / 'builder.py'
        shutil.copyfile(BUILDER, self.builder)
        self.output = Path(self.temp.name) / 'seed.git'

    def invoke(self):
        env = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': self.temp.name, 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_TERMINAL_PROMPT': '0'}
        return subprocess.run(['/usr/bin/python3', '-B', '-I', '-S', str(self.builder), str(self.root), str(CHECKER), str(self.output)], env=env, capture_output=True, text=True)

    def test_empty_source_repository_succeeds(self):
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any((self.root / '.git/objects').glob('[0-9a-f][0-9a-f]/*')))

    def test_missing_tampered_oversized_and_symlink_pack_fail(self):
        pack = self.root / PACK
        original = pack.read_bytes()
        for variant in ('missing', 'tampered', 'oversized', 'symlink'):
            if pack.exists() or pack.is_symlink():
                pack.unlink()
            if variant == 'tampered':
                pack.write_bytes(original + b'x')
            elif variant == 'oversized':
                pack.write_bytes(b'x' * (1024 * 1024 + 1))
            elif variant == 'symlink':
                pack.symlink_to(ROOT / PACK)
            result = self.invoke()
            self.assertNotEqual(result.returncode, 0, variant)
            if self.output.exists():
                shutil.rmtree(self.output)

    def test_valid_pack_with_wrong_inventory_fails(self):
        pack = self.root / PACK
        empty = subprocess.run(['git', 'pack-objects', '--stdout'], input=b'', cwd=self.root, capture_output=True, check=True).stdout
        pack.write_bytes(empty)
        script = self.builder.read_text()
        self.builder.write_text(script.replace(DIGEST, hashlib.sha256(empty).hexdigest()))
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('complete seed object inventory differs', result.stderr)

    def test_pack_with_extra_object_fails(self):
        pack = self.root / PACK
        subprocess.run(['git', 'index-pack', '--stdin'], cwd=self.root, input=pack.read_bytes(), capture_output=True, check=True)
        subprocess.run(['git', 'hash-object', '-w', '--stdin'], cwd=self.root, input=b'unreviewed extra object', capture_output=True, check=True)
        identities = subprocess.run(['git', 'cat-file', '--batch-all-objects', '--batch-check=%(objectname)'], cwd=self.root, capture_output=True, check=True).stdout
        extra = subprocess.run(['git', 'pack-objects', '--stdout'], cwd=self.root, input=identities, capture_output=True, check=True).stdout
        pack.write_bytes(extra)
        self.builder.write_text(self.builder.read_text().replace(DIGEST, hashlib.sha256(extra).hexdigest()))
        result = self.invoke()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('complete seed object inventory differs', result.stderr)


if __name__ == '__main__':
    unittest.main()
