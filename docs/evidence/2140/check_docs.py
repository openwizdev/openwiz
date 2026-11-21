#!/usr/bin/env python3
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

FILES = ('README.md', 'docs/install.md')
STALE = re.compile(r'(?i:coming soon)|0\.0\.12|7370|install open\b|(?i:setup page)|before 0\.1\.0|Apple silicon (?:and|or) Intel|install the CloudRoom app yourself')
WHOLE_ROOT = re.compile(r'(?i)rm -r\w*\s+(?:<root>|~/\.local/lib/openwiz)/?(?=[\s`]|$)'
                        r'|remove the install\b|delet\w*\s+(?:the install folder|(?:the )?`(?:<root>|~/\.local/lib/openwiz)/?`)')
WHOLE_SERVICE = re.compile(r'(?i)delet\w*\s+(?:only\s+|its\s+|all of\s+)?`(?:<state>/|state/)?service/?`'
                           r'|rm -r\w*\s+\S*service/?(?=[\s`]|$)')
WHOLE_STATE = re.compile(r'(?i)delet\w*\s+(?:only\s+)?`(?:<state>|<root>/state|state)/?`'
                         r'|rm -r\w*\s+\S*(?:<state>|state)/?(?=[\s`]|$)|delete it only if')
STEP = re.compile(r'^(\d+)\. \*\*([^*]+)\*\*', re.M)
GUIDE_NEEDS = ('continue the OpenWiz install', 'identity_unreadable', "'^version: 0.2.2$'", '.openwiz-skill')
BARE_CONTINUE = re.compile(r'say "continue"')
LINK = re.compile(r'\]\(([^)\s]+)\)')
INSTALL = re.compile(r'tools/install/install ([a-z-]+)')
RUNTIME = re.compile(r'(runtime/[a-z_/-]+\.py) ([a-z-]+)')


def status(url):
    request = urllib.request.Request(url, method='GET', headers={'User-Agent': 'openwiz-docs-check'})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status
    except urllib.error.HTTPError as error:
        return error.code
    except OSError as error:
        return str(error)


def ok(argv):
    return subprocess.run(argv, capture_output=True).returncode == 0


def tracked(path):
    return ok(['git', 'ls-files', '--error-unmatch', str(path)])


def main(package):
    failures, checked = [], 0
    for name in FILES:
        text = Path(name).read_text()
        for number, line in enumerate(text.splitlines(), 1):
            for word in STALE.findall(line):
                failures.append(f'{name}:{number}: stale words: {word!r}')
            for words in WHOLE_ROOT.findall(line):
                failures.append(f'{name}:{number}: deletes the whole install folder: {words!r}')
            for words in WHOLE_SERVICE.findall(line):
                failures.append(f'{name}:{number}: deletes all of service/, which holds settings: {words!r}')
            for words in WHOLE_STATE.findall(line):
                failures.append(f'{name}:{number}: deletes the whole state folder, which can be shared: {words!r}')
        for target in sorted(set(LINK.findall(text))):
            checked += 1
            if target.startswith('#'):
                continue
            if re.match(r'https?://', target):
                code = status(target)
                if not (isinstance(code, int) and code < 400):
                    failures.append(f'{name}: link {target} answered {code}')
            elif not tracked(Path(name).parent / target.split('#')[0]):
                failures.append(f'{name}: link {target} is not a tracked file, so GitHub answers 404')
        for command in sorted(set(INSTALL.findall(text))):
            checked += 1
            if not ok([sys.executable, '-I', 'tools/install/install', command, '--help']):
                failures.append(f'{name}: `install {command}` is not an installer command')
        for tool, command in sorted(set(RUNTIME.findall(text))):
            checked += 1
            if not ok([sys.executable, '-I', '-B', str(Path(package) / tool), command, '--help']):
                failures.append(f'{name}: `{tool} {command}` is not a command in the package')
    guide = Path('docs/install.md').read_text()
    for words in GUIDE_NEEDS:
        checked += 1
        if words not in guide:
            failures.append(f'docs/install.md: missing {words!r}')
    for words in BARE_CONTINUE.findall(guide):
        failures.append(f'docs/install.md: a bare "continue" stalls some agents: {words!r}')
    steps = [title for _, title in STEP.findall(guide.split('## What happens, in order', 1)[-1].split('\n## ', 1)[0])]
    setup = next((n for n, t in enumerate(steps) if t.startswith('Setup')), None)
    skills = next((n for n, t in enumerate(steps) if t.startswith('Skills')), None)
    checked += 1
    if setup is None or skills is None or not setup < skills:
        failures.append(f'docs/install.md: steps {steps} must put setup and logins before the skills and the restart')
    for failure in failures:
        print('FAIL', failure)
    print(f'{checked} links and commands checked, {len(failures)} failures')
    return 1 if failures else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1]))
