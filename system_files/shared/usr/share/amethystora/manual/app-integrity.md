# Your apps and the system's own files

An update is checked before it installs: the system's must carry Amethystora's signature, and an app's its
maker's or Flathub's. After that, on an ordinary image, nothing checks those files again as they are read;
a [sealed image](streams.md#sealed-images) does. With **App integrity** on
(`ame security settings app-integrity on`, and in every profile), they are compared with what their
updates installed once a day, and again after each change to the apps installed for everyone:

- **The system's own files**, and the kernel and initramfs it starts with. One changed there means
  something with administrator rights changed this system, and the system cannot vouch for its own
  repair: back up your files and reinstall from the installer
  ([Starting over](starting-over.md#starting-over)).
- **Each Flatpak app and runtime**, and its launchers, which have to start the app they belong to. Every
  launcher in the app grid that comes from Flatpak has to be an installed app's. An app installed from a
  source that does not sign it, or signed by a key this machine does not know, is said once.
- **The apps installed for you alone**, read as you, for the accounts used in the last day. What changed
  there is said to you and to the administrators.

Only what changed since the last check is read, so a check with nothing new costs a walk of the files'
details.

```bash
ame security app-integrity                                # what the last check found
ame security app-integrity check                          # check now
ame security app-integrity repair                         # fetch each app it named again
ame security app-integrity repair org.mozilla.firefox     # only that app, named or not
```

A repair puts an app back as its update installed it; what the app keeps in `~/.var/app` stays. Homebrew,
`~/.local/bin`, the AI agents, editor and browser extensions and containers keep no signed record of what
they installed, so they are not checked this way: the watcher looks at where they start from instead.

## Plugins

A [plugin](plugins.md) runs only while its files are the ones you approved, whatever this check says. With
**Plugins** on (`ame security settings plugins on`, and in every profile), the watcher also tells you, at
each of its runs, when something changed them anyway:

- a plugin's files no longer matching what you approved;
- what a plugin may do changed without the plugin helper recording it;
- a file of the plugins' store that is not the system's alone;
- a plugin's process that left the limits it runs under;
- each plugin installed while [plugin approval](security-switches.md#switches) was off.

Only something with administrator rights can make most of these changes, so none of them can be allowed:
put the plugin back with `ame plugin add`, or remove it.
