# Little routines

A routine is something the desktop does by itself when something happens. You write it as a file and check it; the user keeps it. You never
keep, run, resume or remove one: `ame agent routines add` shows the user what it does and asks them in a terminal, and your tools refuse it.

Write one JSON file, for example in `~/Documents` or a temporary folder, then check it:

```json
{"name": "Office morning",
 "when": {"network": "Office Wi-Fi"},
 "do": [{"open": "org.gnome.Nautilus.desktop"},
        {"ame": ["desktop", "volume", "mute"], "undo": ["desktop", "volume", "unmute"]}]}
```

`ame agent routines check FILE` says what is wrong or reads the routine back in words. Then tell the user to run `ame agent routines add FILE`.

- **One trigger** in `when`: `{"time": "18:00", "days": "weekdays"}` (`daily`, `weekdays`, `weekends` or `["mon", "fri"]`), `{"network": "NAME"}`
  (the connection's name as Network settings shows it), `{"app": "ID.desktop", "state": "opens|closes|fullscreen"}`,
  `{"screen": "unlocks"}` (`locks` needs `"while_locked": true` in the routine), `{"battery": "below 20|on-battery|plugged-in"}`,
  `{"phone": "near|away", "name": "optional"}`.
- **1 to 8 actions** in `do`: `{"open": "LAUNCHER.desktop"}` (a launcher id from `state software` or the app grid, never a path), or
  `{"ame": [words]}`: an `ame` command as separate words, which has to exist (`ame --list`) and may carry `"undo": [words]`, the command that
  puts it back. Undo is what the pill's Undo button runs, so give one for a command that switches something (`on`/`off`, `mute`/`unmute`).
- **Never**: a shell line, a program other than `ame`, root, `ame security`, `ame vault`, `ame agent`, `ame plugin`, anything the never list
  refuses, an option starting with a dash, or a word that looks like a password, token or key. A secret is never written into a routine; if a
  routine needs one, tell the user to use the Vault. `check` refuses these.
- Name the routine for what the user said, in their words. Show them the file before they add it.

What you can run: `ame agent routines list`, `show ID`, `check FILE` and `pause ID`. The user's: `add`, `resume`, `remove`, `run`, `undo`, and the
switch `ame agent routines on|off`. Routines are sealed in the Vault under its background key; if `list` says the Vault is closed, tell the user to
open it (you cannot).
