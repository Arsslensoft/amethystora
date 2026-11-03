# Plugins

A plugin adds something of its own to the desktop: a build's status, a package on its way, a door that is open. It runs in a
*sandbox*, as a program of its own, and only describes what to show. It cannot do more than you let it, and nothing it was
approved with can change without an administrator's approval.

This page says what works today. A plugin cannot be seen on the desktop yet: the pill, the top bar and the other places that will
draw one are still to come. What you can do now is the part that keeps you safe: add a plugin, read what it asks for, allow each
part you choose, run it in its sandbox, and take back what you gave.

## Add one

```bash
ame plugin add https://example.org/ci-build.git
ame plugin add ./ci-build
ame plugin add ci-build.amethystora-plugin
```

Nothing of the plugin runs while you add it. The files are first kept where only the system can change them, and you read what
they are: the plugin's name and who wrote it, a *digest* of its files, and what it asks for, each with the reason its author gave.
You answer each permission on its own, and the plugin has to cope with a no. This question is asked in a terminal and has no way to
be answered in advance. When you have decided, your computer asks an administrator to approve, in its own dialog, which names the
plugin and each permission it is about to give.

If a page publishes the digest of a plugin, add it with `--digest` and nothing else is accepted:

```bash
ame plugin add https://example.org/ci-build.git --digest 24d7ee22fb319619df0095cbf547bb884532306cb2294241771ddd9b37099c44
```

A git address is taken at the version it has now, and `ame plugin update` shows what changed, and what is newly asked for, before
anything is installed. Nothing updates by itself.

## What a plugin may ask for

| Permission | What it gives |
|---|---|
| `network` | HTTPS to named hosts, and only those. No other address, no other port |
| `files` | One place under your home folder, read-only unless it says otherwise. Never the home itself, a hidden folder or the private folder |
| `notify`, `open`, `clipboard-write`, `pick` | A notice under its name, an address to open after a click, text for the clipboard after a click, a file you choose |
| `state`, `control`, `events` | What the system says of the clock, the disk, what plays, and the few controls the desktop offers |

Some are only listed so far: the plugin may ask for them, and the parts of the system that would answer come later. Never
grantable: root, a raw bus, the whole home, another plugin's data, the keyring, the screen or microphone, and input.

Two permissions together are said aloud: `files` with `network` means the plugin can send those files to those hosts. An allowed
host is a way out, up to its limit of 1 MiB an hour unless it asked for more and you agreed.

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
ame plugin off ID
ame plugin remove ID         # gone, with what it kept
ame plugin check FOLDER      # what installing would refuse, for a plugin's author
```

If a file of a plugin changes after you approved it, the plugin stops and `ame plugin show` says so: only you decide what to do next.
Plugins are kept in `/var/lib/amethystora/plugins`, which only the system can write, and an administrator can turn them all off
or cut their network in `/etc/amethystora/plugins.conf`.

## What the sandbox does not promise

- It shares the kernel with everything else: a hole in the kernel is a way out.
- It protects your account from the plugin, and the store protects the plugin's code and grants from your account. Neither
  gets back what a program already running as you can read by itself.
- A host you allowed is a way out for whatever the plugin knows, up to its limit. Only a block stops that completely, because
  what goes through HTTPS cannot be read on its way.
- It says the plugin is what you approved, not that it is good.

See [security](security.md) for the rest of what protects this computer.
