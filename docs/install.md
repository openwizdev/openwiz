# Install OpenWiz

Paste this one line into the coding agent you already use (Claude Code, Codex or another):

> Install OpenWiz from github.com/openwizdev/OpenWiz

The agent follows the [install skill](https://github.com/openwizdev/WizSkills/blob/maint/0.2/skills/install/SKILL.md). This page says what it does, the commands it runs, what it may change, and how to recover or remove it.

Using Grok Bot? See [Grok Bot + OpenWiz](grok-bot.md).

**Status:** this is the install for OpenWiz 0.1.0 ("OpenWiz 0.01"). It needs CloudRoom: setup is done only when CloudRoom is installed and signed in.

## Before you start

- **A supported computer:** macOS on Apple silicon, or Linux on x86_64 with glibc 2.34 or later (`ldd --version` shows it). Linux on ARM is not supported. **Intel Macs are not supported for 0.1.0:** OpenWiz 0.1.0 needs CloudRoom, and CloudRoom offers no Intel Mac build today, so an Intel Mac cannot complete the 0.1.0 install yet. The `openwiz` program itself runs there, and preflight still accepts an Intel Mac. On Linux older than glibc 2.39, `openwiz` may print a `GLIBC_2.39` warning and still work.
- **Python 3.9 or later, `git`, and the GitHub CLI** (`gh`, signed in with `gh auth login`).
- **CloudRoom,** installed, opened once and signed in. Opening it once creates its `room-cli`.

## What happens, in order

The agent runs each command itself and reads its JSON. `<root>` is the install folder, by default `~/.local/lib/openwiz`: use a new or empty folder that holds nothing else, because the installer also accepts a folder that already has other files in it. If you give `install install` a separate `--state-root`, it too must be a new or empty folder used by OpenWiz alone, never one shared with another app. `<target>` is `aarch64-apple-darwin`, `x86_64-apple-darwin` or `x86_64-unknown-linux-gnu`.

1. **Preflight.** It clones this repository at the `v0.1.0` tag into a private temporary folder as `$openwiz` (`git clone --depth 1 --branch v0.1.0 https://github.com/openwizdev/OpenWiz "$openwiz"`) and checks this computer: Python, a supported system, free disk space, and whether `room-cli`, `gh` and `git` are installed. It reads only; anything missing comes with its next step. On an Intel Mac it stops here, as "Before you start" says.

   ```sh
   python3 -I "$openwiz"/tools/install/install preflight
   ```

2. **Verified download and install.** It downloads the release's checksum file, manifest and package from GitHub and checks the package's SHA-256 hash against the manifest before using it. No Rust, Node or source build is needed. A hash proves the files are the release's files; it is not a publisher signature. A failed check changes nothing, and an earlier install keeps working.

   ```sh
   python3 -I "$openwiz"/tools/install/install download --target <target> --dir <new private folder>
   python3 -I "$openwiz"/tools/install/install install --assets <that folder> --target <target> --root <root>
   ```

   `install` prints `"activated": true`, the version, and the `build` and `state_root` folders, written below as `<build>` and `<state>`. `download` reads GitHub's latest release, which skips a release marked pre-release; if it refuses with "could not be completed safely", check the [release page](https://github.com/openwizdev/OpenWiz/releases/tag/v0.1.0). The same three files are under `https://github.com/openwizdev/OpenWiz/releases/download/v0.1.0/`: `openwiz-<target>.sha256`, `openwiz-<target>.manifest.json` and `openwiz-0.1.0-<target>.tar.gz`. Put them in a new private folder (mode 700, files 600) and run `install install --assets` on it; it checks every hash the same way.

   Then it proves the build: `<root>/current/openwiz --build-identity` prints the version (`0.1.0`) and the source commit. The `openwiz` program lives there, not on your `PATH`: typing `openwiz` alone says "command not found".

3. **Setup and logins.** Nothing opens in a browser by itself: the agent reads each connection with one check, run from the installed build, and reruns it after each change.

   ```sh
   OPENWIZ_STATE_DIR=<state> python3 -I -B <build>/runtime/setup/setup.py check --state-dir <state>
   ```

   It prints one code per part: `cloudroom`, `personal_github`, `github_app`, `harness` and `project`. On a new computer expect codes such as `app_missing`, `harness_unselected`, `identity_unselected` and `project_unprepared`: those wait until you pick a repository. A connection is ready only when a check proves it, never because a setting was saved.

   - **CloudRoom:** `runtime/cloudroom/cloudroom.py status` reads whether CloudRoom is installed and signed in; `runtime/cloudroom/cloudroom.py install` shows CloudRoom's published installer and its checksum and waits for your yes; `runtime/cloudroom/cloudroom.py sign-in` opens CloudRoom's own sign-in once, where you sign in yourself. Each takes `--state-dir <state>`. CloudRoom is done when `check` reads `cloudroom_login_verified`.
   - **GitHub:** `runtime/setup/github_login.py sign-in --state-dir <state>` reuses your existing `gh` login, or shows what GitHub's own browser sign-in will do and waits for your yes. You paste no token.
   - **`identity_unreadable`** (for `personal_github`): GitHub's answer could not be read just then. Wait a minute and run `check` again; it usually clears.

4. **Skills into your agents, then a restart.** Only now, with setup checked and the logins done, it copies OpenWiz's 18 skills into each agent found on this computer: `~/.claude/skills` (Claude Code), `~/.codex/skills` (Codex), `~/.cursor/skills` (Cursor), `~/.config/opencode/skills` (OpenCode) and `~/.grok/skills` (Grok). A skill of yours with the same name stops the step before anything is written.

   ```sh
   python3 -I "$openwiz"/tools/install/install skills --root <root>
   ```

   The install skill it copies comes from the 0.1.0 package: it is older (`version: 0.2.0`) and still sends the agent to a setup step that 0.1.0 removed. So the agent then overwrites each agent's OpenWiz `install` skill folder (the one holding `.openwiz-skill`) with the fixed one, and checks every copy by its content:

   ```sh
   umask 077; fixed=<new private folder>/WizSkills
   git clone --depth 1 --branch maint/0.2 https://github.com/openwizdev/WizSkills "$fixed"
   if grep -q '^version: 0.2.2$' "$fixed/skills/install/SKILL.md" && ! grep -q 'install ope[n] ' "$fixed/skills/install/SKILL.md"; then
     for d in ~/.claude/skills ~/.codex/skills ${CODEX_HOME:+"$CODEX_HOME/skills"} ~/.cursor/skills ~/.config/opencode/skills ~/.grok/skills; do
       [ -f "$d/install/.openwiz-skill" ] || continue
       cp -R "$fixed/skills/install/." "$d/install/"
       if grep -q '^version: 0.2.2$' "$d/install/SKILL.md" && ! grep -q 'install ope[n] ' "$d/install/SKILL.md"; then echo "ok: $d/install"; else echo "STILL OLD: $d/install"; fi
     done
   else echo "NOT THE FIXED SKILL: nothing copied"; fi
   ```

   It copies the whole folder, scripts included, and only into folders marked as OpenWiz's own. Restart only when every line reads `ok:`. `NOT THE FIXED SKILL` means the download failed or WizSkills does not serve version 0.2.2 yet: nothing was copied, so do not restart; try again later. `STILL OLD` means that folder's copy did not take: fix it before restarting. A later `install skills` run, such as a repair, writes the old copy again: repeat this block after it.

   Your agent finds the skills the next time it starts. Restart it (a new session), then say exactly: "continue the OpenWiz install". It resumes at the first unfinished step, from the installed files.

5. **Explain.** A few sentences on ideas, grills, specs, tickets, builders, independent review and merges.
6. **First fleet.** The same agent continues through the install skill's first-fleet section when you want to try it:
   - It offers a few quick ideas and a new private demo repository, or one of yours. That one repository is used from here on.
   - It connects GitHub for that repository. Then you choose your own OpenWiz App (Create, then Install on that repository only) or skip it. If you skip it, your personal GitHub login acts for the factory and setup says so; the App is never shown as verified.
   - It saves the project with your harness, model and effort, asked once, and makes its private brain (`openwiz init`): in your repository if it is private, or in a new private companion repository if it is public. It tells you where in one line and opens a pull request you merge.
   - It grills you briefly and plans 4 to 7 small tickets. You confirm the spec, then approve the exact plan, as two separate answers.
   - It asks where the agents run, then shows the start: one fleet thread with its own loop, which starts the approved tickets' builders and their reviewers. Only your yes to that starts anything, and that yes covers its Cloud Credits.

## What it may change, and when

- **Nothing without your yes.** Before signing in, creating or skipping the GitHub App, saving project settings, creating the brain, setting up phone access or starting agents, the agent shows exactly what will happen and waits for your yes to that.
- **Installing is not approving work.** Saying yes to the install or setup approves no plan. Agents start only after you confirm a spec and approve its plan.
- **Your files.** The install writes only to its own folder (by default `~/.local/lib/openwiz`), its private state inside it, and OpenWiz's skills in your agents' skill folders (step 4). Existing settings, credentials and skills of your own are never replaced.
- **GitHub.** Your own OpenWiz App is private to your account and installed only on repositories you select, with read access to metadata, checks and actions and write access to contents, issues and pull requests. Your personal login acts only if you explicitly skip the App; an App that fails or is removed never switches to it on its own.
- **Credentials.** Keys stay in owner-only storage: OpenWiz's private state on this computer, or CloudRoom's own secret store. If setup asks you for the App's key, paste it only into CloudRoom's secret prompt. Never paste a key or token into the chat.
- **Cost.** OpenWiz is free now. Paid plans come later. CloudRoom and model charges are separate and not included. Cloud agents, including each fleet thread, use your CloudRoom Cloud Credits, and your model provider bills you directly. More credits are CloudRoom's own plan, bought from [CloudRoom](https://cloudroom.dev/pricing), not from OpenWiz. Referral verification is unavailable. No referral is applied.

## If it is interrupted or fails

Start a new session of your agent and say "continue the OpenWiz install": every step checks what is already done before doing anything. A repeated install reuses the verified build. A refused install or update leaves the last good build in use. Setup, sign-in, App and brain steps remember their operation, so an interrupted step is finished or replaced, never done twice; an unclear result is checked before anything is retried.

`install status --root <root>` checks the installed build against its manifest. If it reports `installed asset mismatch`, a file in the build was changed, and installing the same release again refuses the same way. To repair it and keep your settings, delete only the `<root>/current` link and the one build folder it points to (`<build>`, inside `<state>/service/builds/`), and nothing else in `<state>/service/`. Then run step 2's `install install` again. If a connection shows a code that is not ready, follow its step above, then run `setup.py check` again.

## Remove it

Ask the agent to remove OpenWiz. There is no remove command, and the agent never removes the whole of `<root>`: the installer accepts a folder that already holds other files, so it may hold yours.

- **Look first.** List what is in `<root>` (`ls -A <root>`). A fresh install puts exactly these there: `current` (a link to the installed build), `channel.json`, `install.lock`, `skills/` (after step 3) and, unless you gave a separate `--state-root`, `state/` (your `<state>`, below). Delete only `current`, `channel.json`, `install.lock` and `skills/`. Keep anything else, and any of those names that was there before you installed: you or another program put it there.
- **Then look in `<state>`** (`ls -A <state>`). It is never deleted as a whole: the installer also accepts a state folder that holds other files. OpenWiz's own entries there are exactly these:
  - **the builds:** the folders in `<state>/service/builds/`, each named by a 40-character source commit and holding an `install-manifest.json` whose `"source"` is that same name;
  - **your settings:** `<state>/setup/`, `<state>/cloudroom/`, and in `<state>/service/`: `config.json` (the Decisions repository pair, saved when your brain is in a companion repository), `app/` (your OpenWiz App's saved settings and key), and `state.json`, `loops/` and `clone/` when present.
- **To keep your settings,** delete only those verified build folders. `<state>/service/` is never deleted as a whole.
- **To delete your settings too,** delete exactly the named entries above that exist, then `rmdir <state>/service/builds <state>/service <state>`. `rmdir` removes a folder only when it is empty, so anything of yours or another program's stays; if it says a folder is not empty, leave that folder. Last, `rmdir <root>` removes the install folder if nothing else is left in it.
- **The skill copies stay.** To remove them, delete only OpenWiz's 18 skill folders from each agent's folder listed in step 4: `ask-wiz`, `build-ticket`, `diagnose`, `grill`, `handoff`, `hill-climb`, `huntr`, `image`, `install`, `launch-fleet`, `research`, `review-ticket`, `setup`, `to-spec`, `to-tickets`, `triage`, `wayfind` and `where-are-we`. Other skills there are yours.
- Your repositories, branches, pull requests and CloudRoom threads are your work and are never deleted.

**Removing OpenWiz from this computer does not revoke its access.** To revoke it:

- uninstall OpenWiz's GitHub App, or remove repositories from it, in your GitHub settings;
- delete any Cloudflare Tunnel and Access application, or Tailscale rule, made for phone access;
- delete a fleet thread's loop in CloudRoom when you no longer want that fleet to run.
