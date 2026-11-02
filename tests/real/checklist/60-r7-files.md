# Round 7: files (Earlier versions, Anything opens, Uninstall means gone, Never out of room, Space to look)

Walk on a real machine running an image with these changes, as a person would. The spec is `plans/ui-ideas/r7/spec-files.md`. Each
switch has a line with it off: with its key off, what it wraps is as it was. Put each switch back with the command in the line.
Mark each line as the other files do: `→ PASS`, `→ FAIL: what was seen`. Helpers and what they write are tested in
`build_files/shared/test-files-group.sh`; these lines are what only a session shows.

## Earlier versions (R7-33)

- [ ] R7-FILES-01 The entry. How: in Files, right-click a file in your home. Expect: **Scripts › Earlier versions**, in the session's language. [needs: an account that has logged in on this image]
- [ ] R7-FILES-02 A file's versions. How: change a text file, wait for an hourly snapshot, change it again, then **Scripts › Earlier versions**. Expect: Backups opens on **Versions of NAME**, each with when it was made, on this machine; within a moment of the click; a backup's versions come in after them, with **Looking in your backup…** meanwhile. [needs: hourly snapshots on]
- [ ] R7-FILES-03 Look. How: **Look** on a version. Expect: it opens read-only in its app, with Backups still usable beside it.
- [ ] R7-FILES-04 Keep this one too. How: **Keep this one too** on a version that differs. Expect: `NAME (DATE TIME).ext` appears beside the file; the file itself is unchanged (compare with `sha256sum`); the banner says the earlier version is beside the newer one. Again on the same version: nothing needed.
- [ ] R7-FILES-05 From the backup. How: the same two on a version from the backup. Expect: the same, with **Getting it from the backup…** while it fetches. [needs: a backup set up]
- [ ] R7-FILES-06 It says when they are off. How: with snapshots off, and with no backup, open Earlier versions on a file. Expect: a sentence says which of the two is off; with neither set up, Backups' own page says how to set one up. Nothing is promised that is not kept.
- [ ] R7-FILES-07 A folder, and outside the home. How: **Earlier versions** on a folder; on a file in `/tmp` or on a stick. Expect: the folder opens its timeline as Back in time does; outside the home a notice says it covers the home folder.
- [ ] R7-FILES-08 Backups already open. How: leave Backups open on another folder, then **Earlier versions** on a file. Expect: the same window comes up on that file's versions.
- [ ] R7-FILES-09 Off. How: `ame desktop earlier-versions off`. Expect: the entry leaves Scripts; a script of your own in that folder stays; Back in time and Backups' own **Versions** work as before. Backups' **Keep this one too** stays, since it only adds a file: that one addition is not behind the key. Back with `ame desktop earlier-versions on`.
- [ ] R7-FILES-10 Nothing kept. How: name a file `canary-r7-33.txt`, look at its versions, close Backups, then `grep -rl canary-r7-33 ~/.local ~/.config ~/.cache /tmp "$XDG_RUNTIME_DIR" 2>/dev/null; journalctl --user -b | grep canary-r7-33`. Expect: nothing but Files' own list (`~/.local/share/recently-used.xbel`), and nothing in `~/.cache` from a Look at a snapshot.
- [ ] R7-FILES-10b Look's copy goes by itself. How: **Look** at a version from a **backup**, close the viewer, `find ~/.cache/amethystora-restore/look`, then, a day and a half later with the machine on, again; `systemctl --user list-timers | grep tmpfiles`. Expect: the copy is there at first, read-only, and gone the second time, with no other Look in between; the timer is listed. With hourly snapshots on, those made meanwhile hold it until they expire, as the manual says. [needs: a backup set up]

## Anything opens (R7-40)

- [ ] R7-FILES-11 A file nothing opens. How: open a `.dwg` or a `.sqlite` file in Files. Expect: at once, a card with the file's name and, in a moment, what it is in plain words; **Nothing here opens this yet** and up to three apps from Flathub, each with its size and **Install and open**; a line that the file stays on this computer. GNOME's "no app" message does not show.
- [ ] R7-FILES-12 Nothing leaves. How: watch `sudo tcpdump -n -i any 'not port 22'` (or a second terminal's `ss`) while the card fills in. Expect: no connection until you press a button.
- [ ] R7-FILES-13 Install and open. How: **Install and open** on a small app. Expect: **Installing…** with the polkit question only where Flatpak asks one; the card closes and the app opens the file. Closing the card while it installs: it still opens. [needs: network]
- [ ] R7-FILES-14 Afterwards. How: open another file of that kind. Expect: the app opens it, no card: `gio mime TYPE` names the app first.
- [ ] R7-FILES-15 What is left as it was. How: open a `.txt`, a `.png`, a `.pdf`, a `.zip`, a folder, a `.exe`, a `.rpm`. Expect: each opens as before; the card shows for none. **Open With** in Files never lists the card's launcher.
- [ ] R7-FILES-16 A file that says nothing. How: `head -c 3000 /dev/urandom > noise.qqq`, open it. Expect: the card says the file does not say what kind it is, and points to Open With; no app is offered.
- [ ] R7-FILES-17 No list yet. How: `sudo mv /var/lib/flatpak/appstream/flathub /var/lib/flatpak/appstream/flathub.away`, open such a file, then move it back. Expect: the card says the list is not on this computer yet.
- [ ] R7-FILES-18 Off. How: `ame desktop open-anything off`, open the `.dwg` again. Expect: GNOME's own message; the security report and the watcher say nothing about `~/.local/share/applications/zz-amethystora-open-anything.desktop`. Back with `ame desktop open-anything on`.
- [ ] R7-FILES-19 Nothing kept. How: open `canary-r7-40.dwg`, close the card, then `grep -rl canary-r7-40 ~/.local ~/.config ~/.cache /tmp "$XDG_RUNTIME_DIR" 2>/dev/null; journalctl --user -b | grep canary-r7-40`. Expect: nothing (Files' own Recent list is Files'). Again after **Open** in an app: `~/.local/share/flatpak/db` holds no such name.
- [ ] R7-FILES-19b An app Flathub has not checked. How: a kind only an unverified app claims. Expect: the row says **Flathub has not checked who made this.** before any click; a verified one says **Flathub checked who made this.**; the words never say *free*.
- [ ] R7-FILES-19c A password for Install and open. How: `ame security flatpak-password on` (and again as an account outside `wheel`), **Install and open** on a small app. Expect: GNOME's password question comes up; given, the app installs and opens the file; cancelled, a notice says it could not be installed, and the card offers the button again. Back with `ame security flatpak-password off`. [needs: network]
- [ ] R7-FILES-19d An app added beside the launcher. How: layer a package claiming a kind of the list, for example `rpm-ostree install gimp`; restart; open a `.tiff`. Expect: that app opens it, no card; `gio mime image/tiff` names it, not `zz-amethystora-open-anything.desktop`. [changes setting: `rpm-ostree uninstall gimp`]
- [ ] R7-FILES-19e Key and launcher agree. How: from another console with the screen locked, `gsettings set org.gnome.shell.extensions.amethystora-ui.tools open-anything false`; unlock. Expect: at once `~/.local/share/applications/zz-amethystora-open-anything.desktop` exists and the card does not come; `gdbus introspect --session -d org.gnome.Shell -o /org/amethystora/Tools` lists no `Opener`. Back with `ame desktop open-anything on`: the file is gone and `Opener` listed.

## Uninstall means gone (R7-41)

- [ ] R7-FILES-20 The question. How: right-click a Flatpak app you can spare in the app grid, **Uninstall**. Expect: **Remove NAME?** at once: **The app** with when it was last opened, **Its settings and data** with its size, on; **Documents saved outside its own folder stay. You have 30 days to change your mind.**; **Keep** and **Remove**.
- [ ] R7-FILES-20b Keep. How: **Keep** in that question. Expect: the app, its folder and the trash are as they were.
- [ ] R7-FILES-20c The tick stays yours. How: on an app with a large folder, untick **Its settings and data** while it still says **Looking…**. Expect: when the size arrives the tick stays off; **Remove** keeps the folder. An app with no folder says **It has none**, and its notice says nothing about data.
- [ ] R7-FILES-20d Other trash settings. How: in Settings, Privacy, set trash to 90 days, and then turn it off; **Uninstall**. Expect: **You have 90 days**, then **Its data stays in the trash until you empty it**.
- [ ] R7-FILES-21 Remove. How: **Remove**. Expect: its icon greys with a ring; then **NAME is removed** with **Undo**. `~/.var/app/ID` is in the trash (Files' Trash shows it). A document in `~/Documents` that the app made is where it was.
- [ ] R7-FILES-22 Undo. How: **Undo** on the notice. Expect: the app installs again and opens with its settings as they were; an app that was the account's comes back the account's (`flatpak list --user`). Removed with the data kept, then Undo, with an older removal still in the trash: **NAME is back**, what is in the trash stays there, and the notice says so. [needs: network]
- [ ] R7-FILES-22b A password for Uninstall. How: `ame security flatpak-password on` (and an account outside `wheel`), **Uninstall** a machine-wide app, **Remove**. Expect: GNOME's password question; given, it goes; cancelled, **could not be uninstalled**, and the folder is still there. Back with `ame security flatpak-password off`.
- [ ] R7-FILES-23 Keep the data. How: untick **Its settings and data**, **Remove**. Expect: the app goes, `~/.var/app/ID` stays.
- [ ] R7-FILES-24 Undo later. How: remove one, restart, `ame apps undo-uninstall ID`. Expect: the same as Undo.
- [ ] R7-FILES-25 Not opened for months. How: **Apps you have not opened for months** in Control, or `ame apps unused`. Expect: apps with launchers not used for 90 days, with what each keeps; none installed in the last 90 days; picking one removes it the same way.
- [ ] R7-FILES-26 Off. How: `ame desktop uninstall-data off`, **Uninstall**. Expect: the question as it was, **Keep its data** or **Delete its data**. Back with `ame desktop uninstall-data on`.
- [ ] R7-FILES-27 Installed twice. How: an app in both the machine's and the account's Flatpak. Expect: the note to use Bazaar, as before, with the key on and off. [needs: such an app]

## Never out of room (R7-58)

- [ ] R7-FILES-28 When it fires. How: fill the disk until under GNOME's threshold (under 5% and 2 GB free; on a spare machine, `fallocate -l SIZE ~/fill`), wait a minute; leave some unused runtimes and old trash. Expect: **Nearly out of room**, how much is left, how much can be cleared, none of it your work, with **Show me first** and **Clear N GB**, in place of GNOME's notice. [needs: a machine to fill]
- [ ] R7-FILES-29 Show me first. How: **Show me first**. Expect: **Old app versions and caches** and **The trash**, older than the days set in Settings, with sizes; a line for newer trash, and for snapshots when they are on; **Choose each myself** opens Control's Free disk space.
- [ ] R7-FILES-30 Clear. How: **Clear N GB**. Expect: **Clearing space…**, then **Cleared N** and what is free now. Your documents, the trash's last 30 days, named images and volumes are as they were (`podman image ls`, `podman volume ls`, Files' Trash). With snapshots on it says what comes free later.
- [ ] R7-FILES-30b The trash as you set it. How: Settings, Privacy: trash to 90 days, with something trashed 40 days ago (edit its `DeletionDate` in `~/.local/share/Trash/info`); **Show me first**. Expect: it is not counted, and stays after **Clear**. With removing old trash off, nothing in the trash counts, and the card says it stays until you empty it. Back to 30 days and on.
- [ ] R7-FILES-30c Across a lock. How: lock when the offer has come, unlock. Expect: the offer is still there, one only, and its buttons work; a later notice replaces it, and after **Clear** none is left.
- [ ] R7-FILES-30d A disk of its own. How: with a USB stick under GNOME's threshold and the home low too. Expect: GNOME's notice naming the stick stays, not replaced by the offer. [needs: a stick to fill]
- [ ] R7-FILES-30e Honest numbers. How: `df -B1 --output=avail /var/home` before and after **Clear**, on a machine with hourly snapshots off. Expect: **Cleared N** is what `df` gained, not twice it.
- [ ] R7-FILES-30f A shared machine. How: with a second account, and a runtime no app of yours uses. Expect: **Clear** does not take that runtime (`flatpak list --runtime`); `ame system clean` still asks about it.
- [ ] R7-FILES-30g A password for runtimes. How: `ame security flatpak-password on`, a machine with one account and an unused runtime, **Clear**. Expect: GNOME's password question for the runtime; cancelled, the rest is still cleared. Back with `ame security flatpak-password off`.
- [ ] R7-FILES-30h What a check sends. How: `sudo tcpdump -n -i any 'not port 22'` while `/usr/libexec/amethystora-free-space check --json` runs, with Homebrew installed. Expect: no connection. **Clear** may then reach Homebrew's list of formulae.
- [ ] R7-FILES-31 Nothing to clear. How: the same with nothing that counts. Expect: GNOME's own notice, as it was.
- [ ] R7-FILES-32 It costs nothing. How: `ps -C amethystora-free-space` over a few idle minutes with plenty of room. Expect: never there; no timer of its own (`systemctl --user list-timers | grep -i space`: nothing).
- [ ] R7-FILES-33 In a terminal. How: `ame system free-space`. Expect: the two totals, a question, then what was cleared; Control's **Free disk space** (`ame system clean`) is as before.
- [ ] R7-FILES-34 Off. How: `ame desktop low-space off`, then as R7-FILES-28. Expect: GNOME's notice only. Back with `ame desktop low-space on`.

## Space to look (R7-29)

- [ ] R7-FILES-35 Space. How: in Files, select a picture, a PDF, a text file and a video in turn, press `Space`. Expect: GNOME's previewer shows each; `Space` or `Esc` closes it. It is not tiled. [needs: the first online boot done, so that `org.gnome.NautilusPreviewer` is installed]
- [ ] R7-FILES-36 Arrows and open. How: with a preview up, the arrow keys, then its open button. Expect: the arrows walk the folder; the button opens the file in its app and the preview closes.
- [ ] R7-FILES-37 The private folder. How: open `~/Private` (`ame security private-folder`), `Space` on a file there, close the folder. Expect: it previews while open; afterwards nothing of it is left in `~/.cache` or `~/.var/app/org.gnome.NautilusPreviewer` (`grep -rl NAME`). [needs: the private folder set up]
- [ ] R7-FILES-37b The private folder while sharing the screen. How: share the whole screen, in `~/Private` with Files covered by the share guard, `Space` on a file. Expect: note what is seen: the previewer's window is not one of the guard's, so the preview shows uncovered. This is how it has always been and this round does not change it; tell the maintainer if it matters. [needs: the private folder set up, and something sharing the screen]
- [ ] R7-FILES-38 A USB stick. How: `Space` on a file on a stick. Expect: as the manual says, it does not preview (the previewer reads the home only), and nothing is left waiting. [needs: a stick]
- [ ] R7-FILES-39 It costs nothing. How: close the preview, `flatpak ps`. Expect: within a minute, no previewer running.

## Blast radius

- [ ] R7-FILES-40 No faults of ours. How: use each of the above, lock and unlock, then `journalctl --user -b -o cat _COMM=gnome-shell | grep -E "JS ERROR|already disposed|tools/opener|tools/room|bazaar"`. Expect: nothing naming these.
