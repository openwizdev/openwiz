#!/usr/bin/env python3
import argparse
import http.client
import json
from pathlib import Path
import re
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from install import Refused, TARGETS, install, manifest_at, private, require, sha, unique

DESCRIPTION = """Fixed published-release transport. No source checkout, build, push or shell."""

BASE = 'https://github.com/openwizdev/OpenWiz/releases/'
FEED = 'https://github.com/openwizdev/OpenWiz/releases.atom'
TAG_LINK = re.compile(r'href="https://github\.com/openwizdev/OpenWiz/releases/tag/v(0\.[0-9]+\.[0-9]+)"')


class NotFound(Refused):
    pass


class Redirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, url):
        parsed = urllib.parse.urlsplit(url)
        require(parsed.scheme == 'https' and parsed.hostname in
                ('github.com', 'release-assets.githubusercontent.com', 'objects.githubusercontent.com')
                and not parsed.username and not parsed.password, 'unapproved artifact redirect')
        return super().redirect_request(req, fp, code, msg, headers, url)


def fetch_bytes(url, limit):
    require(url.startswith(BASE) or url == FEED, 'unapproved release endpoint')
    opener = urllib.request.build_opener(Redirects())
    try:
        with opener.open(urllib.request.Request(url, headers={'User-Agent': 'OpenWiz-package-v1'}), timeout=60) as response:
            data = response.read(limit + 1)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            raise NotFound('release asset 404 at ' + url)
        raise Refused('release asset HTTP %d at %s' % (error.code, url))
    except http.client.HTTPException:
        raise Refused('interrupted transfer at ' + url)
    except OSError as error:
        raise Refused('network error at %s: %s' % (url, getattr(error, 'reason', None) or error))
    require(len(data) <= limit, 'artifact exceeds its declared byte bound at ' + url)
    return data


def load(raw, checksum, target, selected=False):
    with tempfile.TemporaryDirectory(prefix='openwiz-package-manifest-') as temporary:
        file = Path(temporary) / 'manifest.json'
        file.write_bytes(raw)
        return manifest_at(file, checksum, target, selected)[0]


def published(target, fetch=fetch_bytes, version=None):
    require(target in TARGETS, 'unsupported package target')
    if version is None:
        try:
            return manifest_from(BASE + 'latest/download/', target, fetch)
        except NotFound as missing:
            tags = TAG_LINK.findall(fetch(FEED, 1024 * 1024).decode('utf-8', 'replace'))
            require(tags, '%s; no published release tag found at %s' % (missing, FEED))
            version = max(tags, key=lambda tag: tuple(map(int, tag.split('.'))))
    require(re.fullmatch(r'0\.[0-9]+\.[0-9]+', version), 'invalid release version')
    found = manifest_from(BASE + 'download/v' + version + '/', target, fetch)
    require(found[2]['version'] == version, 'published version does not match its tag v' + version)
    return found


def manifest_from(prefix, target, fetch):
    name = 'openwiz-' + target
    sums = fetch(prefix + name + '.sha256', 256)
    match = re.fullmatch(rb'([0-9a-f]{64})  ' + re.escape((name + '.manifest.json').encode()) + rb'\n', sums)
    require(match, 'invalid published manifest checksum')
    checksum = match[1].decode('ascii')
    raw = fetch(prefix + name + '.manifest.json', 4 * 1024 * 1024)
    return sums, raw, load(raw, checksum, target), checksum


def discover(target, fetch=fetch_bytes):
    manifest, checksum = published(target, fetch)[2:]
    return {'schema': 'openwiz.package-candidate.v1', 'version': manifest['version'],
            'source': manifest['source'], 'target': target, 'manifest_sha256': checksum}


def stage(target, version, source, checksum, root, state_root, fetch=fetch_bytes):
    require(target in TARGETS and re.fullmatch(r'0\.[0-9]+\.[0-9]+', version), 'invalid version/target')
    root, state_root = Path(root), Path(state_root)
    private(root)
    private(state_root / 'service')
    current = root / 'current'
    require(current.is_symlink(), 'missing packaged current selection')
    selected = current.readlink()
    require(selected.is_absolute() and selected.parent == state_root / 'service/builds', 'foreign selection')
    private(selected)
    private(selected / 'install-manifest.json', False)
    old_raw = (selected / 'install-manifest.json').read_bytes()
    old = load(old_raw, sha(old_raw), target, selected=True)
    require(tuple(map(int, version.split('.'))) > tuple(map(int, old['version'].split('.'))), 'published version must advance')
    name = f'openwiz-{version}-{target}.manifest.json'
    prefix = BASE + 'download/v' + version + '/'
    raw = fetch(prefix + name, 4 * 1024 * 1024)
    manifest = load(raw, checksum, target)
    require(manifest['version'] == version and manifest['source'] == source, 'published identity changed')
    with tempfile.TemporaryDirectory(prefix='.package-download-', dir=state_root / 'service') as temporary:
        directory = Path(temporary)
        manifest_file = directory / 'manifest.json'
        archive = directory / 'bundle.tar.gz'
        manifest_file.write_bytes(raw)
        manifest_file.chmod(0o600)
        archive.write_bytes(fetch(prefix + manifest['archive'], manifest['archive_bytes']))
        archive.chmod(0o600)
        return install(manifest_file, archive, checksum, root, target, state_root=state_root, stage_only=True)


def main():
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    commands = parser.add_subparsers(dest='action', required=True)
    discover_parser = commands.add_parser('discover')
    discover_parser.add_argument('target', choices=TARGETS)
    stage_parser = commands.add_parser('stage')
    for key in ('target', 'version', 'source', 'checksum', 'root', 'state_root'):
        stage_parser.add_argument(key)
    args = vars(parser.parse_args())
    action = args.pop('action')
    try:
        value = discover(**args) if action == 'discover' else stage(**args)
        print(json.dumps(value, separators=(',', ':')))
        return 0
    except (Refused, OSError, ValueError, TypeError, KeyError):
        print('package pull: refused; last-good build retained', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
