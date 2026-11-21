#!/usr/bin/env python3
from contextlib import redirect_stdout
from http.server import BaseHTTPRequestHandler, HTTPServer
from importlib.machinery import SourceFileLoader
import shlex
import shutil
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import threading
import unittest

sys.dont_write_bytecode = True
INSTALL = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(INSTALL))
import test_install
import pull

spec = importlib.util.spec_from_loader('install_entry', SourceFileLoader('install_entry', str(INSTALL / 'install')))
entry = importlib.util.module_from_spec(spec)
spec.loader.exec_module(entry)
TARGET = 'x86_64-unknown-linux-gnu'
NAME = 'openwiz-' + TARGET
ARCHIVE = 'openwiz-0.1.0-' + TARGET + '.tar.gz'


class Host(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    routes = {}
    seen = []
    truncated = set()

    def do_GET(self):
        Host.seen.append(self.path)
        body = Host.routes.get(self.path)
        self.send_response(200 if body is not None else 404)
        if self.path in Host.truncated:
            self.send_header('Transfer-Encoding', 'chunked')
            self.end_headers()
            self.wfile.write(b'%x\r\n' % len(body) + body[:len(body) // 2])
            self.close_connection = True
            return
        body = body if body is not None else b'Not Found'
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


class Installer(test_install.Bundles):
    def setUp(self):
        super().setUp()
        self.server = HTTPServer(('127.0.0.1', 0), Host)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        base = 'http://127.0.0.1:%d/openwizdev/OpenWiz/releases' % self.server.server_port
        for key, value in (('BASE', base + '/'), ('FEED', base + '.atom')):
            self.addCleanup(setattr, pull, key, getattr(pull, key, None))
            setattr(pull, key, value)
        Host.routes, Host.seen, Host.truncated = {}, [], set()
        self.home = self.base / 'home'
        self.home.mkdir(mode=0o700)
        for key in ('HOME', 'PATH'):
            self.addCleanup(os.environ.__setitem__, key, os.environ[key])
        os.environ['HOME'] = str(self.home)
        self.root = self.home / '.local/lib/openwiz'

    def run_entry(self, *argv, host=None):
        out = io.StringIO()
        with redirect_stdout(out):
            code = entry.main(list(argv), host=host)
        return code, json.loads(out.getvalue())

    def publish(self, tag, version='0.1.0'):
        manifest, archive, _ = self.bundle(version=version, mutate=lambda m: m.update(archive=ARCHIVE))
        raw, data = manifest.read_bytes(), archive.read_bytes()
        prefix = '/openwizdev/OpenWiz/releases/download/' + tag + '/'
        sums = '%s  openwiz-%s-%s.manifest.json\n%s  %s\n' % (test_install.sha(raw), version, TARGET,
                                                             test_install.sha(data), ARCHIVE)
        Host.routes.update({prefix + NAME + '.sha256': test_install.sha(raw).encode() + b'  ' + NAME.encode()
                            + b'.manifest.json\n', prefix + NAME + '.manifest.json': raw,
                            prefix + ARCHIVE: data, prefix + 'SHA256SUMS': sums.encode()})
        return prefix

    def feed(self, *tags):
        Host.routes['/openwizdev/OpenWiz/releases.atom'] = ''.join(
            '<entry><link rel="alternate" type="text/html" href="https://github.com/openwizdev/OpenWiz/releases/tag/%s"/>'
            '</entry>' % tag for tag in tags).encode()

    def assets(self, name='assets', override=None):
        folder = self.base / name
        manifest, archive, _ = self.bundle(version='0.1.0', mutate=lambda m: m.update(archive=ARCHIVE),
                                           override=override)
        folder.mkdir(mode=0o700)
        raw = manifest.read_bytes()
        for name, data in ((NAME + '.sha256', test_install.sha(raw).encode() + b'  ' + NAME.encode() + b'.manifest.json\n'),
                           (NAME + '.manifest.json', raw), (ARCHIVE, archive.read_bytes())):
            (folder / name).write_bytes(data)
            (folder / name).chmod(0o600)
        return folder


class Download(Installer):
    def test_a_404_on_latest_falls_back_to_the_newest_published_tag(self):
        self.publish('v0.0.12', '0.0.12')
        self.publish('v0.1.0')
        self.feed('v0.0.12', 'v0.1.0')
        code, value = self.run_entry('download', '--target', TARGET, '--dir', str(self.base / 'dl'))
        self.assertEqual((code, value['outcome'], value['version']), (0, 'downloaded', '0.1.0'), value)
        self.assertEqual(Host.seen[0], '/openwizdev/OpenWiz/releases/latest/download/' + NAME + '.sha256')
        self.assertEqual(sorted(p.name for p in (self.base / 'dl').iterdir()),
                         sorted([NAME + '.sha256', NAME + '.manifest.json', ARCHIVE]))

    def test_the_refusal_names_the_404_and_its_url(self):
        self.feed()
        code, value = self.run_entry('download', '--target', TARGET, '--dir', str(self.base / 'dl'))
        self.assertEqual(code, 5)
        self.assertIn('release asset 404 at ' + pull.BASE + 'latest/download/' + NAME + '.sha256', value['reason'])
        self.assertIn('no published release tag found at ' + pull.FEED, value['reason'])
        self.assertFalse((self.base / 'dl').exists())

    def test_version_downloads_that_tag_only(self):
        self.publish('v0.1.0')
        for version in ('v0.1.0', '0.1.0'):
            with self.subTest(version=version):
                Host.seen.clear()
                folder = self.base / ('dl-' + version)
                code, value = self.run_entry('download', '--target', TARGET, '--dir', str(folder), '--version', version)
                self.assertEqual((code, value['version']), (0, '0.1.0'), value)
                self.assertTrue(all(p.startswith('/openwizdev/OpenWiz/releases/download/v0.1.0/') for p in Host.seen))
        code, value = self.run_entry('download', '--target', TARGET, '--dir', str(self.base / 'dl-x'), '--version', 'v0.0.12')
        self.assertEqual(code, 5)
        self.assertIn('release asset 404 at ' + pull.BASE + 'download/v0.0.12/' + NAME + '.sha256', value['reason'])
        code, value = self.run_entry('download', '--target', TARGET, '--dir', str(self.base / 'dl-y'), '--version', 'latest')
        self.assertEqual(code, 2)

    def test_a_tag_whose_manifest_names_another_version_is_refused(self):
        self.publish('v0.1.0', '0.0.99')
        code, value = self.run_entry('download', '--target', TARGET, '--dir', str(self.base / 'dl'), '--version', 'v0.1.0')
        self.assertEqual(code, 5)
        self.assertIn('published version does not match its tag', value['reason'])


    def test_every_downloaded_file_is_checked_before_success_on_every_route(self):
        for route in ('latest', 'fallback', 'v0.1.0'):
            argv = ('--version', route) if route == 'v0.1.0' else ()
            for case in ('archive', 'sums', 'no sums', 'truncated'):
                with self.subTest(route=route, case=case):
                    Host.routes.clear()
                    Host.truncated.clear()
                    prefix = self.publish('v0.1.0')
                    self.feed('v0.1.0')
                    if route == 'latest':
                        for name in (NAME + '.sha256', NAME + '.manifest.json'):
                            Host.routes['/openwizdev/OpenWiz/releases/latest/download/' + name] = Host.routes[prefix + name]
                    if case == 'archive':
                        data = Host.routes[prefix + ARCHIVE]
                        Host.routes[prefix + ARCHIVE] = data[:-1] + bytes([data[-1] ^ 1])
                    elif case == 'sums':
                        Host.routes[prefix + 'SHA256SUMS'] = Host.routes[prefix + 'SHA256SUMS'].replace(
                            test_install.sha(Host.routes[prefix + ARCHIVE]).encode(), b'0' * 64)
                    elif case == 'no sums':
                        del Host.routes[prefix + 'SHA256SUMS']
                    else:
                        Host.truncated.add(prefix + ARCHIVE)
                    folder = self.base / ('dl-%s-%s' % (route, case.replace(' ', '-')))
                    code, value = self.run_entry('download', '--target', TARGET, '--dir', str(folder), *argv)
                    self.assertEqual((code, value['outcome']), (5, 'refused'), value)
                    self.assertIn({'archive': 'archive checksum mismatch',
                                   'sums': 'SHA256SUMS does not match ' + ARCHIVE,
                                   'no sums': 'release asset 404 at ' + pull.BASE + 'download/v0.1.0/SHA256SUMS',
                                   'truncated': 'interrupted transfer at ' + pull.BASE + 'download/v0.1.0/' + ARCHIVE
                                   }[case], value['reason'])
                    self.assertFalse(folder.exists())


class Preflight(Installer):
    def ldd(self, line):
        folder = self.base / ('bin-' + str(self.counter))
        self.counter += 1
        folder.mkdir()
        (folder / 'ldd').write_text('#!/bin/sh\necho "%s"\necho "Copyright (C) synthetic"\n' % line)
        (folder / 'ldd').chmod(0o755)
        os.environ['PATH'] = str(folder) + os.pathsep + '/usr/bin:/bin'

    def glibc(self, host=('Linux', 'x86_64')):
        code, value = self.run_entry('preflight', host=host)
        return code, value, [c for c in value['checks'] if c['id'] == 'glibc']

    def test_linux_refuses_a_glibc_older_than_2_34_and_names_the_version_found(self):
        self.ldd('ldd (Ubuntu GLIBC 2.31-0ubuntu9.16) 2.31')
        code, value, check = self.glibc()
        self.assertEqual((code, value['ready'], check[0]['state']), (5, False, 'refused'))
        self.assertIn('2.34', check[0]['next'])
        self.assertIn('glibc 2.31', check[0]['next'])

    def test_linux_accepts_glibc_2_34_and_later(self):
        for found in ('2.34', '2.36', '2.40'):
            with self.subTest(found=found):
                self.ldd('ldd (Debian GLIBC %s-9) %s' % (found, found))
                code, value, check = self.glibc()
                self.assertEqual(check[0]['state'], 'ok', check)
                self.assertIn('glibc ' + found, check[0]['next'])

    def test_an_unreadable_glibc_is_refused_and_macos_has_no_glibc_check(self):
        self.ldd('musl libc (x86_64)')
        code, value, check = self.glibc()
        self.assertEqual((code, check[0]['state']), (5, 'refused'))
        self.assertIn('could not read', check[0]['next'])
        self.assertEqual(self.glibc(('Darwin', 'arm64'))[2], [])


class Repair(Installer):
    def test_install_replaces_a_build_that_fails_verification(self):
        folder = self.assets()
        code, value = self.run_entry('install', '--assets', str(folder), '--target', TARGET, '--root', str(self.root))
        self.assertEqual(code, 0, value)
        damaged = Path(value['build']) / 'runtime/setup/setup.py'
        with damaged.open('ab') as handle:
            handle.write(b'synthetic damage\n')
        (self.root / 'state' / 'owner-settings.json').write_text('synthetic owner settings')
        self.assertEqual(self.run_entry('status', '--root', str(self.root))[0], 5)
        code, value = self.run_entry('install', '--assets', str(folder), '--target', TARGET, '--root', str(self.root))
        self.assertEqual((code, value['outcome'], value['repaired']), (0, 'installed', True), value)
        self.assertEqual(damaged.read_bytes(), b'Synthetic runtime\n')
        self.assertEqual(self.run_entry('status', '--root', str(self.root))[1]['outcome'], 'installed')
        self.assertEqual((self.root / 'state' / 'owner-settings.json').read_text(), 'synthetic owner settings')
        self.assertEqual([p.name for p in (self.root / 'state/service/builds').iterdir()], [value['source']])

    def test_install_repairs_a_missing_or_mispermissioned_selected_build(self):
        folder = self.assets()
        for n, case in enumerate(('missing', 'mode 0755', 'mode 0000', 'interrupted')):
            with self.subTest(case=case):
                root = self.home / ('.local/lib/openwiz-%d' % n)
                argv = ('install', '--assets', str(folder), '--target', TARGET, '--root', str(root))
                build = Path(self.run_entry(*argv)[1]['build'])
                (root / 'state' / 'owner-settings.json').write_text('synthetic owner settings')
                if case == 'missing':
                    shutil.rmtree(build)
                elif case == 'interrupted':
                    build.rename(build.parent / ('.damaged-1-' + build.name))
                else:
                    build.chmod(int(case[-4:], 8))
                code, value = self.run_entry(*argv)
                self.assertEqual((code, value['outcome']), (0, 'installed'), value)
                self.assertEqual(self.run_entry('status', '--root', str(root))[1]['outcome'], 'installed')
                self.assertEqual([p.name for p in build.parent.iterdir()], [build.name])
                self.assertEqual((root / 'state' / 'owner-settings.json').read_text(), 'synthetic owner settings')

    def test_a_retry_after_an_interrupted_repair_keeps_the_source_bound_to_its_manifest(self):
        good = self.assets()
        other = self.assets('other', override={'openwiz': b'#!/bin/sh\nexit 1\n'})
        argv = ('--target', TARGET, '--root', str(self.root))
        build = Path(self.run_entry('install', '--assets', str(good), *argv)[1]['build'])
        original = (build / 'install-manifest.json').read_bytes()
        backup = build.parent / ('.damaged-1-' + build.name)
        build.rename(backup)
        code, value = self.run_entry('install', '--assets', str(other), *argv)
        self.assertEqual(code, 5, value)
        self.assertIn('different manifest', value['reason'])
        self.assertEqual((backup / 'install-manifest.json').read_bytes(), original)
        self.assertFalse(build.exists())
        code, value = self.run_entry('install', '--assets', str(good), *argv)
        self.assertEqual((code, value['outcome']), (0, 'installed'), value)
        self.assertEqual((build / 'install-manifest.json').read_bytes(), original)
        self.assertEqual([p.name for p in build.parent.iterdir()], [build.name])

    def test_status_says_not_installed_on_an_empty_root(self):
        for root in (self.root, self.home):
            with self.subTest(root=root):
                code, value = self.run_entry('status', '--root', str(root))
                self.assertEqual((code, value['outcome']), (1, 'not installed'), value)
                self.assertNotIn('retained', json.dumps(value))


class Path_(Installer):
    def test_install_prints_where_openwiz_is_and_link_previews_then_links_on_yes(self):
        code, value = self.run_entry('install', '--assets', str(self.assets()), '--target', TARGET, '--root', str(self.root))
        self.assertEqual(value['openwiz'], str(self.root / 'current/openwiz'))
        self.assertIn('install link --root ' + str(self.root), value['next'])
        link = self.home / '.local/bin/openwiz'
        code, value = self.run_entry('link', '--root', str(self.root))
        self.assertEqual((code, value['outcome'], value['link'], value['to']),
                         (0, 'preview', str(link), str(self.root / 'current/openwiz')), value)
        self.assertFalse(link.exists() or link.is_symlink())
        code, value = self.run_entry('link', '--root', str(self.root), '--yes')
        self.assertEqual((code, value['outcome']), (0, 'linked'), value)
        self.assertEqual(os.readlink(link), str(self.root / 'current/openwiz'))
        self.assertEqual(self.run_entry('link', '--root', str(self.root), '--yes')[1]['outcome'], 'linked')

    def test_the_printed_link_commands_carry_a_custom_state_root(self):
        state = self.base / 'synthetic state'
        code, value = self.run_entry('install', '--assets', str(self.assets()), '--target', TARGET,
                                     '--root', str(self.root), '--state-root', str(state))
        self.assertEqual(code, 0, value)
        preview = shlex.split(value['next'].split('run ', 1)[1].split(' to preview')[0])
        self.assertEqual(preview, ['install', 'link', '--root', str(self.root), '--state-root', str(state)])
        code, value = self.run_entry(*preview[1:])
        self.assertEqual((code, value['outcome']), (0, 'preview'), value)
        confirm = shlex.split(value['next'].split('Run ', 1)[1].split(' to make it')[0])
        self.assertEqual(confirm, preview + ['--yes'])
        self.assertEqual(self.run_entry(*confirm[1:])[1]['outcome'], 'linked')

    def test_link_never_replaces_the_users_own_openwiz(self):
        self.run_entry('install', '--assets', str(self.assets()), '--target', TARGET, '--root', str(self.root))
        link = self.home / '.local/bin/openwiz'
        link.parent.mkdir(parents=True)
        link.write_text('synthetic user program')
        for argv in ((), ('--yes',)):
            code, value = self.run_entry('link', '--root', str(self.root), *argv)
            self.assertEqual(code, 5)
            self.assertIn(str(link) + ' already exists', value['reason'])
        self.assertEqual(link.read_text(), 'synthetic user program')
        code, value = self.run_entry('link', '--root', str(self.base / 'nothing'), '--yes')
        self.assertEqual(code, 5)


if __name__ == '__main__':
    unittest.main()
