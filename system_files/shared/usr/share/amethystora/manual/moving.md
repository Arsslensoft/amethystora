# Moving in

Ways to bring your things from another computer, and to send a file to someone far away. Each says what it reads, what it
copies and what it never touches, and none of them sends anything until you start it. To send a file to a phone or another
computer on your own network, see [Send to nearby](phone.md#send-to-nearby).

## Move to this computer

Your folders and your setup, from an old computer to this one over the network, with nothing in between: no account, no
cloud. Both computers run Amethystora, on a network you named home (see [Security](security.md)), and a person sits at each.

On **this** computer, the new one, in Control's **System** page choose **Move to this computer**, or in a terminal:

```bash
ame system move receive
```

On the **old** computer, **Move this computer's things to a new one**, or:

```bash
ame system move send
```

If the old computer finds more than one, it asks which; to call one directly, `ame system move send --to 192.168.1.20`.
Both screens then show the same six digits. **Compare them by eye**: if they differ, answer no on either side and nothing
moves. They prove that nobody sits between the two computers; they are not a password, and nothing of them is kept.

The new computer then lists what the old one has, and you choose:

- **The home folder**: the folders you made yourself (Documents, Pictures, Music, Videos, Downloads, Desktop and any other),
  the data of your Flatpak apps (`~/.var/app`, less their caches), and the Vault's own backup file, which opens only with its
  recovery key. To bring the Vault's contents, restore that file with `ame vault restore` and your recovery key.
- **The setup file**, which names the apps, theme and shortcuts (`ame setup save` makes it). It lands in `~/From old computer`;
  bring it in with `ame setup apply "$HOME/From old computer/setup.json"`, which shows what it would change and asks first.

What it **never** copies: browser profiles, saved logins, cookies, keyrings, `~/.ssh` and `~/.gnupg`, the Private folder,
every hidden folder not named above, and places that start programs (`~/bin`, `autostart`). Links and devices stay behind.
The new computer refuses these itself, whatever the other end sends.

It **adds and never deletes**: a file whose name is already on this computer is kept as it is, and the new computer says how
many. A file appears under its name only when it is whole, so stopping half way leaves no half files, and running it again
carries on with what is missing. It runs at the lowest disk and processor priority, so the desktop stays quick, and leaves a
gigabyte free: a home that does not fit is left out, and said.

The key is agreed for this move alone (P-256, each direction sealed with AES-256-GCM), lives in memory and is gone when the
command ends. Only a computer on a network named home can reach the other: the firewall opens port 4793 in that zone alone, the
new computer listens on it only while the command waits, and each side checks the zone again before it speaks. A move needs a terminal and
cannot be started by [the agent](ai-agent.md).
