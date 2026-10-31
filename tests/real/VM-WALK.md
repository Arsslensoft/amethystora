# The checklist walked in the VM lab: what is left

Written 2026-10-31 for the agent that takes this over. The real machine's RAM is bad (`STATE.md`), so its unfinished
checklist was walked in two VMs of the local lab (`tests/vm`, `live.sh`, VMs `live1` and `live2`, an image of 9c1b60a
with every later `system_files` change and the apps pushed in live). The maintainer paused that walk on 2026-10-31.
Read `AGENTS.md` and `.claude/skills/real-machine/SKILL.md` (how a line is marked) first.

## How the marks read

- `- [x] ... → PASS (VM)`, `- [!] ... → FAIL (VM): ...`, `- [-] ... → N/A (VM): ...`: seen in a VM, not on hardware.
  Marks without `(VM)` came from the real machine.
- A FAIL fixed later becomes `- [x] ... → PASS (VM), fixed on main: what was seen; was: <the old FAIL>`.
- `FAIL ...: fixed in <commit>, not seen working here`: the code is on main, nobody has seen it run yet.
- Never PASS what was not seen working. Never configure or touch the duress feature.

Counts at 14d9f8e0 (`grep -c` of each box):

| File | open `[ ]` | FAIL `[!]` | PASS | N/A |
| --- | --- | --- | --- | --- |
| 10-desktop.md | 58 | 51 | 480 | 8 |
| 20-apps.md | 80 | 15 | 368 | 16 |
| 30-security.md | 47 | 8 | 127 | 5 |
| 40-system.md | 132 | 5 | 229 | 23 |

`grep -n '^- \[!\]' tests/real/checklist/*.md` lists every FAIL with its note.

## 1. Fixed on main, waiting to be seen (no code to write)

Each needs an image or a VM with main in it, then its mark.

- **The transitions' reveal:** D-THEME-13, D-TRANS-01 to 08, 10, 11, A-CONTROL-28. da7bf83 fixed the stuck picture, and
  6fd44614 put `transition Hold` before the theme is staged. The reveal was seen working in GNOME Shell's own captures
  every 0.4 s in live2. Each line still wants its own transition seen.
- f01296ab: S-WATCH-07, S-WATCH-09, S-SCAN-06, S-NET-17. 3d7908e1: S-LOCK-07, Y-UPD-08. 03a8c23c: Y-TOP-17 (needs a
  restic repository).
- D-TOPBAR-12 (029e959), D-TODAY-09 (240bf6f). D-TOPBAR-04 and 06 (82353f8) need two screens: the lab gives a second
  screen only through a suite of its own.
- b89e7ed5 (the force-quit list says "No app is open." and hides its button when empty, from Y-DSK-80's note) was
  never run.
- D-SHELF-05 is probably what 66d626f8 fixed: the shake's card left one second after the pointer did, while the hand
  was still shaking. A-SHELF-03 and A-CONTROL-107 are PASS with it.

## 2. Open bugs, no fix yet

Read the whole line in the checklist before starting one. What is known of the cause is noted here.

**Agent**
- **Y-AGT-19:** Opencode runs the change tools without asking. The launcher (`amethystora-agent`, `command_for`) allows
  the read tools in `OPENCODE_CONFIG_CONTENT`, and `OPENCODE_PERMISSION` sets only bash, edit and webfetch to ask.
  Opencode allows any tool named in neither, so `amethystora_change` ran unasked. Give the change tools (`change`,
  `software`, `run`, `undo`) `ask` there.
- **Y-AGT-24:** a tool server that ends while a root password dialog is up leaves its record in
  `~/.local/state/amethystora/agent-tools/actions.jsonl` at `result: "running"` (terminal mode: `"opened"`). After that,
  `ame agent history` shows "-> running", and the pill says "Agent asks for root" while any other tool server runs.
  Close such records when their server ends (`apps/agent-tools`), or have the pill count only live servers' records.
- Seen beside a PASS, not filed:
  - `amethystora-agent policy` printed "Level: know (agentic features on, changes on)" while capped at know.
  - A login shell's PATH had `~/.local/bin` five times over, after the agents' installers added it.

**Security**
- **S-LOCK-05:** a Files window open on `~/Private` keeps its file names on screen after the folder closes. FUSE's
  unmount ends Files' directory monitor, and Files never reloads. Files exports no reload over D-Bus: only `go-home`
  and `close-current-view` per window, which cannot tell which window shows `~/Private`. Not tried yet:
  - whether GIO counts the gocryptfs mount as a `GMount`, which Files leaves when it is unmounted;
  - something in GNOME Shell that closes or moves the Files windows titled Private when the folder closes.
- **S-DESK-03:** the top bar is drawn black after `ame desktop shield mark` until the overview opens. Seen three times,
  then not reproduced in five tries.
- **S-NET-01:** the zone's service list differs from the Expect. **A-SECURITY-28:** two drops 0.8 s apart from the
  result page.

**Desktop**
- **D-GRID-07:** nothing greys an app while it updates. Neither Bazaar nor `flatpak update` sends
  `com.canonical.Unity.LauncherEntry`. The shell's side works when the signal is sent by hand.
- **A-CONTROL-82 (a note on a PASS):** while quick settings are arranged with a tile hidden, the menu runs past an
  800 px screen and Done is cut off (`modules/quick.js`, `_buildHint`, `_buildHiddenRow`).
- D-QUICK-05, 11, 12 (a theme switch takes 10 to 12 s), D-TODAY-02, D-NOTIFY-06, D-FOCUS-11, 18, D-LOCK-03,
  D-SEARCH-05, 20, 21, D-SHELF-07, D-EMOJI-02, D-SHOT-16 with D-KEYS-13, D-CAPTIONS-03, D-DOCK-21, 39, D-MENU-01,
  D-TILESET-21, 22, D-CLASSIC-01, 03, D-MOTION-08, D-WS-06, D-THEME-17, D-WALL-03, 11, D-STYLE-22, D-GLASS-04,
  D-CLIPX-04, D-KEYS-15, D-OTHER-05.

**Apps**
- **A-LOGS-25:** apps whose launcher name has a hyphen fail, because `\x2d` is doubled (`apps/logs/src/backend.rs`).
- **A-MANUAL-28:** the Manual's cards ignore reduced motion: `cards.rs` reads GTK's `gtk-enable-animations` only.
- A-ALL-11, 12 (untranslated strings), A-MANUAL-08, 16, 22, 25, A-UPDATES-20, A-NOTES-09, 46, A-BACKUPS-02.
- Seen beside PASS marks, not filed:
  - 20-apps 95: the pill does not say when a snoozed notification comes back.
  - 157: Pick a look's preview scrolls out of view.
  - A-SHELF-08 and 160: no visible focus ring.
  - 94: a digest card missed once.

**Upstream or the maintainer's**
- Upstream: D-ICONS-05 (Files 50.4 has no Star) and Y-APP-27 (Extension Manager 0.6.5).
- The maintainer's call: A-UPDATES-31.

## 3. Lines not walked yet

- **10-desktop.md:**
  - Walkable in a VM: D-POINTER-06, D-TILE-23, D-TILESET-19, 23, 25, D-WIN-13, 14.
  - Then re-check: D-TOPBAR-12, D-TODAY-02, D-TODAY-09, D-THEME-13, D-TRANS-*, D-KEYS-15, D-OTHER-05.
  - The other open lines need hardware, two screens, a touchpad or a phone, or are `risky`.
- **20-apps.md:**
  - Walked to its end in the VM.
  - Left for hardware or a person: 86, 104, 108, 118, 159, and A-PHONE-05 to 27, 31 to 33.
  - `risky`: 120, 121, 122, 126, 135, 136, 137, 143, 144, 149, 150.
  - Not done, each needing setup: 111 (a Private folder), 117 (another computer), 123 (installs OpenTabletDriver),
    140 (a key and the file dialog), 155 (a backup).
- **30-security.md:** S-BROWSER-02, 03, 04, then S-DESK-04, 05, 07, then the other open lines.
- **40-system.md:** from Y-APP-04 (`ame apps bbrew`) to the end.

## 4. Lab notes (for a session with the lab)

- live1 and live2 are still running in the local lab with this walk's helpers in `/usr/local/bin` (each helper's
  header says what it does). Two leftovers there:
  - Test records in tester's `actions.jsonl` on live2.
  - Bazaar and kitty open on live1, its login keyring unlocked.
  Never `live.sh reset` a VM someone else is walking.
- Pushing an extension file restarts the session. Once the account did not come back (only GDM's greeter):
  `live.sh relogin` brings it back.
- A JS template string's `${...}` inside a command passed to `live.sh exec` is expanded by the local shell. Join
  strings with `+`, or base64 a script.
- From a root `exec`, reach the session as `runuser -u tester -- env
  DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$(id -u tester)/bus ...`.
- `keys.py shake:X,Y,DX,STROKES,MS,held` lets go of the button at its end.
  `/run/amethystora-tests/sharecast.py` shares the whole screen; `sharewin.py` shares one window.

## 5. Finishing

When the fixes are in: build HEAD in the lab, run the whole suite (`--parallel 2`, at most two VMs, none beside a
build), and only when it is green queue GitHub's Latest build (the `queue-latest` skill). Without the lab, a
`test/<sha>` branch builds on GitHub and runs `20-tests.sh` only (`run-tests` skill, section 5): it publishes nothing
and validates no suite. Never start Latest without a green full run.
