#!/usr/bin/env python3
"""The cold-consumer install journey (#1109, IF1-IF9 integration; joined in #636), on fixtures only.

From the one install line to a first fleet, in the confirmed order the packaged
install skill gives (its numbered sections, read from the installed copy): the
actual entry `tools/install/install` checks the machine, downloads a synthetic
release through an injected transport and installs it; the installed runtime's
own setup tool checks connections against T3's recording fakes; and the first
fleet replays the onboarding's recorded conversation through the installed
onboarding helper and launch-fleet scripts. At its Connect step the installed
setup, sign-in skip, App and `openwiz init` tools run for the one chosen
repository against one recording GitHub (T5's world, through gh.py), and the
onboarding carries the state they leave (#636). No engine or tick is run: the
first fleet is one launch-fleet fleet thread. The consumer is a plain folder
with no OpenWiz sources and no skills checkout. Recording `cargo`, `rustc`,
`npm`, `node` and `npx` come first on PATH: any call fails the journey.

Fixtures prove the composition, its gates and its refusals; never a real
release, account, machine, elapsed time or live behaviour.
"""
import hashlib
import importlib.machinery
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import socket
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, HTTPServer
import unittest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
TOOLS = ROOT / 'tools/install'
ENTRY = TOOLS / 'install'
CONNECT = ROOT / 'tools/install/tests/fixtures/install-connections'
ONBOARD_TESTS = TOOLS / 'tests/onboard'
TARGET = 'x86_64-unknown-linux-gnu'
VERSION = '0.0.99'
NAME = 'openwiz-' + TARGET
BASE = 'https://github.com/openwizdev/OpenWiz/releases/'
LINE = 'Install OpenWiz from github.com/openwizdev/OpenWiz'
ORDER = ['Preflight', 'Verified install', 'Setup and connections', 'Explain', 'First fleet']
# #2293 (PR6, copy only): independent literals for the free-only release copy.
FREE_ONLY = ('OpenWiz is free now. Paid plans come later. '
             'CloudRoom and model charges are separate and not included.')
NO_REFERRAL = 'Referral verification is unavailable. No referral is applied.'
NOT_OFFERED = re.compile(r'upgrad|\btrial|\$\s?[0-9]|per month|checkout|subscri', re.I)
BUILD_TOOLS = ('cargo', 'rustc', 'rustup', 'npm', 'node', 'npx')
REPO = 'synthetic-owner/synthetic-consumer'
PROFILE = {'metadata': 'read', 'contents': 'write', 'issues': 'write', 'pull_requests': 'write',
           'checks': 'read', 'actions': 'read'}
TOKEN = 'ghs_SyntheticInstallationToken0000000000000001'
PERSONAL = 'gho_SyntheticPersonalToken00000000000000000002'


def settings_table(title, provider, model):
    return ('## %s\n\n| Setting | Value | `room-cli thread spawn` flag |\n| --- | --- | --- |\n'
            '| Harness | Synthetic | `--provider %s` |\n| Model | Synthetic | `--model %s` |\n'
            '| Effort | High | `--reasoning-level high` |\n\nSource: synthetic owner, 2026-11-01.\n\n' % (title, provider, model))


# The fleet settings the setup skill writes from the owner's recorded choice (#636).
FLEET_SETTINGS = ('# Fleet settings\n\nSynthetic settings.\n\n'
                  + settings_table('Builder', 'codex', 'syn-codex-model-01')
                  + settings_table('Builder for small tickets', 'codex', 'syn-codex-model-01')
                  + settings_table('Reviewer for GPT-built', 'claude-code', 'syn-claude-model-01')
                  + '## Test commands\n\n| # | Folder | Command | CI check |\n| --- | --- | --- | --- |\n'
                  '| 1 | root | `make test` | `test` |\n\nSource: the synthetic consumer\'s CI workflow.\n\n'
                  '## CI checks\n\n| Check | Runs | Required on |\n| --- | --- | --- |\n| `test` | 1 | `main` |\n\n'
                  'Source: the synthetic consumer\'s CI workflow.\n\n## Standing merge rules\n\n'
                  'None yet: every merge waits for the user.\n\nSource: synthetic owner, 2026-11-01.\n')

sys.path.insert(0, str(TOOLS))
sys.dont_write_bytecode = True
from install import PROGRAMS, REQUIRED, sha  # noqa: E402


def module(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    value = importlib.util.module_from_spec(spec)
    loader.exec_module(value)
    return value


def entry():
    """The actual install entry, loaded as a module so the transport can be injected."""
    return module('openwiz_install_entry', ENTRY)


def tracked(folder, prefix):
    """Every tracked regular file under a source folder, at its runtime path."""
    out = {}
    for path in sorted(folder.rglob('*')):
        if (path.is_file() and not path.is_symlink() and '__pycache__' not in path.parts
                and not any(part.startswith('.') for part in path.relative_to(folder).parts)):
            out[prefix + path.relative_to(folder).as_posix()] = path.read_bytes()
    return out


def bundle_assets(source):
    """A synthetic release bundle holding the tracked runtime the journey runs.

    The program is a fixture: `openwiz` is the journey's dispatcher. Every
    runtime file is the repository's own bytes."""
    assets = {path: b'Synthetic runtime, no effects.\n' for path in REQUIRED}
    for program in PROGRAMS:
        assets[program] = b'#!/bin/sh\nexit 0\n'
    assets['openwiz'] = (HERE / 'fake_openwiz').read_bytes()
    assets['runtime/cloudroom/cloudroom.py'] = (HERE / 'fake_cloudroom.py').read_bytes()  # #636: recording CloudRoom
    for name in ('install.py', 'pull.py'):
        assets['runtime/' + name] = (TOOLS / name).read_bytes()
    assets.update(tracked(TOOLS / 'setup', 'runtime/setup/'))
    assets.update(tracked(ROOT / 'tools/github-app', 'runtime/github-app/'))
    assets['runtime/access/access.py'] = (TOOLS / 'access/access.py').read_bytes()
    for skill in ('setup', 'grill', 'to-spec', 'to-tickets', 'launch-fleet', 'install'):  # the 0.2.0 catalog (#1187)
        assets.update(tracked(ROOT / 'skills' / skill, f'runtime/skills/{skill}/'))
    assets = {k: v for k, v in assets.items() if not re.search(r'(^|/)test[^/]*$|/tests?/', k)}
    identity = f'openwiz.package.v1\n{VERSION}\n{source}\n{TARGET}\n'.encode()
    assets['runtime/openwiz-build.txt'] = identity
    return assets


def release(folder, source='1' * 40, tamper=False):
    """The release's published files, as GitHub serves them: checksum file, manifest, archive."""
    folder.mkdir(mode=0o700)
    assets = bundle_assets(source)
    archive_name = f'openwiz-{VERSION}-{TARGET}.tar.gz'
    buffer = io.BytesIO()
    rows = []
    with tarfile.open(fileobj=buffer, mode='w:gz', format=tarfile.USTAR_FORMAT) as tar:
        for path, data in sorted(assets.items()):
            mode = '0700' if path in PROGRAMS or path.endswith('.sh') else '0600'
            info = tarfile.TarInfo(path)
            info.size, info.mode = len(data), int(mode, 8)
            tar.addfile(info, io.BytesIO(data))
            rows.append({'path': path, 'sha256': sha(data), 'bytes': len(data), 'mode': mode})
    archive = buffer.getvalue()
    manifest = json.dumps({'schema': 'openwiz.install.v1', 'version': VERSION, 'source': source,
                           'target': TARGET, 'setup_api': 1, 'archive': archive_name,
                           'archive_sha256': sha(archive), 'archive_bytes': len(archive),
                           'assets': rows}).encode()
    sums = f'{sha(manifest)}  openwiz-{VERSION}-{TARGET}.manifest.json\n{sha(archive)}  {archive_name}\n'.encode()
    if tamper:
        archive = archive[:-1] + bytes([archive[-1] ^ 1])
    files = {NAME + '.sha256': f'{sha(manifest)}  {NAME}.manifest.json\n'.encode(),
             NAME + '.manifest.json': manifest, archive_name: archive, 'SHA256SUMS': sums}
    for name, data in files.items():
        (folder / name).write_bytes(data)
    return {'assets': assets, 'archive': archive_name, 'files': files}


class Transport:
    """GitHub's release downloads, from a local folder; every URL is recorded."""

    def __init__(self, published):
        self.published = published
        self.urls = []

    def __call__(self, url, limit):
        self.urls.append(url)
        files = self.published['files']
        for prefix in (BASE + 'latest/download/', BASE + f'download/v{VERSION}/'):
            if url.startswith(prefix) and url[len(prefix):] in files:
                data = files[url[len(prefix):]]
                if len(data) > limit:
                    raise ValueError('artifact exceeds its declared byte bound')
                return data
        raise OSError('synthetic: 404')


class AppAPI:
    """GitHub's App API on 127.0.0.1 for the installed setup check; records each request."""

    def __init__(self, case, repository=REPO):
        requests = self.requests = []

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def reply(self, status, value):
                body = json.dumps(value).encode()
                self.send_response(status)
                self.send_header('content-type', 'application/json')
                self.send_header('content-length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def do_GET(self):
                requests.append(('GET', self.path))
                if self.path == '/app':  # the setup check's App-owner read (#606 review R3)
                    return self.reply(200, {'id': 1001, 'owner': {'login': 'synthetic-owner'}})
                if self.path == '/app/installations':
                    return self.reply(200, [{'id': 2002, 'app_id': 1001, 'account': {'login': 'synthetic-owner'},
                                             'permissions': PROFILE, 'repository_selection': 'selected',
                                             'suspended_at': None}])
                self.reply(404, {'message': 'synthetic: unexpected route'})

            def do_POST(self):
                body = json.loads(self.rfile.read(int(self.headers.get('content-length', 0))) or b'{}')
                requests.append(('POST', self.path))
                if self.path == '/app/installations/2002/access_tokens':
                    expiry = time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime(time.time() + 3000))
                    return self.reply(201, {'token': TOKEN, 'expires_at': expiry,
                                            'permissions': body.get('permissions', {}),
                                            'repository_selection': 'selected',
                                            'repositories': [{'full_name': repository}]})
                self.reply(404, {'message': 'synthetic: unexpected route'})

        self.server = HTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        case.addCleanup(self.server.server_close)
        case.addCleanup(self.server.shutdown)
        self.base = 'http://127.0.0.1:%d' % self.server.server_address[1]


class Consumer:
    """A cold consumer: a private HOME, a plain project folder and recording build tools."""

    def __init__(self, case):
        self.case = case
        self.dir = Path(tempfile.mkdtemp(prefix='install-journey-'))
        case.addCleanup(shutil.rmtree, self.dir, True)
        self.dir.chmod(0o700)
        self.home = self.dir / 'home'
        self.home.mkdir(mode=0o700)
        self.project = self.dir / 'project'
        shutil.copytree(CONNECT / 'consumer', self.project)
        self.root = self.home / '.local/lib/openwiz'
        self.downloads = self.dir / 'downloads'
        self.shims = self.dir / 'shims'
        self.shims.mkdir()
        self.build_log = self.dir / 'build-tools.log'
        for tool in BUILD_TOOLS:
            shim = self.shims / tool
            shim.write_text('#!/bin/sh\necho "%s $*" >> "%s"\nexit 97\n' % (tool, self.build_log))
            shim.chmod(0o755)
        # One recording GitHub for setup, sign-in, App skip and init (#636): T5's
        # world through the journey's gh.py, alone in its own folder.
        self.gh = self.dir / 'gh'
        self.gh.mkdir()
        (self.gh / 'gh').write_text('#!/bin/sh\nexec /usr/bin/python3 -I "%s" "$@"\n' % (HERE / 'gh.py'))
        (self.gh / 'gh').chmod(0o755)
        self.world = self.dir / 'github.json'
        self.engine_log = self.dir / 'engine.log'
        self.github(REPO)
        self.fakes = self.dir / 'fakes'
        self.fakes.mkdir()
        os.symlink(ONBOARD_TESTS, self.fakes / 'onboard')
        os.symlink(CONNECT, self.fakes / 'connections')
        self.scenario = self.dir / 'scenario.json'
        self.calls = self.dir / 'calls.jsonl'
        self.scenario.write_text(json.dumps({
            'cloudroom': 'ok', 'repository': REPO, 'identity': 'person', 'settings': True,
            'issue': {'number': 7, 'state': 'open'}, 'threads': {'thr_synthetic_answer': 'prj_synthetic'},
            'harnesses': [{'harness': 'claude-code', 'provider': 'claude-code', 'attachments': None,
                           'steer': None, 'models': [{'model': 'synthetic-model', 'efforts': ['high']}]},
                          {'harness': 'codex', 'provider': 'codex', 'attachments': None, 'steer': None,
                           'models': [{'model': 'syn-codex-model-01', 'efforts': ['low', 'medium', 'high']}]}]}))
        self.outputs = []

    def github(self, *repositories):
        """T5's recording GitHub: each repository private, owned by the signed-in user,
        holding the filled fleet settings, with its service-record issue."""
        world = json.loads(self.world.read_text()) if self.world.exists() else {
            'user': {'login': 'synthetic-owner', 'type': 'User'}, 'repos': {}, 'faults': [], 'log': [], 'issues': {}}
        for name in repositories:
            world['repos'][name.lower()] = {
                'full_name': name, 'private': True, 'visibility': 'private', 'default_branch': 'main',
                'permissions': {'push': True}, 'owner': {'login': name.split('/')[0], 'type': 'User'},
                'description': None, 'homepage': None, 'labels': [], 'pulls': [],
                'branches': {'main': {'docs/agents/fleet.md': FLEET_SETTINGS}}}
            world['issues']['%s#7' % name] = {'number': 7, 'state': 'open'}
        self.world.write_text(json.dumps(world))

    def merge_brain(self, name):
        """The owner merges init's visible brain pull request on GitHub."""
        world = json.loads(self.world.read_text())
        repo = world['repos'][name.lower()]
        repo['branches']['main'].update(repo['branches']['openwiz/brain'])
        for pull in repo['pulls']:
            pull.update(state='closed', merged_at='2026-11-01T11:40:00Z')
        self.world.write_text(json.dumps(world))

    def github_writes(self):
        return [call for call in json.loads(self.world.read_text())['log'] if call[0] != 'GET']

    def env(self):
        env = {k: v for k, v in os.environ.items() if k in ('LANG', 'LC_ALL', 'TMPDIR')}
        if getattr(self, 'host', None):
            env['JOURNEY_HOST'] = self.host
        env.update(HOME=str(self.home), PATH='%s:%s:/usr/bin:/bin' % (self.shims, self.gh),
                   GH_TOKEN=PERSONAL, SYNTHETIC_SCENARIO=str(self.scenario),
                   SYNTHETIC_CALLS=str(self.calls), JOURNEY_FAKES=str(self.fakes),
                   FAKE_GH_WORLD=str(self.world), JOURNEY_ENGINE_LOG=str(self.engine_log))
        return env

    def run(self, argv, check=None):
        result = subprocess.run(argv, cwd=self.project, env=self.env(), capture_output=True,
                                text=True, timeout=120)
        self.outputs.append(result.stdout + result.stderr)
        if check is not None:
            self.case.assertEqual(result.returncode, check, (argv, result.stdout, result.stderr))
        return result

    def entry(self, *args, check=0):
        return self.run([sys.executable, '-I', str(ENTRY), *args], check)

    def build_tool_calls(self):
        return self.build_log.read_text().splitlines() if self.build_log.exists() else []


def documented(skill, marker, **values):
    """The one command the skill documents containing `marker`, as an argv, with
    its placeholders (<build>, <state>, <root>, <id>, cloud|local) filled in."""
    text = skill.read_text()
    found = [line.strip() for line in re.findall(r'^(.*)$', text, re.M) if marker in line and not line.lstrip().startswith(('#', '-', '>'))]
    found += re.findall(r'`([^`\n]*%s[^`\n]*)`' % re.escape(marker), text)
    commands = sorted({c for c in found if c.startswith(('python3 ', '<build>/'))})
    assert len(commands) == 1, (marker, commands)
    argv = shlex.split(commands[0])
    names = {'<build>': 'build', '$build': 'build', '<state>': 'state', '<root>': 'root', '<id>': 'operation'}
    out = []
    for word in argv:
        for token, key in names.items():
            if token in word:
                word = word.replace(token, str(values[key]))
        out.append('local' if word == 'cloud|local' else word)
    return [sys.executable if out[0] == 'python3' else out[0], *out[1:]]


def steps(skill):
    """The packaged skill's numbered sections, in order."""
    return [m for m in re.findall(r'^## [0-9]+\. (.+)$', skill.read_text(), re.M)]


def section(skill, title):
    text = skill.read_text()
    return text.split(f'. {title}\n', 1)[1].split('\n## ', 1)[0]


class InstallJourney(unittest.TestCase):

    def install(self, c, published, transport=None):
        """The skill's step 2, with the transport injected into the actual entry."""
        transport = transport or Transport(published)
        out = io.StringIO()
        code = entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                            fetch=transport, stdout=out)
        self.assertEqual(code, 0, out.getvalue())
        result = c.entry('install', '--assets', str(c.downloads), '--target', TARGET, '--root', str(c.root))
        return json.loads(result.stdout), transport

    def test_the_line_resolves_to_the_packaged_skill_and_its_confirmed_order(self):
        # #555: the plugin manifest is the installed WizSkills release's.
        plugin = json.loads((ROOT / '.wizskills/.claude-plugin/plugin.json').read_text())
        self.assertIn('./skills/install', plugin['skills'])
        skill = ROOT / 'skills/install/SKILL.md'
        self.assertIn('> ' + LINE + '\n', skill.read_text())
        self.assertIn('Install OpenWiz from', skill.read_text().split('\n---\n', 1)[0], 'its description names the line')
        self.assertEqual(steps(skill), ORDER)

    def test_the_packaged_skill_and_guide_offer_free_now_and_paid_plans_later(self):
        # #2293: OpenWiz is free; a CloudRoom purchase is upstream and separate, never an
        # OpenWiz upgrade; no price, trial or checkout; no referral, and the copy says why.
        for path in ('skills/install/SKILL.md', 'docs/install.md'):
            with self.subTest(path):
                text = ' '.join((ROOT / path).read_text().split())
                self.assertIn(FREE_ONLY, text)
                self.assertIn(NO_REFERRAL, text)
                self.assertEqual(NOT_OFFERED.findall(text), [])
                self.assertEqual(set(re.findall(r'cloudroom\.dev/[^\s`)]*', text)), {'cloudroom.dev/pricing'},
                                 "CloudRoom's own page, with no referral code or parameter")

    def test_from_the_line_to_a_first_fleet_on_a_cold_consumer(self):
        c = Consumer(self)
        published = release(self.dir_for(c))
        done = []

        def preflight():
            # The actual entry on this machine's real PATH and disk; the host is
            # the journey's supported Linux target, so any CI host replays it.
            out = io.StringIO()
            self.assertEqual(entry().main(['preflight'], host=('Linux', 'x86_64'), stdout=out), 0, out.getvalue())
            result = type('Result', (), {'stdout': out.getvalue()})
            report = json.loads(result.stdout)
            self.assertEqual(report['schema'], 'openwiz.preflight.v1')
            self.assertEqual(report['target'], TARGET)
            self.assertTrue(report['ready'])
            ids = [check['id'] for check in report['checks']]
            self.assertEqual(ids, ['python', 'target', 'disk', 'glibc', 'room-cli', 'gh', 'git'])
            self.assertNotIn('cargo', result.stdout)

        def verified_install():
            result, transport = self.install(c, published)
            self.assertEqual((result['version'], result['target'], result['activated']), (VERSION, TARGET, True))
            self.assertEqual(transport.urls, [BASE + f'latest/download/{NAME}.sha256',
                                              BASE + f'latest/download/{NAME}.manifest.json',
                                              BASE + f'download/v{VERSION}/{published["archive"]}',
                                              BASE + f'download/v{VERSION}/SHA256SUMS'])
            self.build = Path(result['build'])
            self.state = Path(result['state_root'])
            self.assertEqual(Path(os.readlink(c.root / 'current')), self.build)
            self.assertEqual((c.root / 'channel.json').read_text(),
                             '{"schema":"openwiz.update-channel.v1","channel":"packaged"}\n')
            status = json.loads(c.entry('status', '--root', str(c.root)).stdout)
            self.assertEqual((status['version'], status['channel'], status['build']), (VERSION, 'packaged', str(self.build)))

        def setup_and_connections():
            # The installed runtime's own setup tool; each component's result
            # code is the evidence the setup page turns into its states. The
            # repository-bound connections wait for the onboarding's Connect step.
            api = AppAPI(self)
            check = [sys.executable, '-I', '-B', str(self.build / 'runtime/setup/setup.py'), 'check',
                     '--state-dir', str(self.state), '--api-base', api.base]
            results = json.loads(c.run(check, 0).stdout)['results']
            self.assertEqual((results['cloudroom'], results['github_app'], results['project']),
                             ('cloudroom_login_verified', 'app_missing', 'project_unprepared'))
            self.assertEqual(api.requests, [], 'no App API call before an App is connected')

        def explain():
            installed = self.build / 'runtime/skills'
            said = section(installed / 'install/SKILL.md', 'Explain')
            quote = [line for line in said.splitlines() if line.startswith('> ')]
            for concept in ('**idea**', '**grill**', '**spec**', '**tickets**', '**builder**', '**reviewer**', '**merges**'):
                self.assertIn(concept, quote[0])

        def first_fleet():
            j = self.onboarding(c)
            results = j.replay()
            for turn, result in results:
                self.assertEqual(result.returncode, turn['exit'], (turn['tool'], result.stdout, result.stderr))
            carried = next(r for t, r in results if t['tool'][0] == 'continue').stdout
            skipped = any(t.get('setup') == 'personal_login' for t in self.onboard.JOURNEY['turns'])
            acting = ('synthetic-owner, your personal GitHub login (the App is skipped)' if skipped
                      else 'your OpenWiz GitHub App')
            for line in ('Acting identity: %s.' % acting,
                         'Private brain: %s (in repository).' % self.repository,
                         'Next: the grill (the explanation is done)'):
                self.assertIn(line, carried)
            self.assertEqual(len(j.spawns()), 1, 'one fleet thread starts the approved frontier')
            self.assertEqual([call['form'] for call in j.logged()], ['fleet'], 'the installed openwiz only reads progress')
            self.assertIn('5 tickets: 3 building, 2 blocked', results[-1][1].stdout)
            self.assertNotEqual(Path(self.onboard.HELPER).resolve().parents[3], ROOT.resolve())

        runners = {'Preflight': preflight, 'Verified install': verified_install,
                   'Setup and connections': setup_and_connections, 'Explain': explain, 'First fleet': first_fleet}
        skill = ROOT / 'skills/install/SKILL.md'
        self.assertEqual(steps(skill), ORDER)
        for title in steps(skill):
            runners[title]()
            done.append(title)
            if title == 'Verified install':
                self.assertEqual(steps(self.build / 'runtime/skills/install/SKILL.md'), ORDER,
                                 'the installed skill gives the same order')
        self.assertEqual(done, ORDER)
        self.assertEqual(c.build_tool_calls(), [], 'no Rust or Node tool was run')
        self.assertFalse(any(p.startswith('private/') or '/private/' in p for p in published['assets']))
        self.assertEqual(sorted(p.name for p in c.project.iterdir()), ['README.md', 'docs'],
                         'the consumer has no OpenWiz sources or skills checkout')
        self.assertFalse(c.engine_log.exists(), 'no engine or tick call')
        for output in c.outputs:
            self.assertNotIn(TOKEN, output)
            self.assertNotIn(PERSONAL, output)

    # Helpers for the journey's state.
    def dir_for(self, c):
        return c.dir / 'published'

    def connect_app(self):
        """The App connection T3's flow saves: a disposable key and the App's id."""
        folder = self.state / 'service/app'
        folder.mkdir(parents=True, mode=0o700, exist_ok=True)
        (self.state / 'service').chmod(0o700)
        subprocess.run(['/usr/bin/openssl', 'genpkey', '-algorithm', 'RSA', '-pkeyopt', 'rsa_keygen_bits:2048',
                        '-out', str(folder / 'private-key.pem')], check=True, capture_output=True)
        (folder / 'private-key.pem').chmod(0o600)
        (folder / 'app.json').write_text('{"app_id":1001}')
        (folder / 'app.json').chmod(0o600)

    def test_a_tampered_release_is_refused_and_the_last_good_build_stays(self):
        c = Consumer(self)
        good = release(c.dir / 'good')
        first, _ = self.install(c, good)
        bad = release(c.dir / 'bad', source='2' * 40, tamper=True)
        shutil.rmtree(c.downloads)
        out = io.StringIO()
        self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                      fetch=Transport(bad), stdout=out), 5, out.getvalue())
        self.assertIn('archive checksum mismatch', json.loads(out.getvalue())['reason'])
        self.assertFalse(c.downloads.exists())
        self.assertEqual(os.readlink(c.root / 'current'), first['build'], 'the last good build stays selected')
        self.assertFalse((c.root / 'state/service/builds' / ('2' * 40)).exists())

    def test_a_repeated_install_reuses_the_verified_build(self):
        c = Consumer(self)
        published = release(c.dir / 'published')
        first, _ = self.install(c, published)
        again = json.loads(c.entry('install', '--assets', str(c.downloads), '--target', TARGET,
                                   '--root', str(c.root)).stdout)
        self.assertEqual(again['build'], first['build'])
        self.assertEqual(os.readlink(c.root / 'current'), first['build'])

    def test_an_interrupted_download_installs_nothing_until_it_completes(self):
        c = Consumer(self)
        published = release(c.dir / 'published')
        cut = dict(published, files={k: v for k, v in published['files'].items() if k != published['archive']})
        out = io.StringIO()
        self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                      fetch=Transport(cut), stdout=out), 5)
        self.assertEqual(json.loads(out.getvalue())['outcome'], 'refused')
        c.entry('install', '--assets', str(c.downloads), '--target', TARGET, '--root', str(c.root), check=5)
        self.assertFalse((c.root / 'current').exists())
        shutil.rmtree(c.downloads, ignore_errors=True)
        result, _ = self.install(c, published)
        self.assertTrue(result['activated'])

    def test_a_published_checksum_that_does_not_match_downloads_nothing(self):
        c = Consumer(self)
        published = release(c.dir / 'published')
        files = dict(published['files'])
        files[NAME + '.sha256'] = ('0' * 64 + f'  {NAME}.manifest.json\n').encode()
        out = io.StringIO()
        self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                      fetch=Transport(dict(published, files=files)), stdout=out), 5)
        self.assertFalse(c.downloads.exists() and any(c.downloads.iterdir()))

    def test_a_manifest_naming_any_other_archive_path_downloads_nothing(self):
        c = Consumer(self)
        published = release(c.dir / 'published')
        for name in ('../escape.tar.gz', 'nested/openwiz-0.0.99-x86_64-unknown-linux-gnu.tar.gz', 'archive.zip'):
            manifest = json.loads(published['files'][NAME + '.manifest.json'])
            manifest['archive'] = name
            raw = json.dumps(manifest).encode()
            files = {NAME + '.sha256': f'{sha(raw)}  {NAME}.manifest.json\n'.encode(),
                     NAME + '.manifest.json': raw, name: published['files'][published['archive']]}
            transport = Transport(dict(published, files=files))
            out = io.StringIO()
            self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                          fetch=transport, stdout=out), 5, (name, out.getvalue()))
            self.assertEqual(len(transport.urls), 2, 'the archive is never requested')
            self.assertFalse(c.downloads.exists())

    def test_a_fresh_home_installs_without_any_precreated_folder(self):
        # R7: a new user's HOME has no ~/.local/lib; the documented install still completes.
        c = Consumer(self)
        self.assertFalse((c.home / '.local').exists())
        result, _ = self.install(c, release(c.dir / 'published'))
        self.assertTrue(result['activated'])
        for folder in (c.home / '.local', c.home / '.local/lib', c.root):
            self.assertEqual(folder.stat().st_mode & 0o777, 0o700, folder)

    def test_the_install_parent_is_never_made_outside_home_or_through_a_link(self):
        c = Consumer(self)
        published = release(c.dir / 'published')
        out = io.StringIO()
        self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                      fetch=Transport(published), stdout=out), 0, out.getvalue())
        outside = c.dir / 'elsewhere/lib/openwiz'
        c.entry('install', '--assets', str(c.downloads), '--target', TARGET, '--root', str(outside), check=5)
        self.assertFalse((c.dir / 'elsewhere').exists(), 'nothing is made outside HOME')
        (c.dir / 'target').mkdir()
        os.symlink(c.dir / 'target', c.home / '.local')
        c.entry('install', '--assets', str(c.downloads), '--target', TARGET, '--root', str(c.root), check=5)
        self.assertEqual(list((c.dir / 'target').iterdir()), [], 'nothing is made through a link')

    def installed(self, c):
        result, _ = self.install(c, release(c.dir / 'published'))
        return Path(result['build']), Path(result['state_root'])

    # The onboarding's Connect step (#636), on the installed tools, for one repository.
    def tool(self, c, name, *args, check=0):
        program = [sys.executable, '-I', '-B', str(self.build / 'runtime/setup' / name)]
        return c.run([*program, *args], check)

    def confirm(self, c, name, *args):
        """A tool's preview, then the same with --yes and its operation id: the owner's yes."""
        proposed = json.loads(self.tool(c, name, *args).stdout)
        self.assertEqual(proposed['phase'], 'proposed', proposed)
        applied = json.loads(self.tool(c, name, *args, '--yes', '--operation', proposed['operation_id']).stdout)
        self.assertEqual(applied['phase'], 'applied', applied)
        return applied

    def init(self, c, *args, check=0):
        return c.run([str(self.build / 'openwiz'), 'init', '--repository', self.repository,
                      '--state-dir', str(self.state), '--json', *args], check)

    def connect(self, c, identity='skip', api=None):
        """Sign-in read-back, prepare, the App or the explicit skip, init and the check:
        each disclosed effect previewed, then applied on its yes. Returns the check results."""
        state = ['--state-dir', str(self.state)]
        c.github(self.repository)
        login = json.loads(c.run([sys.executable, '-I', '-B', str(self.build / 'runtime/setup/github_login.py'),
                                  'status', '--repository', self.repository, *state], 0).stdout)
        self.assertIn('synthetic-owner', json.dumps(login), 'the existing login is read back and reused')
        self.confirm(c, 'setup.py', 'prepare', *state, '--repository', self.repository, '--project', 'prj_synthetic',
                     '--operator', 'synthetic-owner', '--service-issue', '7', '--answer-thread', 'thr_synthetic_answer',
                     *[x for k in ('harness', 'model', 'effort') for x in ('--' + k, self.settings[k])])
        if identity == 'skip':
            self.confirm(c, 'github_login.py', 'skip-app', *state)
        else:
            self.connect_app()
        preview = json.loads(self.init(c).stdout)
        self.assertEqual((preview['outcome'], preview['brain']['location']), ('proposed', 'in_repository'))
        applied = json.loads(self.init(c, '--yes', '--operation', preview['operation_id']).stdout)
        self.assertEqual(applied['outcome'], 'applied', applied)
        check = ['check', *state] + (['--api-base', api.base] if api else [])
        return check

    def onboarding(self, c, identity=None, api=None):
        """The onboarding journey through the installed helper, its Connect step on the installed tools."""
        onboard = module('onboard_journey', ONBOARD_TESTS / 'test_journey.py')
        self.repository = onboard.JOURNEY['repo']  # the conversation's one repository
        said = onboard.answers(onboard.JOURNEY['turns'])
        self.settings = {k: said[k] for k in ('harness', 'model', 'effort')}  # answered once, at Connect
        installed = self.build / 'runtime/skills/install'  # onboard merged into install (#1187)
        onboard.SKILL = installed / 'SKILL.md'
        onboard.HELPER = installed / 'scripts/first_fleet.py'
        self.onboard = onboard
        j = onboard.Journey(self)
        j.openwiz = self.build / 'openwiz'
        j.setup_dir = self.state
        env = j.env
        j.env = lambda: dict(env(), PATH='%s:%s' % (c.shims, env()['PATH']), JOURNEY_FAKES=str(c.fakes))

        def write_setup(recorded):
            # The real installed setup, not a synthetic state: the conversation
            # names the identity (its App or the explicit skip); the brain's pull
            # request is open until the owner merges it, so nothing carries yet.
            chosen = identity or ('skip' if recorded['identity']['selected'] == 'personal_login' else 'app')
            check = self.connect(c, chosen, api or AppAPI(self, self.repository))
            results = json.loads(self.tool(c, 'setup.py', *check).stdout)['results']
            self.assertEqual(results['project'], 'project_settings_missing', 'an unmerged brain is not ready')
            early = j.helper(['continue', '--explained'])
            self.assertEqual(early.returncode, 5, early.stdout)
            self.assertIn('Project: not verified (project_settings_missing)', early.stdout)
            c.merge_brain(self.repository)
            results = json.loads(self.tool(c, 'setup.py', *check).stdout)['results']
            self.assertEqual((results['personal_github'], results['project']), ('identity_verified', 'project_verified'))
            self.assertEqual(results['github_app'], 'app_missing' if chosen == 'skip' else 'app_verified')
        j.write_setup = write_setup
        return j

    def test_both_identities_continue_through_the_installed_tools(self):
        # #636 (P2, P4 at the installed seam): the explicit skip and the user's
        # own App each carry the same repository, project, brain and agents into
        # onboarding. Catches an App failure read as a skip, a skip read as a
        # verified App, or a brain that is not the one init made.
        for identity, acting in (('skip', 'synthetic-owner, your personal GitHub login (the App is skipped)'),
                                 ('app', 'your OpenWiz GitHub App')):
            with self.subTest(identity):
                c = Consumer(self)
                self.build, self.state = self.installed(c)
                api = AppAPI(self, 'synthetic-owner/synthetic-sheep')
                j = self.onboarding(c, identity, api)
                results = j.replay(until='grill')
                carried = next(r for t, r in results if t['tool'][0] == 'continue')
                self.assertEqual(carried.returncode, 0, carried.stdout)
                self.assertIn('Acting identity: %s.' % acting, carried.stdout)
                self.assertIn('Private brain: %s (in repository).' % self.repository, carried.stdout)
                saved = json.loads((self.state / 'setup/state.json').read_text())
                self.assertEqual(saved['identity']['app_choice'] if 'identity' in saved else 'none',
                                 'skip' if identity == 'skip' else 'none', 'only the owner\'s skip selects the login')
                self.assertEqual(any(r[0] == 'POST' for r in api.requests), identity == 'app',
                                 'the App path proves access with an installation token; the skip mints none')

    def test_a_resumed_connect_repeats_no_effect(self):
        # #636 (P5): an interrupted Connect run again reuses each operation and
        # reads before it writes. Catches a second label, branch, file or pull
        # request, or a changed onboarding record, from a repeated yes.
        c = Consumer(self)
        self.build, self.state = self.installed(c)
        j = self.onboarding(c)
        j.replay(until='grill')
        writes = c.github_writes()
        brain = 'repos/%s/' % self.repository
        # #1361: init's own label and the five records v1 labels, each created once.
        self.assertEqual(writes, [['POST', brain + 'labels']] * 6 + [['POST', brain + 'git/refs']]
                         + [['PUT', brain + 'contents/.openwiz/brain/' + f]
                            for f in ('README.md', 'settings.json', 'grills/README.md', 'decisions/README.md')]
                         + [['POST', brain + 'pulls']], 'six labels, one branch, four files and one pull request')
        before = (j.state.read_text(), (self.state / 'setup/state.json').read_text())
        init_ops = [o for o in json.loads(before[1])['operations'] if 'brain' in ' '.join(o['effects']).lower()]
        self.assertTrue(init_ops, 'init saved its operation')
        again = json.loads(self.init(c, '--yes', '--operation', init_ops[-1]['operation_id']).stdout)
        self.assertEqual(again['outcome'], 'applied', again)
        self.assertEqual(c.github_writes(), writes, 'a repeated init makes no second provider effect')
        repeat = j.helper(['continue', '--explained'])
        self.assertEqual(repeat.returncode, 0, repeat.stdout)
        self.assertEqual(json.loads(j.state.read_text()), json.loads(before[0]), 'the carried record is unchanged')

    def test_status_refuses_a_selection_it_cannot_verify(self):
        # R6: the selected build must be this state's, named by its own verified source.
        for case in ('foreign', 'manifest', 'source', 'asset'):
            c = Consumer(self)
            build, state = self.installed(c)
            if case == 'foreign':
                other = state / 'service/builds/not-a-commit'
                shutil.copytree(build, other, symlinks=True)
                os.unlink(c.root / 'current')
                os.symlink(other, c.root / 'current')
            elif case == 'manifest':
                manifest = json.loads((build / 'install-manifest.json').read_text())
                manifest['assets'] = manifest['assets'][:3]
                (build / 'install-manifest.json').write_text(json.dumps(manifest))
            elif case == 'source':
                other = state / 'service/builds' / ('2' * 40)
                shutil.copytree(build, other, symlinks=True)
                os.unlink(c.root / 'current')
                os.symlink(other, c.root / 'current')
            else:
                (build / 'runtime/setup/setup.py').write_text('# changed\n')
            install = entry()
            out = io.StringIO()
            self.assertEqual(install.main(['status', '--root', str(c.root)], stdout=out), 5, (case, out.getvalue()))

    def test_status_refuses_a_symlinked_ancestor(self):
        # R6 (re-check): a valid build reached through a redirected ancestor of the
        # root, an explicit state, service/ or builds/ is refused; the paths read
        # the same, but one folder on the way is now a link to a moved copy.
        def redirect(folder):
            moved = folder.with_name(folder.name + '-moved')
            os.rename(folder, moved)
            os.symlink(moved, folder)

        for case in ('control', 'custom-control', 'root-ancestor', 'custom-state-ancestor', 'service', 'builds'):
            with self.subTest(case=case):
                c = Consumer(self)
                custom = case.startswith('custom')
                state_arg = []
                if custom:
                    (c.home / 'custom').mkdir(mode=0o700)
                    state_arg = ['--state-root', str(c.home / 'custom/state')]
                published = release(c.dir / 'published')
                out = io.StringIO()
                self.assertEqual(entry().main(['download', '--target', TARGET, '--dir', str(c.downloads)],
                                              fetch=Transport(published), stdout=out), 0, out.getvalue())
                result = json.loads(c.entry('install', '--assets', str(c.downloads), '--target', TARGET,
                                            '--root', str(c.root), *state_arg).stdout)
                state = Path(result['state_root'])
                if case == 'root-ancestor':
                    redirect(c.home / '.local/lib')
                elif case == 'custom-state-ancestor':
                    redirect(c.home / 'custom')
                elif case == 'service':
                    redirect(state / 'service')
                elif case == 'builds':
                    redirect(state / 'service/builds')
                install = entry()
                want = 0 if case.endswith('control') else 5
                out = io.StringIO()
                self.assertEqual(install.main(['status', '--root', str(c.root), *state_arg], stdout=out), want,
                                 (case, out.getvalue()))

    def test_preflight_names_each_supported_target_and_refuses_the_rest(self):
        install = entry()
        for host, target in ((('Darwin', 'arm64'), 'aarch64-apple-darwin'),
                             (('Darwin', 'x86_64'), 'x86_64-apple-darwin'),
                             (('Linux', 'x86_64'), 'x86_64-unknown-linux-gnu'),
                             (('Linux', 'aarch64'), None), (('Windows', 'AMD64'), None)):
            out = io.StringIO()
            code = install.main(['preflight'], host=host, stdout=out)
            report = json.loads(out.getvalue())
            self.assertEqual(report['target'], target, host)
            if target is None:
                self.assertEqual(code, 5)
                check = next(item for item in report['checks'] if item['id'] == 'target')
                self.assertEqual(check['state'], 'refused')
                self.assertIn('macOS', check['next'])


if __name__ == '__main__':
    unittest.main()
