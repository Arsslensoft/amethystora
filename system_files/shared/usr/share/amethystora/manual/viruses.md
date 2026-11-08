# Viruses and ransomware

A virus scanner that only reports unless you tell it otherwise, and hourly snapshots of every home folder
that ransomware cannot write to.

## Scanning for viruses

**Virus scan** in the **Security** app (or `amethystora-security scan`) scans whenever you ask:

- **Your home folder**, apart from caches.
- **A file or folder**: choose it, or drop it anywhere on the window.
- **The whole machine**: every home and temporary folder, as the weekly scan does. It asks for no
  password and carries on if you close the window. What it finds is kept where only an administrator can
  read it, so seeing that does ask for your password.

A scan you start runs as you, so it checks what your account can read. While the virus scanner is off,
the page offers **Turn it on…** instead.

### What a scan finds

By default nothing a scan finds is deleted, quarantined or moved: the result lists each file, with
**Show in Files**, **Copy path** and **Scan again**, and leaves the decision to you. **Stop the scan**
stops one you started. A match can be a false positive, so check where a file came from before you delete
it, then scan again.

![A scan that quarantines](media/virus-scan-quarantine.webm)

If you set scans to quarantine or delete ([Settings](security-settings.md#what-happens-to-what-a-scan-finds)),
the app asks for your password once the scan is done and says what happened to each file.

### What was scanned

What a scan of yours found stays on the app's overview until a clean scan of the same place, or
**Dismiss**. The scans you ran are listed in `~/.local/state/amethystora/security-scans.json`, and
`ame security scan log` shows everything the weekly scan and real-time scanning logged.

## Ransomware protection

Every hour, a read-only snapshot of every home folder on this machine, kept in `/var/home/.snapshots`.
Ransomware running as you can encrypt everything you can write, and a snapshot is not something you can
write: changing or deleting one takes an administrator.

```bash
ame security ransomware
```

It is on from the first start of a machine installed from an Amethystora ISO; on one that was running
before, it stays as you left it. Every snapshot from the last day is kept, and one a day for two weeks.
The report says when the latest one is more than a few hours old.

### Getting files back

Open **Backups** and pick a time from before the attack ([Backups](backups.md)): **Put the whole folder
back** replaces the files the folder had then and leaves the ones it did not, such as what ransomware
renamed.

In a terminal, `ame security ransomware restore` asks for the time to go back to: **Open it in Files**
shows your home folder as it was then, to copy from, and **Put a folder back** does what Backups does.
Nobody else's files are visible in a snapshot, and yours are not visible to them.

### What it needs and costs

It needs the home folders on a btrfs subvolume of their own, which is how the installer sets up a disk
unless it was partitioned by hand. The cost is disk space: a snapshot holds on to what was changed or
deleted since, so a file you delete to make room frees its space only when the last snapshot that has it
goes, up to two weeks later.

Snapshots are never deleted to make room, because ransomware that rewrites every file would then delete
the ones from before it. Turning it off offers to delete them.

### Keep a backup elsewhere

Snapshots live on the same disk, and ransomware that gets administrator rights can delete them. Keep a
[backup](backups.md) somewhere else as well.
