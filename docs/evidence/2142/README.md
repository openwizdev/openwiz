# #2142 evidence

- `red.log`: `tools/install/tests/installer/test_installer.py` at the red commit, before the fix: 16 failures and errors, each at the missing behaviour (temporary paths and the port replaced with placeholders).
- Fixture proof after the fix: the same file, 11 tests, OK. A local HTTP server stands in for the release host (a 404 on `latest`, a releases feed, tagged assets), a fake `ldd` stands in for glibc, a damaged build and an empty root.
- One read-only live check on 2026-11-21: `latest/download/openwiz-x86_64-unknown-linux-gnu.sha256` returned 404, and `install download --target x86_64-unknown-linux-gnu` with no `--version` fell back to the newest tag and downloaded v0.1.0 (source `2abcd5f532c5c2a080a1b876e27dec2b9e7d29ab`), checked against its published checksum. Nothing was installed or run; the folder was deleted.
