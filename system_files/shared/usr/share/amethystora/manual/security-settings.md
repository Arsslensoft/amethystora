# Settings

The **Settings** page of the Security app sorts every switch by what it is about: the security watcher,
the audit log, looking for new threats, virus scans, the network, logins, devices, apps and updates, the
kernel and boot, what is said, and what leaves this machine. Each category is a tile that says how many of
its switches are on; **Open** shows them, with what each does and, while it runs, what it costs.

![Turn a switch on](media/security-switch-on.webm)

Every change there opens in a terminal that says what it changes. They are kept in
`/etc/amethystora/security.conf`, and apply at once, apart from the audit log, which follows at the next
restart.

```bash
ame security settings                        # every setting, its value and its name
ame security settings virus-scan manual      # change one: its name, then the value
```

## What happens to what a scan finds

`on-detection`: `report` (the default) leaves it where it is, `quarantine` moves it to
`/var/lib/amethystora/quarantine`, where nothing can open or run it, and `delete` deletes it. This applies
to the weekly scan, the scans you start in the app and real-time scanning. Each file is checked again
before anything is done to it. A file stays in quarantine until it is put back or deleted for good.

```bash
ame security scan quarantine   # what is in quarantine
ame security scan restore      # put a file back, and optionally have the scanner leave it alone
ame security scan delete       # delete it for good
```

## The virus scanner

`virus-scan`: `weekly` (the default), on Sunday night; `manual`, which scans only when you start a scan;
or `off`, which runs nothing and downloads nothing. Either way clamd, which holds the virus signatures in
memory (over a gigabyte), runs only while something scans: it starts when a scan connects, which takes it
a moment, and stops within half an hour of the last one. Real-time scanning keeps it running.

## Real-time watching

`realtime`: `off` (the default) or `on`, for both halves at once; each can also be set on its own.
`realtime-watch` has the security watcher read each change as the audit log records it. `realtime-scan`
scans every file written in a home folder as it is closed: it costs some CPU while files are being
written, and starts once the first virus signatures have downloaded.

## YARA rules

Three sources, each `off` (the default) or `on`, which ClamAV then matches files against by their
structure, where a hash signature misses a variant:

- `yara-elastic`: Elastic's rules for Linux malware.
- `yara-signature-base`: Florian Roth's, mostly about Windows malware and attack tools, some of which a
  security professional runs on purpose.
- `yara-forge`: YARA Forge's core set from many authors, only the rules whose licence asks nothing but
  their author's name.

They are downloaded on this machine daily, not on a metered connection, and each rule ClamAV cannot load
is left out. A match names the rule and its author. The weekly scan, with them or not, waits for mains
power on a laptop; a scan you start does not.

## Network protection, audits and the audit log

- `network`: `off` (the default), `watch` or `block` ([Network protection](network-security.md#network-protection)).
- `settings-audit`: Lynis every month, `on` (the default) or `off`. `ame security audit` runs one whenever
  you ask.
- `audit-log`: `on` (the default) or `off`, from the next restart.

## The watcher's parts

`watcher`: `on` (the default) or `off`, and each part of what it reads on its own. Each of these is `off`,
`record` (kept in the history without a notification) or `on`, the default, apart from `user-programs`,
which is recorded:

- The audit log's kinds of change: accounts (`identity`), `sudo` (`privilege`), what starts by itself
  (`persistence`), the security settings (`hardening`), kernel modules (`modules`), keyboard readers
  (`input-capture`) and programs in home folders (`user-programs`).
- The journal (`journal`), open ports (`listeners`) and the commands a session finds first (`user-paths`).

The image's settings (`image-drift`), the daily setuid search (`setuid-sweep`) and the report's alerts
(`report-alerts`) are `on` or `off`.

## Notifications

`notify`: `all` (the default), `critical`, or `none`. Whatever is not said is still in the report and in
`ame security events`.
