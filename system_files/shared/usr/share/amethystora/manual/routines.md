# Little routines

A routine is something done by itself when something happens: "when I join the office Wi-Fi, open Files and mute".
The agent writes it; only you keep it.

![A routine runs](media/routine-trigger.webm)

### What a routine is

One readable file names **one trigger** and up to eight **actions**. The trigger is a time (with days), a network, an
app opening, closing or going fullscreen, the screen unlocking or locking, the battery below a level or unplugged or
plugged in, or your phone coming near or going away. An action is an app to open by its launcher's id, or an `ame`
command as words, with an optional `undo` command that puts it back:

```json
{"name": "Office morning",
 "when": {"network": "Office Wi-Fi"},
 "do": [{"open": "org.gnome.Nautilus.desktop"},
        {"ame": ["system", "sound", "mute"], "undo": ["system", "sound", "unmute"]}]}
```

### Keeping one

Ask the agent to write one; it checks it with `ame agent routines check FILE`. **Keeping it is yours**:
`ame agent routines add FILE` shows what it does in words and asks in a terminal, and the agent's tools will not run
it. `list`, `show`, `pause`, `resume`, `remove`, `run` and `undo` manage them.

A routine runs as you, never as root, never a shell line, never an `ame security`, `vault`, `agent` or `plugin`
command, and never anything the agent itself may not run. A file with something that looks like a password or a token
in it is refused.

### When it runs

Each run shows in the pill once: **Office morning ran**, what it did, **Pause this routine** and **Undo** for a minute
(Undo runs the way back the file gave, and closes the apps the routine opened that were not open before). A routine
that fails says so once and pauses itself until you start it again. The same trigger twice in ten seconds runs it
once.

While the screen is locked the desktop's extension is off, so a time that falls then is missed; a routine for the
screen locking has to say `"while_locked": true`.

### Routines and the Vault

What a routine names tells a stranger about you, so routines are **sealed in the [Vault](vault.md)**, under its
background key. **A routine runs only while the Vault is open, unless that key is on** (`ame vault methods background
on`, which needs a TPM): locking the screen closes the Vault, and without the key the routines stop until you open it
again.

Nothing waits and nothing is queued: what happens meanwhile is missed. A notice says once that your routines wait for
the Vault (with **Open Vault**), and a routine that finds the Vault closed when its trigger fires says so once. While
the Vault's window is open the desktop looks again every fifteen seconds and starts listening as soon as it can.
`ame agent routines add` says all this before it keeps one.

### Routines and the agentic features

Routines run only while the agentic features are on (`ame agent routines off` stops them too). With none made, nothing
is made, watched or asked: no folder, no listening, and no call to the Vault. Only the sources a routine names are
listened for.
