# Diagnosing a failed run

Every path here is inside the lab container; read them with `docker exec amevm ...` (with
`MSYS_NO_PATHCONV=1` on Windows). `/vm/runs/last` is the latest run.

## Finding the first real error

**The build** (`build.log`, a `podman build` of the `Containerfile` with every script traced by `set -x`):

```bash
grep -n "::group:: ===\|Error: building at STEP\|::error::" /vm/runs/last/build.log | tail -n 15
```

Each build script starts a `::group:: ===NAME===` line, so the last one before `Error: building at STEP` names
the script that failed. Above the error, the last `+ ` line is the command that failed, and the lines
between are what it said. `20-tests.sh` runs inside the build, so its checks fail the build too: look for
`::error::`, `Missing package`, `... Exiting`, or the last traced test before the exit. A step before the
scripts (`just build`'s own checks: `verify-container`, `skopeo`, the pinned digests) fails before the first
`::group::`.

**The disk** (`disk.log`): `bootc install to-disk` and `virt-fw-vars`. A failure here is almost always the
lab's: `/dev`, udev, loop devices or space.

**A suite** (`NN-name.sh.log`, `NN-name.sh.png`, `journal.log`):

- `not ok - ...` lines, each followed by indented lines saying why; the suite's stderr follows `--- stderr`.
- `--- phase 2` marks the part after a reboot.
- `could not run: ...` in `run.log` means the runner lost the VM, not that a check failed.
- The screenshot is the screen right after the suite: `docker cp amevm:/vm/runs/last/20-apps.sh.png
  SCRATCHPAD/` and Read it.
- `journal.log` is the guest's warnings and failed units at the end of the run.

To look closer: `bash tests/vm/run.sh test --keep`, then the `amethystora-vm` MCP tools on VM `test`
(`vm_exec`, `vm_read`, `vm_screenshot`, `vm_serial`). When the tools are not loaded in the session, a short
Node script in the lab calls the same ones: `import { call } from '/opt/amethystora-vm/lab.mjs'`, then
`await call('vm_exec', { name: 'test', command: '...' })`.

## Trying a fix before rebuilding

A build is most of an hour, so check the fix on its own first:

- **A line of a build script** (a glob, a `sed`, a package name): run it in a container of the published
  image, or of the base image the build starts from, in the lab:
  `docker exec amevm podman run --rm --entrypoint bash ghcr.io/arsslensoft/amethystora:latest -c '...'`.
  After a failed build there is no new image; the published one or `ghcr.io/ublue-os/silverblue-main:44`
  stand in.
- **A check in `20-tests.sh`**: run its lines in the published image with the changed files copied over it.
- **A file the image ships** (a unit, a drop-in, a configuration, a script in `/usr/libexec`): try the same
  change on the running VM first. `/etc` takes a drop-in or a copy directly; `/usr` is read-only, so put a
  drop-in in `/etc/systemd/...`, or `rpm-ostree usroverlay` for a test that ends at the next reboot. Then
  rerun the suite's checks by hand with `vm_exec`, and only then change the image and rebuild.
- **A suite**: commit it and `run.sh test`, which uses the disk the last run made.

## The lab or the image?

A failure is the lab's when the suite asked the wrong question or the runner lost its way, and the image's
when the machine did the wrong thing. Fix each in its own place: the lab in `tests/vm`, the image in
`system_files` and `build_files`. When unsure, try it in the VM by hand the way a person would, and check
whether it fails the same way on the published image, where the feature is known to work or not.

Problems the lab has already met, with their fixes:

| What you see | Why | What works |
| --- | --- | --- |
| `Connection reset by peer`, `Transport endpoint is not connected` from `systemctl --user -M` or `systemd-run --machine` | from a system service, machined's route to an account's manager breaks | `lib.sh`'s `user_systemctl`, `user_bus` and `in_session`: `runuser` with the account's `XDG_RUNTIME_DIR` and bus |
| `pgrep -x some-long-name` finds nothing | `-x` sees only the first 15 characters of a process's name | `pgrep -f /path/of/the/program` |
| `gnome-session-service` of `gnome-initial-setup` aborts | `10-account.sh` restarts GDM under it | count only the test account's crashes (`crashes_since TIME COREDUMP_UID=...`) |
| `OSError: [Errno 12] Cannot allocate memory` reading a file | `/proc` and `/sys` allocate what one `read()` asks for | the helper reads in 64 KiB pieces; keep it that way |
| a check of the desktop fails at once, passes by hand | GNOME draws in software under nested virtualization: seconds behind | `wait_for SECONDS COMMAND` on the state, never a short fixed `sleep` |
| an audit-based detection finds nothing | audit rules are written at boot; `ausearch` gives an event only once a later one is 2 s newer | switch the key, then `reboot_and_continue`; make a later event before running the watcher |
| `the VM did not come back within 15 minutes of a reboot` | the boot hung, or the helper did not answer | `vm_serial` and `vm_screenshot` on VM `test` with `--keep` |
| the VM hangs right after GRUB | Secure Boot firmware with SMM under nested KVM | the lab boots with `uefi-vars` and `smm=off`; keep it |
| QEMU dies loading a snapshot | QEMU 10.2's `uefi-vars` device crashes on a live snapshot | `vm_snapshot` keeps checkpoints of a stopped VM only |
| `docker exec` says the container is not running | another session stopped `amevm` | `run.sh` starts it; `setup.sh` rebuilds it |
| `no /dev/kvm` | Docker Desktop forgot the KVM module when it restarted | `run.sh` and `setup.sh` load it again |
| a file copied into the lab misbehaves | the Windows checkout has CRLF line endings | `run.sh` sends a git bundle, which holds LF; strip `\r` from anything copied by hand |
| `run.log` says "no session after this boot: powering the VM off and on again", or the suites after a restart all skip with "no session" | a boot stalled (about 1 boot in 50 at idle, 1 in 10 after busy suites, 2026-10-13): systemd 257 and later asks the console for its size with escape sequences and waits for the answer, which never comes while plymouthd holds the console (TIOCCONS). `stalled-boot-N.txt` shows PID 1 in `redirected_tty_write` → `n_tty_write` → `wait_woken` and plymouthd in `tty_read` → `n_tty_wait_for_input`, just after `systemd-vconsole-setup.service` restarted twice within 10 ms; the screenshot shows the console after switch-root | `systemd.tty.term.console=dumb` (`kargs.d/12-amethystora-console.toml`) skips the query until Fedora's systemd has systemd/systemd#43802; `run.mjs` still powers a stalled VM off and on, twice at most, then fails the suite; the stall is systemd's, not the suite's |
| a phase's lines are all lost: "could not run: no answer on test.agent within 1815 s" | the phase ran past its half hour, often a terminal question that never came (`in_terminal` waits its whole timeout) | split the suite's phase; read what it did in the guest's journal (`sudo` logs each command) |
| the test account cannot run or read a suite's file | `50-raid.sh` makes `/var/lib/vmlab-tests` root's alone (0700) | keep what the account uses in `/run/<suite>` or in its home |
| `dbus-monitor` shows every notification twice | GNOME's notification service owns `org.freedesktop.Notifications` and passes each `Notify` call on to the Shell; both show only unique names | count the calls that carry the `x-shell-sender` hint the service adds |
| downloads fail in several suites at once ("lookup quay.io: no such host", Homebrew's and Flathub's), then work again in later ones | the Windows host's own DNS dropped out: the guest's resolved asks QEMU's 10.0.2.3, which asks the host (2026-10-13, about half an hour); Docker Desktop's `%LOCALAPPDATA%\Docker\log\host\httpproxy.log` shows its own lookups failing then | not the image's or the suite's: run the suites again |

Add a row whenever a run teaches the lab something new.

## Fixing

- Fix the cause where it lives, in the fewest lines, matching the code around it.
- A check that is wrong is fixed, and the commit says why it was wrong; a check is never loosened only to
  pass.
- Stage your files by name. A file another session has uncommitted changes in is not yours to edit: report
  the failure instead.
- Commit each cause on its own: `fix(scope): ...`, a body with what the run found, and the `Assisted-by`
  footer only.
- The bug gets the check that would have caught it: in `20-tests.sh` when it is a fact of the built image (a
  package, a file, a pattern that matched the wrong thing), in `tests/vm/suites` when it only shows on a
  running machine.
