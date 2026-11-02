# The disk is full

## Symptom
"No space left on device"; an update failed because the disk is full; apps cannot save; files deleted
to make room give no space back; or `df` shows free space while writes still fail.

## Look
- `state overview` and `state disks`: free space on `/`, `/var` and `/var/home`.
- `state backups`: whether the hourly snapshots are on, and how many are kept.
- `state updates`, and `logs` with unit `uupd.service`, grep `space`, when an update failed.
- `df -h /var /var/home`, `sudo btrfs filesystem usage /var` (root), `rpm-ostree status`,
  `flatpak list --columns=application,size`, `podman system df`, `journalctl --disk-usage`,
  `coredumpctl list`, `du -xh --max-depth=1 ~ | sort -h | tail -15`.

## Causes on Amethystora
1. **The hourly snapshots hold what was deleted.** With ransomware protection on, every home folder is
   snapshotted into `/var/home/.snapshots` each hour; every snapshot of the last day is kept, and one a
   day for two weeks. A deleted or rewritten file keeps its space until the last snapshot holding it
   goes. Nothing deletes snapshots to make room, on purpose: ransomware that rewrites every file would
   then delete the ones from before it. Tell: `state backups` says on, and deleting gave nothing back.
2. **Deployments.** The running system, the one to roll back to, a staged update and any pinned one each
   keep their files; switching images (to developer mode, say) shares little. Tell: `rpm-ostree status`
   lists more than two, or one marked pinned.
3. **Unused Flatpak runtimes and app data.** Tell: `flatpak uninstall --unused` would remove some;
   `du -sh ~/.var/app/*` for each app's own data.
4. **Containers.** Podman images and volumes, the `amepkg` containers, Docker on the developer image,
   virtual machine disks, Waydroid. Tell: `podman system df`, `amepkg containers list`.
5. **Logs and crash dumps.** Tell: `journalctl --disk-usage`, `coredumpctl list`.
6. **btrfs metadata full.** Data has room but metadata does not, so writes fail while `df` shows free
   space. Tell: `btrfs filesystem usage` shows Metadata near its size.
7. **The user's own files** (downloads, videos, games, the trash). Tell: the `du` list.

## Fix
### 1
Never delete a snapshot, and never run `btrfs subvolume delete`. The space comes back as snapshots age
out, within two weeks. Only the user may decide on `ame security ransomware off`, which stops the
snapshots and offers to delete them, giving up ransomware protection; never run it for them.

### 2, 3, 4
Control's **Free disk space** (entry `clean`, an action: hand `ame system clean` to the user with
`run`). It asks before pruning Podman's and Docker's unused images and volumes, then removes unused
Flatpak runtimes, rpm-ostree's cached base layers and metadata, and Homebrew's cache. `ame system free-space`
(what the low-space notice's Clear does) asks once for what needs no choosing: unused runtimes, Homebrew's old versions, unnamed
container layers, the caches of apps not running, thumbnails, and what the trash's own housekeeping (`org.gnome.desktop.privacy`) lets go, never under 30 days. Beyond them, each
asked first:
- The rollback deployment: `sudo rpm-ostree cleanup -r` (root); there is nothing to roll back to until
  the next update. A pinned one: `sudo ostree admin pin --unpin INDEX` (root), INDEX from
  `rpm-ostree status`.
- Never `podman system prune` or `docker system prune`: they delete stopped containers, which `amepkg`
  containers are between uses. Removing one is the user's choice (`amepkg containers rm NAME`).

### 5
`sudo journalctl --vacuum-size=500M` (root). What it removes is gone, the security watcher's history
of it too: say so first.

### 6
`sudo btrfs balance start -dusage=10 /var` (root): packs nearly empty data chunks so that metadata gets
room. It reads and writes for a while; raise to `-dusage=30` if little came back.

### 7
List the largest folders and let the user choose what goes. Deleting their files, or emptying the
trash, is theirs to do or to approve one by one.

## Verify
- `state disks` or `df -h /var /var/home` shows free space again; mind cause 1 if it did not move.
- `sudo btrfs filesystem usage /var` (root) shows metadata with room.
- A failed update: run `ame update`, then `state updates`.
- `state health` no longer points to this playbook.

## Undo
- Flatpak runtimes come back with the next install or update that needs them; Podman images are pulled
  again when used; the rollback deployment returns with the next update.
- A balance and a vacuum need no undo, and a vacuum cannot be undone.
- `ame security ransomware on` starts the snapshots again (root).
