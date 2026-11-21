#!/usr/bin/env python3
import importlib.util
import os
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
INSTALL = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('test_install', INSTALL / 'test_install.py')
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
sys.path.insert(0, str(INSTALL / 'tests' / 'installer'))
from test_installer import Download, Path_, Preflight, Repair


class LinkedConfig(base.HarnessSkills):
    def linked_config(self, opencode):
        dotfiles = self.base / 'dotfiles'
        (dotfiles / ('opencode' if opencode else 'other')).mkdir(parents=True)
        os.symlink(dotfiles, self.home / '.config')
        (self.home / '.claude').mkdir()
        return dotfiles

    def test_an_absent_harness_under_a_linked_config_blocks_nothing(self):
        dotfiles = self.linked_config(opencode=False)
        code, result = self.skills()
        self.assertEqual((code, result.get('outcome')), (0, 'installed'), result)
        rows = {r['id']: r for r in result['harnesses']}
        self.assertEqual(rows['opencode']['state'], 'absent')
        self.assertEqual(rows['claude-code']['state'], 'installed')
        self.assertEqual(base.skill_files(self.home / '.claude/skills'),
                         base.skill_files(self.root / 'current/runtime/skills'))
        self.assertEqual(base.snapshot(dotfiles), {'other': 'dir'})

    @unittest.skipIf(os.getuid() == 0, 'root ignores search permission')
    def test_a_linked_config_without_search_permission_is_refused_on_its_own_row(self):
        dotfiles = self.linked_config(opencode=True)
        dotfiles.chmod(0o600)
        self.addCleanup(dotfiles.chmod, 0o700)
        before = base.snapshot(self.home)
        code, result = self.skills()
        self.assertEqual((code, result.get('schema'), result.get('outcome')),
                         (5, 'openwiz.skills-install.v1', 'refused'), result)
        self.assertEqual(base.snapshot(self.home), before)
        rows = {r['id']: r for r in result['harnesses']}
        self.assertEqual({h: r['state'] for h, r in rows.items()},
                         {'claude-code': 'installed', 'codex': 'absent', 'cursor': 'absent',
                          'opencode': 'refused', 'grok': 'absent'})
        self.assertIn(str(self.home / '.config'), rows['opencode']['next'])
        self.assertIn('chmod u+rx', rows['opencode']['next'])
        self.assertIn('run this again', rows['opencode']['next'])
        dotfiles.chmod(0o700)
        self.assertEqual(base.snapshot(dotfiles), {'opencode': 'dir'})

    def test_a_config_file_in_place_of_the_folder_means_opencode_is_absent(self):
        (self.home / '.config').write_bytes(b'Synthetic dotfile.\n')
        (self.home / '.claude').mkdir()
        code, result = self.skills()
        self.assertEqual((code, result.get('outcome')), (0, 'installed'), result)
        self.assertEqual({r['id']: r['state'] for r in result['harnesses']}['opencode'], 'absent')
        self.assertEqual((self.home / '.config').read_bytes(), b'Synthetic dotfile.\n')

    def test_a_present_harness_behind_a_linked_config_is_refused_on_its_own_row(self):
        dotfiles = self.linked_config(opencode=True)
        before = base.snapshot(self.home), base.snapshot(dotfiles)
        code, result = self.skills()
        self.assertEqual((code, result.get('outcome')), (5, 'refused'), result)
        self.assertEqual((base.snapshot(self.home), base.snapshot(dotfiles)), before)
        rows = {r['id']: r for r in result['harnesses']}
        self.assertEqual({h: r['state'] for h, r in rows.items()},
                         {'claude-code': 'installed', 'codex': 'absent', 'cursor': 'absent',
                          'opencode': 'refused', 'grok': 'absent'})
        self.assertIn(str(self.home / '.config'), rows['opencode']['next'])
        self.assertIn('link', rows['opencode']['next'])
        self.assertIn('run this again', rows['opencode']['next'])


def load_tests(loader, tests, pattern):
    suite = loader.loadTestsFromNames(sorted(name for name in LinkedConfig.__dict__ if name.startswith('test_')),
                                      LinkedConfig)
    suite.addTests(loader.loadTestsFromTestCase(case) for case in (Download, Path_, Preflight, Repair))
    return suite


if __name__ == '__main__':
    unittest.main()
