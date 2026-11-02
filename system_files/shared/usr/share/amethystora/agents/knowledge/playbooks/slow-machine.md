# The machine is slow

## Symptom
The machine is very slow: windows lag, fans spin, the desktop stutters, apps take long to open, or it
slows down at certain times (an hour after start, at night, once a week).

## Look
- `state overview`: memory and swap, free disk space, failed units.
- `ame system start-times`: how long each start took by date (administrators only); `ame system start-times why` when it got slower: what changed.
- `state security-settings`: the detection profile and every switch, and the enforced ones changed.
- `state security`: the `stricter` check names stricter settings that are on.
- `state units` with a unit's name, and `logs` with unit `systemd-oomd.service` for programs killed for
  memory.
- `systemd-cgtop`, `ps -eo pid,comm,%cpu,%mem --sort=-%cpu | head -15`, `cat /proc/pressure/cpu
  /proc/pressure/memory /proc/pressure/io`, `systemctl list-timers`, `systemctl --user list-timers`,
  `free -h`, `zramctl`, `sensors`, `tuned-adm active`, `gnome-extensions list --enabled`.

## Causes on Amethystora
1. **What looks for new threats.** Off as the image ships; `ame security profile thorough` and single
   switches turn on parts that cost CPU or memory all the time: the behaviour sensor
   (`amethystora-behaviour.service`, Falco), network protection (`amethystora-ips.service`, Suricata),
   real-time scanning (`amethystora-clamav-onaccess.service`, which keeps clamd's signatures, over 1 GB,
   in memory). Tell: `systemd-cgtop` puts one on top; the profile is not `default`.
2. **The virus scanner.** clamd runs only while something scans: `clamd@scan.socket` starts it and it
   stops within half an hour. The weekly scan (`amethystora-clamav-scan.service`) reads every home
   folder, on mains power. Tell: clamd or clamscan on top.
3. **The audit log instead of the file-watch sensor.** Where the kernel runs `amethystora-sensor.service`,
   watching costs little; with `ame security audit-locked` on, or a kernel the sensor cannot load on,
   audit weighs every system call. Tell: the sensor inactive, `sudo auditctl -l` (root) lists many rules.
4. **Work in the background.** `uupd.service` (system, Flatpaks, Homebrew), the user's
   `amethystora-pkg-upgrade.service` (containers) and `amethystora-backup.service`,
   `amethystora-integrity.service` (its first check reads every app), `flatpak-preinstall.service` on a
   new machine. Tell: one of them active now.
5. **Memory.** zram swap fills and memory pressure rises; `amethystora-zram-recompress.timer` repacks
   hourly on mains power. Tell: `free -h`, `zramctl`, the memory pressure line.
6. **Stricter kernel arguments** (`init_on_free=1`, `iommu.strict=1`), turned on with
   `ame security kernel-args`. Tell: the `stricter` check.
7. **Heat or the power mode.** Tell: `sensors`, `journalctl -k -b -g 'throttl|temperature'`, Power
   Saver in the quick settings or `tuned-adm active`.
8. **A GNOME extension.** Tell: `logs` with identifier `gnome-shell`, grep `JS ERROR|Extension`.
9. **The CPU scheduler** for a busy desktop: heavy builds starve the desktop.
10. **A full disk** slows btrfs (`playbook/disk-full`); Waydroid (`waydroid-container.service`) holds
    memory while it runs.

## Fix
1. The user chooses: `ame security profile default` or `light` (root), or one switch alone, such as
   `ame security settings realtime-scan off`, `ame security settings behaviour off`,
   `ame security settings network off` (root). Each gives up the detection it names; the agent never
   runs these.
2. Let a scan finish. The user may choose `ame security settings virus-scan manual` (root), which
   scans only when asked.
3. The user may run `ame security audit-locked off` (root) (restart): the sensor again, which root can
   stop. A kernel that will not run the sensor: `ame report`.
4. Wait for it to finish; uupd itself skips a run while the machine is busy or on low battery.
5. `change` the Control entry `zram` to `fast` (no hourly repacking, more memory used) (root) (restart).
   Close what holds memory. Do not raise `vm.swappiness` or change `vm.page-cluster` or the watermarks
   for zram: measured on this image, swappiness 180 made the desktop slower and none of them helped.
6. The user may run `ame security kernel-args init_on_free=1 off` (or `iommu.strict=1`) (root)
   (restart).
7. Clean the vents; pick Balanced or Performance in the quick settings.
8. `gnome-extensions disable UUID`; an image extension comes back with the next update. For the tiling,
   `change` the Control entry `layout` to `classic`.
9. `change` the Control entry `scheduler` to `bpfland` (root); it falls back to the kernel's own if the
   kernel refuses it.
10. Stop Android: Control entry `android-run`, an action, so hand `ame apps android stop` to the user
    with `run` (root).

## Verify
- `systemd-cgtop` and the pressure files again; `state overview` for memory.
- `state security-settings` shows the profile chosen; `state settings` the scheduler and zram.
- `state health` no longer points to this playbook.

## Undo
- `ame security profile` with the profile it had, or each switch back with the same command (root).
- `ame security audit-locked on`, `ame security kernel-args init_on_free=1 on` (root) (restart).
- `ame system zram default` (root) (restart); `ame system scheduler kernel` (root).
- `gnome-extensions enable UUID`; `ame desktop layout tiling`.
