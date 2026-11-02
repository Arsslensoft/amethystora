# An update that fails

## Symptom

System Updates says the update failed or finished with errors, or sits on one step for hours; `ame update`
stops with an error; or nothing new arrives, and the security report says the image is weeks old or the
last update that finished was long ago.

## Look

- `state updates`: how `uupd.service` and `uupd-manual.service` last ended, `uupd.timer`, the deployments.
- `state overview`: layered packages, free space, failed units.
- `state security`: the checks `automatic-updates`, `image-age`, `single-build`, `signed-updates`.
- `logs` unit `uupd-manual.service` (or `uupd.service`), grep `module_fail|error|rejected|space`; a staged
  update lost at a restart: `logs` unit `ostree-finalize-staged.service`, boot `-1`.

Then:

- `rpm-ostree status` (idle or busy, staged, LayeredPackages, Pinned)
- `df -h / /var /boot`, `btrfs filesystem usage /` (root)
- `resolvectl query ghcr.io`, `timedatectl`
- A sealed image (`/usr/share/amethystora/sealed` exists) has no rpm-ostree:
  `/usr/libexec/amethystora-deployments`

## Causes on Amethystora

1. **Layered packages.** Packages from `rpm-ostree install` are resolved again on every update: slower,
   and it fails when the new base conflicts or a repository in `/etc/yum.repos.d` is unreachable. Tell:
   `state overview` lists them; the log says "conflicting requests" or names a repository.
2. **Only signed images.** Updates must carry Amethystora's signature (`/etc/containers/policy.json`). Tell:
   "Source image rejected" or "signature" in the log after a switch to another image, or `policy.json`
   differs from `/usr/etc/containers/policy.json`.
3. **Encrypted DNS or the clock.** With `ame security dns` on, a network that blocks port 853 resolves
   nothing ("lookup ghcr.io"); with `ame security time` on, one that blocks NTS leaves the clock off and
   TLS fails ("certificate"). Tell: `/usr/libexec/amethystora-dns status`, `timedatectl`.
4. **One build held.** A dated tag (`stable-44.20260922`) pulls the same build forever: the `single-build`
   check.
5. **Automatic updates off or held back.** `uupd.timer` disabled (`automatic-updates`), or every automatic
   run skipped: uupd waits on battery under 20%, load over 50 or memory over 90% (`/etc/uupd/config.json`).
   Update now skips those checks.
6. **A staged update not applied.** It takes over at a restart, finalized at shutdown. Tell:
   `rpm-ostree status` shows it staged; or the previous boot's `ostree-finalize-staged.service` failed,
   often with `/boot` full of pinned deployments.
7. **No space.** "No space left on device". A file deleted since the last hourly snapshot keeps its space
   for up to two weeks.
8. **Busy.** `rpm-ostree status` says busy; `ame update` refuses while `uupd.service` runs.
9. **Another part failed.** "Updates finished with errors!" with `module_fail` for Flatpak or Brew; the
   system's update is still staged. With `ame security brew-attestations` on, Homebrew fails without
   `gh auth login`; with `ame security flathub-verified` on, unverified apps stop updating. Containers
   report under Containers only: `logs` user_unit `amethystora-pkg-upgrade.service`.

## Fix

### Layered packages

Ask first. Move each package to Flatpak, Homebrew or a container, then `rpm-ostree uninstall PACKAGE`
(root) (restart), or `rpm-ostree reset` (root) (restart) for all.

### Signature

`change` `stream` (or `image`) opens `ame system rebase`, which takes signed images only. For the user's
own image, the user runs `ame system trust-image REPOSITORY KEY` (root) first. Never loosen `policy.json`.

### DNS or clock

Fix the connection (`playbook/wifi-dns`). The user runs `ame security dns pause` (root) or
`ame security time off` (root); never for them.

### One build held

`change` `stream` with the stream's name, or the command the `single-build` check prints.

### Automatic updates

`ame system auto-updates on` (root). Held back: plug in, or `systemctl start uupd-manual.service`, which
the person at the machine may start without a password.

### Staged update

Restart (restart). With `/boot` full, ask first: `ostree admin pin --unpin INDEX` (root) for a pinned
deployment, or `rpm-ostree cleanup -r` (root), which gives up the rollback.

### No space

`change` `clean` (`ame system clean`, which asks before each step), `flatpak uninstall --unused`. Never
delete a snapshot or anything from the backup.

### Busy

Wait for `uupd.service`. A transaction stuck for hours: `rpm-ostree cancel` (root).

### Another part

Homebrew: `gh auth login` as that account, or the user runs `ame security brew-attestations off` (root).
Flatpak: `flatpak update` in a terminal names the app at fault; `flatpak repair --user`.

## Verify

- `state updates`: `uupd-manual.service` ended in success, with a deployment staged.
- `state security`: no `automatic-updates`, `image-age` or `single-build` warning.
- `journalctl -u uupd-manual.service -g 'Updates Completed Successfully' -n 1`

## Undo

- A new image that misbehaves: the boot menu's second entry, then `bootc rollback` (root) (restart).
- A new image that did not start, with `ame update boot-guard` on (the owner's switch, off as shipped, never the agent's): the computer went back by itself, `/var/lib/amethystora/boot-guard/went-back.json` says from which, and the automatic update waits for a newer image.
- A staged update not wanted: `rpm-ostree cleanup -p` (root).
- Packages taken off: `rpm-ostree install PACKAGE` (root) (restart).
- A rebase: `ame system rebase` to the image and stream before.
- `ame system auto-updates off` (root); `ame security dns resume` (root) ends a pause early.
