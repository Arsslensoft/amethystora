# Fixing live in a kept VM

The maintainer's rule (2026-10-29): build and test round trips are the expensive part, so a failure is fixed in
a running VM of the image the run built and validated there, then committed, and only one final full run
checks the commits together. The exact commands are below; they are written and proven against the lab by a
tester before they are relied on.

## The loop

1. **Keep a VM.** End the run with the VM left running (`run.sh test --keep`, or start one from the last run's
   disk under a name of its own). Never reuse another session's VM.
2. **Make the system writable for this boot.** The image is read-only; an unlocked deployment gives a writable
   overlay on `/usr` until the next restart, so a fix tried live never survives into the next boot.
3. **Put the fix in.** Copy the changed file from the checkout into the guest with LF line endings (the
   checkout is CRLF on Windows).
4. **Reload what reads it.**
   - A GNOME Shell extension file: log the test account out and back in (on Wayland the shell cannot reload
     an extension in place).
   - A `/usr/libexec` helper or an `ame` recipe: nothing, the next call reads it.
   - A systemd unit: `systemctl daemon-reload`, then restart the unit.
   - An Amethystora app: rebuild the one program in the `amethystora-rust` container and copy the binary in.
5. **Rerun only what failed.** Run that suite, or that step, against the VM, and read its log and screenshot:
   the issue is fixed when it is gone from both, not when the count is.
6. **Repeat** until it passes there, then **rerun it once more** to be sure it is stable, and run the other
   suites of the current work the same change touches. Carry the change into the checkout and commit it (one
   commit per cause).
7. **Reset** the VM to the run's clean disk before validating the next, unrelated fix, if the last one
   changed state the next would see.
8. **One final build** with every live fix. On it, the **full suite only if the changes are risky**
   (packages, the build, boot, security, PAM, the kernel, the image's units, or a shared library many suites
   use), else a final test of the current work's suites. While iterating, never run every suite.

## What cannot be fixed live

The kernel, the initramfs, a package, a change to `20-tests.sh` or another build-time script. Check such a fix
in a container of the image (`ghcr.io/arsslensoft/amethystora:stable` or the run's own image) and leave it to the
final run.

## Exact commands

`tests/vm/live.sh` does the loop (it copies `tests/vm/live.mjs` into the lab and runs it there, beside the lab's own
`lab.mjs`; it never touches a run's `/build/src`). Everything is per VM: `VMLAB_LIVE=NAME` (default `live`), with its
files in the lab's `/vm/live/NAME/` (`runs/SUITE.log` and `.png`). Docker on PATH and `MSYS_NO_PATHCONV=1` as in
[SKILL.md](SKILL.md). Times measured on round 1's image (r1), 16 processors and 14 GB in the lab.

**A disk to keep.** A run's `make-disk.sh` replaces `/vm/disks/test.qcow2` with `mv`, which leaves an overlay on it
pointing at a new base. Keep the run's disk under a name of its own first, a hard link (no space, the inode stays):

```bash
docker exec amevm sh -c 'cd /vm/disks && ln test.qcow2 r1.qcow2 && ln test.vars.json r1.vars.json && cp test.digest r1.digest'
docker exec amevm podman tag localhost/amethystora-run:test localhost/amethystora-live:r1   # for live.sh delta; a run prunes untagged images
```

**Start, sign in, keep the state** (once per disk):

```bash
export VMLAB_LIVE=live1
export VMLAB_MEM=3072                    # optional: a 3 GB guest (default 4096), so that a third VM fits beside two
bash tests/vm/live.sh start r1 --fresh   # 70-120 s: overlay, boot, `bootc usr-overlay`
bash tests/vm/live.sh ready              # 10-account.sh (50 s), flatpak-preinstall waited for (585 s after a first
                                         # boot), powered off from inside and kept as the stopped VM live1-ready
bash tests/vm/live.sh reset              # live1 deleted and started fresh `from` live1-ready (vm_start's from, as
                                         # run.sh --parallel): booted, session up, /usr unlocked, pushes forgotten
bash tests/vm/live.sh save               # the running VM kept whole, memory too (QEMU migration into a file)
bash tests/vm/live.sh restore            # back to it, running and signed in, without a boot, with the memory the
                                         # saved guest had (VMLAB_MEM at its start)
```

`ready` waits for flatpak-preinstall because a state kept half way installs the Flatpaks again at every start, and
the account's shell then starts so late that GDM puts its greeter in front and GNOME locks the session. It powers
off from inside: after vm_stop's power button the next boot came up the same way.

**One fix round:**

```bash
bash tests/vm/live.sh affected [REV]     # 1.5-3 s: the suites whose "# vmlab-paths:" a change since REV touches
bash tests/vm/live.sh push-diff [REV]    # every changed system_files/ and suite file (REV default HEAD)
bash tests/vm/live.sh push FILE...       # or named files: 21 s for one extension file (most of it the relogin)
bash tests/vm/live.sh suites 11-dock 12  # each suite in turn; prints its counts, every not ok, the log and png paths
bash tests/vm/live.sh round [REV]        # push-diff, then the affected suites
```

`push` puts the files in with LF endings and the checkout's modes (git's, else Git Bash's `-x`), labels them for
SELinux, and reloads what reads them: extension schemas compiled per extension with the image's overrides (as
`build-gnome-extensions.sh`; the image's own extensions put back in zz0's `enabled-extensions`, which the source file
lacks), the system schemas, `dconf update`, icon caches, `systemctl daemon-reload` (restart the unit yourself),
`amethystora-theme reload` for themed templates, and a new session for anything the shell reads. `/etc/...` goes to
`/etc` and `/usr/etc`. A pushed `/usr/libexec/amethystora-sensor` is started anew: its unit refuses a manual stop, so its
main process is ended and systemd starts the pushed binary. 328 files, everything main changed since round 1, took 51 s.
A restart throws `/usr` away. `live.sh` unlocks it again after one and puts back every file pushed since the last reset
(`push.list`) and the apps program `live.sh app` put in. Every restart a suite asks for (exit 100) starts QEMU anew on the same disk:
a fresh VM is powered off from inside, and a restored one is synced and cut. QEMU's uefi-vars device does not survive the
firmware running again in the same process: a restored QEMU died in it, and a fresh 3 GB one froze.

**Look inside:** `live.sh exec 'CMD'` (root), `live.sh exec --session 'CMD'` (the test account's session),
`live.sh shot FILE.png`, `live.sh relogin`; `stop`, `delete`.

**What a push cannot reach** (the build turns files into others: emoji data, the shell's gresource, translations, the
agent's index, the apps): `live.sh delta REV` builds an image `FROM` the VM's own image with the working tree's
system_files and only the build steps they feed since REV (`tests/vm/delta.sh`), pushes it to the lab's registry
and switches the VM to it with `bootc switch` and a soft reboot when it allows one. The apps need their build:
`DELTA_APPS=<an apps stage image>`, or `live.sh app` (a release build of the apps in `amethystora-rust`, hard-linked to
every app's path as `13-apps.sh` does, the Phone and the Shelf included). `app` builds only `amethystora-apps`. To put the agent's tools in
too, build them and leave the binary in the lab as `/vm/live/NAME/amethystora-agent-tools` first: `app` then installs
it as `/usr/libexec/amethystora-agent-tools`.

**Rules learnt:**
- **No VM beside a lab build.** A build and one 4 GB VM filled Docker's 13 GB and wedged it (2026-10-29, again).
  Beside nothing else, two VMs at 4 GB, or three at 3 GB each (`VMLAB_MEM=3072`); beside a run's test stage, one.
- A suite asking `next_boot` (a power cycle with disks or credentials) or `usb_next` needs `run.sh test --suites NN`;
  `live.sh suite` does plain restarts (exit 100) and `continue_phase` (101). Nor can a suite that deploys a commit of
  its own and restarts into it (39, 41, 49) be judged live, since what it tests is a deployment other than the one
  pushed into. The same goes for one that uses the installer VM (46): leave those to `run.sh test --suites` or the final run.
- `10-account.sh` sets the test account's `idle-delay` to 0: the suites drive the session through Eval and AT-SPI,
  which GNOME does not count as use, and a lock mid-suite turns the extensions off. A suite that wants the lock locks it.
- GDM's automatic login leaves the login keyring locked (oo7's and gnome-keyring's), and the first Secret portal
  request then puts up GNOME's unlock dialog, which takes every click. `suites/lib.sh` unlocks it once a boot
  (`unlock_login_keyring`, marked by `/run/vmlab-keyring`). A suite that restarts GDM itself calls it again after
  the restart (16 does).
- The guest's agent answers one client at a time: never leave a long `exec` running while something else needs the VM.
- Never edit `live.sh` while it runs (bash reads a script as it goes).
- A run's own VM (`test`, `test-screens` after `run.sh test --keep`) can be pushed into and tested, never
  started, reset or deleted from `live.sh`.
