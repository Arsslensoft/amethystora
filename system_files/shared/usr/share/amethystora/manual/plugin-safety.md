# What a plugin may do

A plugin reaches only what it asked for and you allowed, and you can see, check and take back each permission at any time.

## What a plugin may ask for

| Permission | What it gives |
|---|---|
| `network` | HTTPS to named hosts, and only those. No other address, no other port |
| `files` | One place under your home folder, read-only unless it says otherwise. Never the home itself, a hidden folder or the private folder |
| `notify`, `open`, `clipboard-write`, `pick` | A notice under its name, an address to open after a click, text for the clipboard after a click, a file you choose |
| `state`, `control` | What the system says of the clock, the disk, what plays, and the few controls the desktop offers |
| `events` | Runs when the network, the power, the screen's lock, the light or dark style, or (asked for on its own) the app in front changes |
| `network-local` | One service on your home network, by name and port, only while you are on a network you named home |
| `change` | Settings of Control's it names, after a click, each recorded with its way back as the agent's changes are |
| `window` | A window of its own, over Wayland only: no X11, no screen or sound capture, no portals |
| `key` | One shortcut, which never takes one already in use |

Never grantable: root, a raw bus, the whole home, another plugin's data, the keyring, the screen or microphone, and input.

Two permissions together are said aloud: `files` with `network` means the plugin can send those files to those hosts. An allowed host is a
way out, up to its limit of 1 MiB an hour unless it asked for more and you agreed.

## See, check, take back

```bash
ame plugin list              # what is added, on or off
ame plugin show ID           # what it may do, what it asked for, what is blocked, and how it is sandboxed
ame plugin verify            # every file against what you approved
ame plugin history           # what was added, allowed, blocked and removed, and when
ame plugin revoke ID PERM    # less (all of it, with no PERM); no password needed
ame plugin block ID [HOST]   # stop what it sends, to a host or to all; at once, and what is open is cut
ame plugin unblock ID        # an administrator approves, so that nothing running as you can lift a block
ame plugin grant ID          # allow more of what it asked for: asks in a terminal, and an administrator approves
ame plugin rollback ID       # the version before its last update, with no more than you allowed
ame plugin on ID             # run it, within what it was given
ame plugin run ID [ACTION]   # one run now, as a script may: it opens, copies and changes nothing
ame plugin off ID
ame plugin remove ID         # gone, with what it kept
ame plugin check FOLDER      # what installing would refuse, for a plugin's author
```

If a file of a plugin changes after you approved it, the plugin stops and `ame plugin show` says so: only you decide what to do next.

### In Security

Security's **Apps** page lists your plugins with what each was given, and blocks one's network or takes back what it was given in a click,
with no password. With [Plugins](app-integrity.md#plugins) on in Security, the security watcher says so too, and the report counts what
it found.

### For administrators

Plugins are kept in `/var/lib/amethystora/plugins`, which only the system can write. An administrator can turn them all off or cut their
network in `/etc/amethystora/plugins.conf`. `ame security plugin-approval off` lets an administrator install a plugin without the dialog,
which also lets any program running as that administrator do it: each plugin installed that way is recorded in `ame plugin history` and
said in a notification.

## What the sandbox does not promise

- It shares the kernel with everything else: a hole in the kernel is a way out.
- It protects your account from the plugin, and the store protects the plugin's code and grants from your account. Neither
  gets back what a program already running as you can read by itself.
- A host you allowed is a way out for whatever the plugin knows, up to its limit. Only a block stops that completely, because
  what goes through HTTPS cannot be read on its way.
- It says the plugin is what you approved, not that it is good.

See [security](security.md) for the rest of what protects this computer.
