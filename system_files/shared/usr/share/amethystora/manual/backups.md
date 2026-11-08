# Backups

A daily restic backup to a repository that ransomware on this machine cannot delete from, and **Backups**, the window
that puts files back from it or from the hourly snapshots.

```bash
ame backup
```

[Show me](guide:set-up-a-backup)

## Where the backup goes

The backup only ever adds to the repository and never deletes old snapshots, so an append-only repository at the other
end keeps everything backed up before an attack. `ame backup` explains how to set one up on rest-server, Borg, an
object store with immutability, or a USB drive you unplug between backups.

```bash
ame backup now        # back up straight away
ame backup snapshots  # every point in time to go back to, in the backup and in the hourly snapshots
ame backup on|off     # the daily run; off keeps the settings and the repository
ame backup setup      # change where and how
```

The hourly [snapshots](viruses.md#ransomware-protection) are switched in Security, which Backups only opens.

## Putting files back

**Backups**, in the app grid (or under **System** in the Amethystora menu), shows your home folder as it was, with every
point in time down its side: the hourly snapshots kept on this machine and the daily backups, each marked by where it
is. Pick a time, open a folder, select what you want and press **Put back**.

![Snapshots and backups on one timeline](media/backup-timeline.webm)

Nothing is deleted: a file your folder no longer has simply comes back, and one that changed since comes back beside it,
with its date in its name (`report (2026-10-04 14.00).pdf`), unless you choose **Replace** or **Skip** when it asks.

- Beside each file it says whether your folder no longer has it, has changed it since, or has it the same. **Show
  hidden files** shows the ones whose names start with a dot.
- **Put the whole folder back** returns a folder as it was, replacing what has the same name.
- **Back in time** shows the folder's past as windows receding, the earlier points behind it: scroll through them, with
  the wheel or Page Up and Page Down, and press **Put this back** to bring the folder there back beside what you have.
- `Alt+Up` or `Backspace` goes to the folder above, `Ctrl+A` chooses everything in it, and `Esc` closes a question or
  the versions.

## Versions and Look

**Versions** and **Look** are there when one item is chosen. **Versions** lists each time a file changed; each version
has **Go there**, which opens its folder at that time, and **Keep this one too**, which puts that version beside your
file with its date in its name and never touches the file.

**Look** opens a read-only copy. A file looked at from the backup is first copied into
`~/.cache/amethystora-restore/look`, read-only, and removed a day after it was last read, by your account's own daily
cleaning, so within about two days. With the hourly snapshots on, those made meanwhile hold that copy too, until they
expire within 14 days.

## Opening it on a folder

In Files, **Open With** on a folder offers Backups too.

```bash
amethystora-backups ~/Documents                       # open it on that folder
amethystora-backups --at=2026-10-04_1400 ~/Documents  # at that time: a snapshot's name or a backup's id, as `ame backup snapshots` lists them
amethystora-backups status                            # the Status page
```

In a script, `/usr/libexec/amethystora-restore` does what the window does, each with `--json`: `status`, `points`,
`ls`, `versions`, `put-back` (with `--keep-both`, `--replace` or `--skip`) and `open`.

## The Status page

It shows the last good backup and when the next one runs, with **Back up now**, **Change…** (`ame backup setup`, in a
terminal; **Set up a backup…** before there is one) and the daily run's switch. The security report warns when no
backup has succeeded for a week, and **Ask the agent** looks into one that failed.

## Thinning old snapshots

Nothing on this machine deletes from the repository, so old snapshots build up there. Thin them out from another
machine, with credentials this one does not hold, for example:

```bash
restic -r <repository> forget --keep-daily 14 --keep-weekly 8 --keep-monthly 12 --prune
```

## Déjà Dup

Déjà Dup is no longer installed. An account it backed up on a schedule is told so once; the backups it made stay where
they are, and Déjà Dup, from Bazaar, still reads them.

## Your setup, kept beside your files

Each backup also keeps your setup: the image and stream you follow, your Flatpak apps and where they came from, your
Homebrew packages, the containers `amepkg` made with what you installed in them, the GNOME extensions you had on and
your theme. No passwords, keys or tokens go in it. On a new machine, point `ame backup` at the same repository with the
same password, then:

```bash
ame restore-setup             # offers the newest few
ame restore-setup <snapshot>  # an older setup, by its id or by the id of a backup of your files
```

It shows what it would do first, and does only the parts you pick: switch to the same image, through
`ame system rebase`, which takes signed images only; install the apps and Homebrew packages; make the containers again
and install what was in them; turn the extensions back on; set the theme. Given a backup of your files, it takes the
setup saved next from that machine. Your files themselves come back in Backups. To keep your setup in a file of your
own as well, in git, see [Your setup in a file](setup.md).
