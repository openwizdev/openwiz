#!/usr/bin/env python3
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tarfile
import tempfile

DESCRIPTION = """Verified, offline package installation. Python 3.9+; no Cargo, npm or network.

Transport supplies a manifest, its separately checked checksum, and an archive.
Checksums establish integrity, not a signed publisher identity. No program from
an archive is run while validating or installing it.
"""

TARGETS = ('aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu')
PROGRAMS = {'openwiz'}
REQUIRED = PROGRAMS | {'runtime/openwiz-build.txt', 'runtime/install.py', 'runtime/pull.py',
    'runtime/skills/setup/SKILL.md', 'runtime/skills/grill/SKILL.md',
    'runtime/skills/to-tickets/SKILL.md', 'runtime/skills/launch-fleet/SKILL.md',
    'runtime/skills/launch-fleet/scripts/fleet.sh',
    'runtime/setup/setup.py', 'runtime/setup/store.py', 'runtime/setup/github_login.py',
    'runtime/github-app/setup.py',
    'runtime/github-app/app.py', 'runtime/github-app/manifest.json',
    'runtime/cloudroom/cloudroom.py',
    'runtime/setup/init.py'}
LEGACY_PROGRAMS = PROGRAMS | {'openwiz-service', 'openwiz-server'}
LEGACY_REQUIRED = REQUIRED | LEGACY_PROGRAMS | {'web/openwiz-build.txt', 'web/index.html', 'web/setup.html'}
PREVIOUS_SKILLS = {'runtime/skills/to-tickets/SKILL.md': 'runtime/skills/plan-graph/SKILL.md'}


def previous(required):
    return {PREVIOUS_SKILLS.get(path, path) for path in required}


class Refused(Exception):
    pass


def require(value, reason):
    if not value:
        raise Refused(reason)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate manifest key')
        result[key] = value
    return result


def shape(value, keys):
    require(type(value) is dict and set(value) == set(keys.split()), 'unexpected manifest shape')


def digest(value, length=64):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{%d}' % length, value) is not None


def safe(path, legacy=False):
    require(isinstance(path, str) and re.fullmatch(r'[A-Za-z0-9_+./-]+', path), 'unsafe asset path')
    require(not path.startswith('/') and all(p not in ('', '.', '..') and not p.startswith('.')
            for p in path.split('/')), 'unsafe asset path')
    require(path in (LEGACY_PROGRAMS if legacy else PROGRAMS)
            or path.startswith(('runtime/', 'web/') if legacy else 'runtime/'), 'unknown asset role')
    return path


def manifest_at(path, checksum, target, selected=False):
    data = path.read_bytes()
    require(len(data) <= 4 * 1024 * 1024 and sha(data) == checksum, 'manifest checksum mismatch')
    value = json.loads(data, object_pairs_hook=unique)
    shape(value, 'schema version source target setup_api archive archive_sha256 archive_bytes assets')
    require(value['schema'] == 'openwiz.install.v1', 'unsupported manifest schema')
    require(isinstance(value['version'], str) and re.fullmatch(r'0\.[0-9]+\.[0-9]+', value['version']), 'invalid version')
    require(digest(value['source'], 40), 'invalid source identity')
    require(target in TARGETS and value['target'] == target, 'unsupported or mismatched target')
    require(type(value['setup_api']) is int and value['setup_api'] == 1, 'incompatible setup API')
    require(isinstance(value['archive'], str) and re.fullmatch(r'[A-Za-z0-9_.-]+\.tar\.gz', value['archive']), 'invalid archive name')
    require(digest(value['archive_sha256']), 'invalid archive hash')
    require(type(value['archive_bytes']) is int and value['archive_bytes'] > 0, 'invalid archive size')
    require(type(value['assets']) is list and value['assets'], 'empty asset list')
    legacy = selected and any(type(item) is dict and (item.get('path') in LEGACY_PROGRAMS - PROGRAMS
                              or str(item.get('path')).startswith('web/')) for item in value['assets'])
    programs = LEGACY_PROGRAMS if legacy else PROGRAMS
    assets = {}
    for item in value['assets']:
        shape(item, 'path sha256 bytes mode')
        path = safe(item['path'], legacy)
        require(path not in assets and not any(path.startswith(p + '/') or p.startswith(path + '/') for p in assets),
                'duplicate or overlapping asset')
        require(digest(item['sha256']) and type(item['bytes']) is int and item['bytes'] >= 0, 'invalid asset identity')
        require(item['mode'] == ('0700' if path in programs or path.endswith('.sh')
                                 or re.fullmatch(r'runtime/adapters/[a-z0-9-]+/driver', path) else '0600'),
                'invalid asset mode')
        assets[path] = item
    required = LEGACY_REQUIRED if legacy else REQUIRED
    layouts = (required, previous(required)) if selected else (required,)
    require(any(layout <= assets.keys() for layout in layouts), 'incomplete program or skill runtime')
    identity = f"openwiz.package.v1\n{value['version']}\n{value['source']}\n{target}\n".encode()
    for stamp in ('runtime/openwiz-build.txt', 'web/openwiz-build.txt') if legacy else ('runtime/openwiz-build.txt',):
        require(assets[stamp]['sha256'] == sha(identity) and assets[stamp]['bytes'] == len(identity),
                'runtime build identity mismatch')
    return value, assets, data


def private(path, directory=True):
    meta = path.lstat()
    require((stat.S_ISDIR(meta.st_mode) if directory else stat.S_ISREG(meta.st_mode))
            and meta.st_uid == os.getuid() and meta.st_mode & 0o077 == 0
            and (directory or meta.st_nlink == 1), 'path must be private, owned and have the expected type')


def path_guard(path):
    require(path.is_absolute() and '..' not in path.parts and '.' not in path.parts, 'absolute plain path required')
    for parent in (path, *path.parents):
        if parent.exists() or parent.is_symlink():
            require(not parent.is_symlink() and parent.is_dir(), 'symlink or non-directory ancestor')


def directory(path):
    if not path.exists() and not path.is_symlink():
        path.mkdir(mode=0o700)
    private(path)


def sync(path):
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def write(path, data, mode=0o600):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(data)
            stream.flush()
            os.fsync(stream.fileno())
    except BaseException:
        path.unlink(missing_ok=True)
        raise


def verify_installed(build, assets, raw):
    private(build)
    require((build / 'install-manifest.json').read_bytes() == raw, 'existing source has a different manifest')
    actual = set()
    for parent, dirs, files in os.walk(build, followlinks=False):
        private(Path(parent))
        for name in dirs:
            private(Path(parent) / name)
        for name in files:
            file = Path(parent) / name
            private(file, False)
            relative = file.relative_to(build).as_posix()
            if relative != 'install-manifest.json':
                actual.add(relative)
                require(relative in assets and sha(file.read_bytes()) == assets[relative]['sha256']
                        and file.stat().st_size == assets[relative]['bytes']
                        and stat.S_IMODE(file.stat().st_mode) == int(assets[relative]['mode'], 8), 'installed asset mismatch')
    require(actual == set(assets), 'installed asset set mismatch')


def damaged(build, assets, raw):
    try:
        verify_installed(build, assets, raw)
        return False
    except (Refused, OSError):
        meta = build.lstat()
        require(stat.S_ISDIR(meta.st_mode) and meta.st_uid == os.getuid(), 'a foreign build is in the way')
        os.chmod(build, 0o700)
        stored = build / 'install-manifest.json'
        try:
            require(not (stored.exists() or stored.is_symlink())
                    or (stored.is_file() and not stored.is_symlink() and stored.read_bytes() == raw),
                    'existing source has a different manifest')
        except BaseException:
            os.chmod(build, stat.S_IMODE(meta.st_mode))
            raise
        return True


def repair_backups(builds, source, raw):
    found = sorted(builds.glob('.damaged-*-' + source))
    for backup in found:
        meta = backup.lstat()
        require(stat.S_ISDIR(meta.st_mode) and meta.st_uid == os.getuid(), 'a foreign repair backup is in the way')
        stored = backup / 'install-manifest.json'
        require(stored.is_file() and not stored.is_symlink() and stored.read_bytes() == raw,
                'existing source has a different manifest in its repair backup ' + backup.name)
    return found


def activate_link(build, root):
    temporary = root / ('.current-' + str(os.getpid()))
    os.symlink(build, temporary)
    try:
        os.replace(temporary, root / 'current')
        sync(root)
    finally:
        temporary.unlink(missing_ok=True)


def install(manifest, archive, manifest_sha256, root, target, state_root=None,
            free_bytes=lambda path: shutil.disk_usage(path).free, activate=activate_link, stage_only=False):
    manifest, archive, root = Path(manifest), Path(archive), Path(root)
    value, assets, raw = manifest_at(manifest, manifest_sha256, target)
    require(archive.stat().st_size == value['archive_bytes'], 'archive size mismatch')
    hasher = hashlib.sha256()
    with archive.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            hasher.update(chunk)
    require(hasher.hexdigest() == value['archive_sha256'], 'archive checksum mismatch')
    path_guard(root)
    require(root.parent.is_dir(), 'installation parent must exist')
    directory(root)
    lock_path = root / 'install.lock'
    if lock_path.exists() or lock_path.is_symlink():
        private(lock_path, False)
    with lock_path.open('a+b') as lock:
        os.chmod(lock_path, 0o600)
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        private(root)
        state_root = Path(state_root) if state_root else root / 'state'
        path_guard(state_root)
        directory(state_root)
        directory(state_root / 'service')
        builds = state_root / 'service/builds'
        directory(builds)
        link = root / 'current'
        if link.exists() or link.is_symlink():
            require(link.is_symlink() and link.lstat().st_uid == os.getuid(), 'foreign current selection')
            selected = Path(os.readlink(link))
            require(selected.is_absolute() and selected.parent == builds and digest(selected.name, 40), 'foreign current selection')
            if selected != builds / value['source']:
                private(selected)
        channel = root / 'channel.json'
        require(channel.exists() or not link.is_symlink(), 'source installation requires an explicit channel migration')
        channel_bytes = b'{"schema":"openwiz.update-channel.v1","channel":"packaged"}\n'
        if channel.exists() or channel.is_symlink():
            private(channel, False)
            require(channel.read_bytes() == channel_bytes, 'existing update channel requires explicit migration')
        build = builds / value['source']
        with tarfile.open(archive, 'r:gz') as tar:
            members = []
            seen = set()
            for member in tar:
                require(len(members) < len(assets), 'archive asset count mismatch')
                members.append(member)
                path = safe(member.name)
                require(member.isfile() and not member.pax_headers and path not in seen and path in assets,
                        'unsafe, duplicate or unlisted archive member')
                item = assets[path]
                require(member.size == item['bytes'] and member.mode == int(item['mode'], 8), 'archive asset metadata mismatch')
                seen.add(path)
            require(seen == set(assets), 'incomplete archive')
            repair = (build.exists() or build.is_symlink()) and damaged(build, assets, raw)
            if repair or not (build.exists() or build.is_symlink()):
                backups = repair_backups(builds, value['source'], raw)
                needed = sum(item['bytes'] for item in assets.values()) + len(raw)
                require(free_bytes(builds) >= needed, 'insufficient disk space')
                temporary = Path(tempfile.mkdtemp(prefix='.stage-', dir=builds))
                try:
                    for member in members:
                        item = assets[member.name]
                        stream = tar.extractfile(member)
                        hasher = hashlib.sha256()
                        destination = temporary / member.name
                        for parent in reversed(destination.parent.relative_to(temporary).parents):
                            directory(temporary / parent)
                        directory(destination.parent)
                        with destination.open('xb') as out:
                            os.chmod(destination, int(item['mode'], 8))
                            for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                                hasher.update(chunk)
                                out.write(chunk)
                            out.flush()
                            os.fsync(out.fileno())
                        require(hasher.hexdigest() == item['sha256'], 'asset checksum mismatch')
                    write(temporary / 'install-manifest.json', raw)
                    verify_installed(temporary, assets, raw)
                    for parent, _, _ in os.walk(temporary, topdown=False):
                        sync(Path(parent))
                    if repair:
                        aside = builds / ('.damaged-%d-%s' % (os.getpid(), value['source']))
                        os.rename(build, aside)
                        try:
                            os.rename(temporary, build)
                        except BaseException:
                            os.rename(aside, build)
                            raise
                        backups.append(aside)
                    else:
                        os.rename(temporary, build)
                    sync(builds)
                    verify_installed(build, assets, raw)
                    for backup in backups:
                        shutil.rmtree(backup, ignore_errors=True)
                finally:
                    if temporary.exists():
                        shutil.rmtree(temporary)
        if not stage_only:
            if not channel.exists():
                write(channel, channel_bytes)
                sync(root)
            activate(build, root)
        return {'schema': 'openwiz.install-result.v1', 'version': value['version'], 'source': value['source'],
                'target': target, 'build': str(build), 'state_root': str(state_root),
                'activated': not stage_only, 'repaired': repair}


def main():
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    for flag in ('manifest', 'archive', 'manifest-sha256', 'root', 'target'):
        parser.add_argument('--' + flag, required=True)
    parser.add_argument('--state-root')
    parser.add_argument('--stage-only', action='store_true')
    args = vars(parser.parse_args())
    try:
        result = install(**args)
        print(json.dumps(result, separators=(',', ':')), flush=True)
        return 0
    except (Refused, OSError, ValueError, KeyError, TypeError, tarfile.TarError):
        print('openwiz-install: refused; last-good selection and owner settings retained', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
