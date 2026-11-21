# #2140 evidence: the README and install guide against the 0.1.0 package

`check_docs.py` is this ticket's proof. From the repository root:

```sh
python3 -I docs/evidence/2140/check_docs.py <unpacked openwiz-0.1.0-x86_64-unknown-linux-gnu package>
```

It reads `README.md` and `docs/install.md` and prints one line per failure:

1. **No stale words:** a "coming soon" 0.1.0, 0.0.12 as current, the removed setup page, port 7370, `install open`, or Intel Mac listed as supported or sent to a CloudRoom installer (CloudRoom offers no Intel Mac build).
2. **Every link resolves:** a relative link to a file Git tracks (a gitignored path such as `skills/` is a 404 on GitHub), a web link to an HTTP status below 400, redirects followed.
3. **No whole-folder delete:** no instruction deletes the install folder itself (`rm -rf <root>`, "delete the install folder", "remove the install"), because the installer accepts a folder that already holds other files.
4. **No whole `service/` delete:** no instruction deletes `service/` as a whole, because it also holds settings (`service/config.json`, `service/app/`); only the builds in `service/builds/` are removable.
5. **No whole state-folder delete:** no instruction deletes `<state>` (or `state/`) as a whole, because the installer accepts a state folder shared with other files.
6. **The install skill's order and steps:** setup and logins come before the skills and the restart; the guide has the overwrite step (each agent's `install/` copy marked `.openwiz-skill`, checked for `version: 0.2.2`), the exact resume phrase "continue the OpenWiz install" (never a bare "continue"), and the `identity_unreadable` row.
7. **Every command exists:** each `tools/install/install <command>` is a command of this checkout's installer, and each `runtime/<tool>.py <command>` is a command of that tool in the unpacked 0.1.0 package (its `--help` exits 0).

Exit 0 when all hold, 1 otherwise. It reads only and installs nothing. The package came from `https://github.com/openwizdev/OpenWiz/releases/download/v0.1.0/`, with its SHA-256 checked against the release's `SHA256SUMS`.

- `red.txt`: the run at `main` before the fix (exit 1).
- `red-intel-mac.txt`: the run after the Intel Mac checks were added, before those lines were fixed (exit 1).
- `red-skill-order.txt`: the run after the skill's order and steps were required, before the guide was reordered (exit 1).
- `red-whole-state.txt`: the run after deleting the whole state folder was added as a failure, before the settings-removal steps were fixed (exit 1).
- `red-whole-service.txt`: the run after deleting all of `service/` was added as a failure, before the build-only steps were fixed (exit 1).
- `red-whole-root.txt`: the run after the whole-folder check was added, before the removal and repair steps were fixed (exit 1).
- `removal-run.txt`: the guide's repair and removal steps run on the real 0.1.0 package, with a file of the user's already in `<root>` and a skill of the user's in `~/.claude/skills`: both survive, settings survive the repair, and a separate `--state-root` holds the builds.
- `service-run.txt`: a fresh install and a Decisions setting saved by the packaged first-fleet helper, then the guide's build-only removal: only the verified build folder goes, `service/config.json` and `setup/state.json` are unchanged, and a reinstall keeps them.
- `shared-state-run.txt`: the guide's full removal on the real package, with a `--state-root` that already held another app's files (including one inside its `service/`): every OpenWiz entry goes and the other app's files stay; with a dedicated `<root>` and the default state, nothing is left.
- `overwrite-run.txt`: step 4's overwrite block, run as written after `install skills` on the real package. Against `maint/0.2` today it prints `NOT THE FIXED SKILL` and copies nothing. Against WizSkills #239's head it overwrites both marked `install` folders with version 0.2.2, scripts included, keeps the `.openwiz-skill` marker, and leaves an unmarked folder of the user's alone.
- `green.txt`: the run on this branch's docs (exit 0).
