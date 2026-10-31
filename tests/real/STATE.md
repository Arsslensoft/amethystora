# Real-machine testing: state

Resume from `git pull` alone: read this, then `.claude/skills/real-machine/SKILL.md`. Updated with every push of marks.
If the machine hung: first save the tails of `journalctl -b -1` into `tests/real/hangs/`, push, then go on.

## Now

- **Since 2026-10-31 the checklist was walked in the VM lab instead, and that walk is paused:** `tests/real/VM-WALK.md`
  says what is fixed and waiting to be seen, the open bugs, and the lines not walked yet.
- **Kernel panic, 2026-10-30 about 19:29, on 7.2.13, no dock attached:** "Fatal exception in interrupt" on the drm_panic screen. A photo of the QR
  decodes locally (`zxing-cpp`; never open its link), but it ends at the journal's last kernel line, 14 minutes earlier: no Oops, RIP or trace, so the
  cause is unknown. Nothing implicates amdgpu or a test. It weakens dock and evdi as the only cause, and doesn't show the silent hangs are the same fault.
  Next, each the maintainer's to decide: `efi_pstore.pstore_disable=0` as a kernel argument, a full memtest86+ pass, a BIOS newer than 1.06, kdump if pstore comes back empty too.
- **Bad RAM, the maintainer's finding of 2026-10-30, about 21:30.** It fits the panic with no crash report, and supersedes the hang's earlier ranking below. Not yet shown: that the hangs stop. Run normally, then with the dock, before calling them solved. Crashes seen on this RAM may not repeat, so a fault that came once and never reproduced proves little; fixes made from a reproduced fault stand. Crash saving is active since 21:27 (`pstore_disable` N, backend efi, nothing saved so far).
- **Crash capture, staged 2026-10-30, active since the 21:27 boot:** `efi_pstore.pstore_disable=0` on the next boot, the maintainer's approval, staged by another real-machine session, which also pinned the older deployment (`latest-44.20261027.1`) so it isn't removed. Undo later: `rpm-ostree kargs --delete=efi_pstore.pstore_disable=0`, `ostree admin pin --unpin N`. Order: memtest86+, then normal running, then BIOS; none started. After the restart check `/sys/module/efi_pstore/parameters/pstore_disable` (N) and `/sys/module/pstore/parameters/backend` (efi).
- **To put back at the end:** the walk turned `amethystora-transitions` off at 18:39 on 2026-10-30, since its reveal bug (fixed on main, not in this image) sticks the picture: turn it on again once a fixed image or the overlay is loaded. No reveal is tested meanwhile, which may be why no stuck picture was seen since. The panic's boot had it off too.
- **Every live test goes through `tests/real/ledger.sh`, one at a time** (the maintainer's request of 2026-10-30, who suspects a
  test is behind the freezes). After a hang, run `tests/real/ledger.sh hang` first: the test left open is the suspect, and
  `~/.local/state/amethystora/walk/{ledger,beats}.tsv` hold it, `tests/real/LEDGER.tsv` the pushed copy. The maintainer asked to speed up on 2026-10-30, so the terminal track runs beside
  the walker again, with its own `ledger-terminal.tsv` (`WALK_TRACK=terminal`); `ledger.sh hang` shows both. At 15:32 both ran too, with no record.
- **Paused by the maintainer, 2026-10-30 20:35, every track stopped, nothing open in any ledger.** On resuming, the walker's order: A-NOTES-64 again (a task due today; wait 10 s into the input session before clicking the clock, or Quiet while sharing takes the click); A-NOTES-18, 19, 53, 54, 56, then 55, 28, 57 (open at the panic, neither implicated nor cleared); A-LOGS 07, 10 to 17, 21; A-MANUAL's keys and pointer items, A-MANUAL-13's last step again; A-SHELF 01, 02, 03, 05 to 08, 13; A-BACKUPS-23; A-CONTROL 03, 04, 08, 160. Only one track that opens windows at a time: a new window takes the focus even without a token, and a walker's Return reached another track's window at 20:06.
- Waiting on main, not in this image, not seen running: the shell fixes (`fa4ada5`, `444ee2d`, `01cc56c`, `101053f`, `6e291dc`, `0be3d10`, `2380bc9`, `5ea768d`, `dc74782`, `a2fe1e9`), tile corners `34b7071`, Glance `377cf9a` (Left and the arrows not fixed), `f1f5c19`, and the apps' `ef83fa4`, `1e3b8e1`, `f01f98b`. The restart cleared the overlay: load after any restart (`sudo tests/real/load-shell.sh`, log out, apply the theme again), then check these first.
- Open for the maintainer: firewall zone and remote desktop findings, the Dell dock test, A-UPDATES-31 (links beside the wordmark, the old look had them under: decide once it can be seen), the scratch-home marks of A-BACKUPS, A-UPDATES and A-SECURITY (kept, each note says what it covers). Heartbeat still running; journald drop-in still in `/etc`.
- Track 2 (terminal), 18:40 EDT: this round through (read-only items of `30-security.md` and `40-system.md`); what is left needs root, a terminal, or the walker. Nothing was set off, and nothing is left changed.
- Updated: 2026-10-30 15:08 EDT. Boot e04b5405, started 15:01:44, after the hard hang at 14:57. Image
  `latest-44.20261030`, built from 587de45, without today's fixes. DisplayLink is running again (its mask ended with the
  restart), `kernel.sysrq` is 16, and the journald drop-in is still in `/etc`, so sync stays 1 s.
- The first 40 items of `10-desktop.md` from D-GRID-05 are walked (last marks fa85ddd, 14:49). Nothing is walking now. Next:
  the items the walk left for a mouse or a person, then the rest of `10-desktop.md`, then 20, 30, 40. Left held: D-MENU-01, 02.
- Driver: `$D click` never landed (RemoteDesktop); a large relative move stops at the current screen's edge, so home through the join.
- One Terminal launcher, the maintainer's choice of 2026-10-30, is committed (6e8e0dd) and on this machine through the overlay until the next restart. Files' Open in Console still opens Ptyxis while a Ptyxis is running, since it then holds the bus name; a click on it has not been seen reaching kitty.
- Journal tails of each hang are kept on the machine, in `~/.local/state/amethystora/hangs`, not here: this repository is
  public and they hold personal data.
- Owner: the real-machine Claude session, since the lab session handed this over on 2026-10-30. Nobody takes commits
  through the lab meanwhile, and nothing here is validated by it. Never start `Latest Images` from here.

## The hang

Seen:
- Abrupt ends of today's boots, by journal: 09:45:46, 10:21:20, 12:13:17, 13:05:32. Uptimes 35, 35, 110 and 51 minutes.
- 15:32:16: boot e04b5405 ended the same way, 31 minutes in: no kernel error, no core dump. Boot 917847cd ended at 15:38:26, four minutes in, the maintainer saying "it crashed again" at 15:38:01, but its journal shows an orderly shutdown, so that one was a restart after whatever they saw, not a cut-off. Tails in `~/.local/state/amethystora/hangs`.
- 12:13: the maintainer forced the restart, all 4 screens black, 30 s after a resume, with `Failed to get ACT` and
  `Bad EDID` before it. That login turned the two DisplayLink LG screens on; the healthy logins at 12:15 and 12:30 did not.
- 13:05: all screens black, keyboard dead, `Ctrl+Alt+F3` nothing, direct monitor and keyboard no help, forced power-off,
  next boot 13:59:21. In the maintainer's words: "i started the tests, then i went for a break, got back and it was black".
- No kernel error, GPU reset, lockup, oops or core dump in the kept log of the 13:05 boot. `/var/lib/systemd/pstore` empty.
  `/sys/fs/pstore` needs sudo and has not been read.

- 14:37: journal ends 14:37:33.02 with sync at 1 s, so this tail is firm. DisplayLink's service had been stopped since
  14:06:29, `evdi` loaded. In the maintainer's words: "it crashed again afgter i plugged the dock back again. So i have a
  multiswitch kvm, when i switched back to this machine. it froze". SysRq (`kernel.sysrq=1`) did nothing; a keyboard plugged
  straight into the machine stayed off. No kernel line after 14:25:29 (the Lenovo dock's USB disconnect, which it
  survived for 12 minutes), so no USB connect line for the replug. No oops, no core dump, `pstore` folder empty. The walk
  was sending keys and screenshots at that moment.

Inferred:
- The kernel itself stopped: SysRq is handled in the kernel, and a powerless keyboard suggests the USB controller went too.
  Not GNOME Shell, and not a daemon. DisplayLink's service is not required for it, since it was stopped; its driver was loaded.
- The KVM, by the maintainer: switched before the freezes at 09:45, 10:21, 12:13 and 14:37, and NOT at 13:05, when it was
  already on this machine and nobody changed it. So a hotplug is a likely trigger, and not a necessary one.
- 14:57, the first with its trigger in the log: the maintainer unplugged the Lenovo dock. Seen: 14:57:34.19 USB disconnects,
  14:57:34.20 evdi `Disconnected from Task ... DisplayLinkMana` and `Removing i2c adapter`, 14:57:34.73 displaylink.service
  stopped, 14:57:37.60 GNOME Shell `Spurious clutter_actor_allocate` for the dock, 14:57:37.64 evdi `Closed by (sd-close)`,
  then nothing. Forced restart, next boot 15:01:44. No oops, no core dump. Sync was 1 s, so this tail is firm.
- Not every unplug or teardown hangs on 7.2.13: the same evdi teardown was survived at 09:12:47, 09:58:21, 10:59:00 and
  14:06:27 (the last by stopping the service), and the 14:25 unplug with the service stopped logged nothing of evdi. The last
  90 s of the other five hangs have no USB disconnect and no evdi teardown. Mid-session teardowns on 7.2.10 and 7.2.11 were
  survived too, so this alone does not separate the kernels.
- Inferred: GNOME Shell's change of the screens, about 3 s after the unplug, comes right before the end. Not shown.
- 15:32, in the maintainer's words: "while you were testing the desktop froze for minutes, nothing responded, that forced me
  to unplug the dell, which had the other screen on it", and the unplug "shutdown" the machine although it was on its power
  supply, until the power was pulled. Seen: the journal ends 15:32:16 at 1 s sync, the walk's last screenshot 15:31:56, no USB
  disconnect logged for the Dell. Inferred: nothing was logging by the unplug, so the machine was already stopped and the
  unplug did not cause the freeze; no hotplug came before it.
- AMD's reset reason (`x86/amd: Previous system reset reason`), read at boot: printed for the clean restarts, and absent at
  each of the seven boots after a hang since 09:46. Meaning not settled.
- A second kind of freeze, not the hard hang: the stuck picture. On an image without da7bf83, `this._reveal is not a function`
  leaves GNOME's frozen `screenTransition` actor up (clock and dock stale, menus still draw, shell answers, journal goes on).
  Seen at 16:36:28, cleared with `gnome-extensions disable` and `enable` of the transitions extension. Also in the journal at
  09:45:41 (5 lines, 5 s before that boot's end) and to 09:50:12. The heartbeat does not catch this form. Inferred: at 16:36:26
  every settings reader woke as `dnf` finished in the walk's toolbox, whose scriptlets reach the host's buses; not proven.
- Studied 2026-10-30, 18:30: `clocksource: Watchdog remote CPU N read timed out` is not the cause (a 50 us limit, shorter than a C3
  wake-up; `clocksource.c` identical in 7.2.10, 7.2.11, 7.2.13; on clean boots too; TSC never marked unstable). Ranking: the dock's display
  path with something new, first; amdgpu on it; 7.2.13 open; evdi lowered (e04b5405 and 917847cd stopped without it); firmware or power
  possible but not testable from logs; tests alone unlikely. Every hung boot had the dock hub; the current boot is the first of 28 without
  it, over 3 h, which also changed screens, MST, DSC and evdi at once. Older kernels have only 279 awake minutes, longest 57.
- 917847cd (15:38): the maintainer cut the power, so it is not a hang: seven abrupt ends, not eight. Empty pstore, no
  hung_task lines, no AER and no BERT are no evidence: this kernel and firmware have none of them on.
- At the next hang: ping and SSH from another machine, the wired NIC's link light, whether a USB device gets power, a short press of the
  power button, then a 4 s hold, and not the cord unless that fails. Then read the next boot's reset reason.
- The maintainer's choices, 18:40: the Dell dock's test later, on their word; the shell fixes' install block and logout after the walker's current stretch.
- Leading idea: kernel 7.2.13, or something else in the image of 2026-10-30. Seen: all 19 boots from 2026-10-15 to 10-27
  (kernels 7.2.5 to 7.2.11) ended with a clean shutdown, several through MST link drops, `Failed to get ACT` and page flip
  failures (boots of 10-23, 10-26, 10-27). Of the boots on 7.2.13, one ended clean (the first, 5 minutes) and five hung.
  The update's package list has the kernel and its `kmod-evdi` (1.15.1 both sides), and no Mesa, firmware or mutter. The
  `dcn31_hubbub.c:151` warning is not new: it was on 7.2.10 too, in a boot that ended clean. Usage also changed today (the
  walk), so this is a correlation.
- Next test, the maintainer's decision: `sudo rpm-ostree rollback` and a restart (image of 10-27, kernel 7.2.11), then the
  same use with KVM switches, for longer than today's longest boot (110 minutes). The walk waits meanwhile: that image has
  none of the last days' work. Earlier amdgpu display warnings are in the same area (10:18 DSC, 12:13 ACT, 14:08 REG_WAIT).
- The whole machine rather than GNOME Shell, since the console switch failed.
- The journal then synced only every 5 minutes, so up to 5 minutes before each end may be missing: the last lines
  (a graphical program starting, in three of the four) are not firm evidence.
- Not explained: the cause. Candidates are amdgpu on the dock's MST link, and evdi (DisplayLink, out of tree).

## The no-DisplayLink test

- Started 14:06:29: `displaylink.service` masked with `--runtime` and stopped, `kernel.sysrq=1`,
  `/etc/systemd/journald.conf.d/hang-test.conf` with `SyncIntervalSec=1s`. `evdi` could not be unloaded (still loaded,
  use count 2): that needs a logout. The LG screens are off, the Samsungs are `DP-1` and `DP-11`.
- Not input-free: the walk was resumed at the maintainer's word at about 14:12.
- Ended by the 14:37 hang, 31 minutes in. Result: it hangs without DisplayLink's service. `evdi` stayed loaded, so the driver
  is not ruled out.
- Seen at 14:08:02, on a switch to the text console, recovered: amdgpu `REG_WAIT timeout ... dcn31_program_compbuf_size`,
  a WARNING at `dcn31_hubbub.c:151`, under `fbcon_init`.
- Next, the maintainer's hands: switch the KVM away and back with nothing walking, to see if that alone hangs it; then the same with the Samsungs plugged directly into the machine.
- Take away afterwards: `sudo rm /etc/systemd/journald.conf.d/hang-test.conf`, `sudo systemctl restart systemd-journald`.
  The mask and `sysrq` end at the next restart.

## To check on this machine once an image has them

Needs an image past 587de45, or `/usr` overlay and a logout (the overlay is lost at each restart).

- Glance, b2446bd: Space stays up, Down steps, Space closes with the search kept (D-SEARCH-20).
- Private mode's keyboard, aa1d7bd (D-CLIP-03; the walk leaves this item open).
- The login reveal, workspace click and rename (D-TOPBAR-04, 06), Today headings, Playing now heading, Manual cards and
  section links.
- The wheel, 01cdf9b: needs the maintainer's real mouse (D-TOPBAR-07).
- `ame desktop daylight on`, when the image has 3886e3d.
- The search crash fix, 376d1b0, was seen working here at 12:34 through an overlay that a restart then discarded.

- The dock's short pill: fixed on this machine through the overlay until the next restart (see the commit), not in any image.

## Open, not fixed

- 16:36:28, a stuck picture with the shell alive: `this._reveal is not a function` in the transitions (16:36:28, then the 5 s cap at 16:36:32), so GNOME's frozen `screenTransition` actor stayed up. Seen: bar's clock still 16:36 at 16:40, dock frozen short, menus and the glow drawn over it, heartbeat ok. This image's `_reveal` field shadows the method after the login reveal ends; da7bf83 renamed it, not in the image. Also 8 errors in the 09:45 boot (09:47 to 09:50), and in no other of the last 8 boots. So a desktop that looks frozen with a live shell is one known form; not the hard hangs, whose journal ends. What changed a watched key at 16:36:28 not found; the walk was not running then.
- While the screen is shared or recorded, no drop in the shell lands: `modules/pill/glow.js` adds a full-screen non-reactive actor, and DND picks with `PickMode.ALL`, which sees it. Seen 16:06 to 16:09 on injected drags under the RemoteDesktop session, gone with `pill glow` off (D-TOPBAR-08, 09: the items passed only with the glow off). Not fixed: a shell file needs a logout to test.
- 15:28:32: `run-user-1000-doc.mount` deactivated with `xdg-document-portal` still running, so every Flatpak launched after it died at `bwrap: Can't find source path /run/user/1000/doc/by-app/...` with no window. `systemctl --user restart xdg-document-portal` brought it back. Who or what unmounted it not found; two other sessions were running.
- D-TODAY-02, 16:33:02: Notes aborted (Rust panic, `panic = "abort"`) on `amethystora-notes tasks` reaching the running instance, from a click on a task row in the calendar, with an input session live and the window under the share guard. Second instance's command line, GApplication's `command-line` signal in the stack. Tasks were written into scratch data by hand for the test, which the Tasks page showed. Not repeated, cause not found.
- D-TOPBAR-24: no busy chip in 170 s of full load in a terminal under an app scope. D-TOPBAR-25's no chip therefore shows nothing, and it stays open.
- D-GRID-02: a **No results** label stays over the app grid. Cause not found; not checked on a fresh login.
- D-SEARCH-21: with Glance off, Space on a file row closes the box and opens nothing.
- `meta_window_set_stack_position_no_sync: assertion` about once a minute in the shell's log. Unexplained.
- `gsd-media-keys` segfaults at session changes, in `_pa_context_get_card_info_by_index_cb`.
- 14:37:10 to 14:37:29: a kitty launched every 4.6 s through the menu, with two screenshots a cycle, until the hang. The walk's history has no such loop. Unexplained.

## Decisions for the maintainer

- Extension Manager (D-APP-27).
- Space on the Ask the agent row: it starts the agent, against D-SEARCH-18's "nothing starts before Enter".
- The identical HDMI output names.
- A patched mutter for GNOME/mutter#5136.
- Whether `Latest Images` is queued for today's fixes.
- A git identity for this machine (commits pass the author inline meanwhile).

## Machine state to put back

- Suspend and blanking are off at the maintainer's request (old values in
  `~/.local/state/amethystora/idle-settings-before-2026-10-30.txt`), and Daylight is off. Put back when told.
