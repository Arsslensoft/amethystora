# The logs

**Logs**, in the app grid or `amethystora-logs`, reads what the machine and its programs have written. It
opens on **Important**: the errors from everything on the machine since it started. Logs takes the place of
GNOME Logs, which the image no longer installs; Bazaar still has it.

![Search, and the list narrows](media/logs-filter.webm)

### The views

The sidebar has **All**, split into your session, the system, the kernel and hardware, security (sign-ins,
`sudo`, what the audit rules or the file-watch sensor caught) and **Crashes**. Important and Crashes say
how many there have been since the machine started. `amethystora-logs` followed by `all`, `session`,
`system`, `kernel`, `security`, `crashes` or `sources` (Apps and services) opens it on that view.

### Finding an entry

- **Search** (`Ctrl+F` or `/`) looks through what the messages say as you type, whatever their case. The
  `.*` button takes the search as a regular expression.
- **Time** goes back to the previous boot, any boot the journal still keeps, the last hour, day or week,
  or everything. **Level** hides what is less serious than you pick.
- **Apps and services** lists everything that has written to the journal. Pick an app to read what it said
  each time it ran, or a service of the system or of your session; the field at the top finds one, and the
  foot says how much room the journal takes on disk.
- The newest 400 entries show first; **Show older** reads further back. A view with nothing in it offers
  **Look further back** and **Every level**.
- `Up` and `Down`, or `j` and `k`, go from entry to entry; `Esc` empties the search, then closes the open
  entry.

### Reading an entry

Click an entry to read all of it, with every field the journal keeps. **Only …** narrows the view to where
it came from, and **Around this** shows everything logged in the minute before and after it. With the
[AI agent](ai-agent.md) on, **Ask the agent** on a crash or on what a service wrote has it find out why,
changing nothing.

### Following and exporting

**Follow** adds new entries as they are written, for a time that runs up to now. **Export** saves what is
shown, up to 250,000 entries, to a text file or to JSON with one entry a line. The terminal button opens
the same view in `journalctl`, and the command for the view is at the bottom of the sidebar.

### Who sees what

Administrators, the accounts in `wheel`, see everything. Any other account sees its own session and apps
only, and the views of the system say so.

### In a terminal

| Command | Shows |
| --- | --- |
| `journalctl -b -p warning` | Warnings and errors since this boot |
| `journalctl -b -1 -p warning` | The same for the previous boot, after a crash or a freeze |
| `journalctl --user -b` | Your own session: the desktop, your apps |
| `systemctl --failed` | Services that failed to start |
| `coredumpctl list` | Programs that crashed |
| `ame system logs-this-boot` | Everything from this boot |
| `ame system logs-last-boot` | Everything from the previous boot |
| `ame system local-overrides` | The files in `/etc` you, or something you ran, have changed |
