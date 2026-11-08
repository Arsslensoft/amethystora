# Getting started

A few minutes now and the machine looks after itself from then on. None of these steps is required, but
each is worth doing once.

## 1. Let Secure Boot trust the image

With Secure Boot on, the kernel modules built into the image (DisplayLink docks, virtual cameras) load
only once their signing key is enrolled. The first boot may already have asked; if not, run:

```bash
ame security secure-boot
```

At the next restart a blue screen appears, the MOK manager. Choose **Enroll MOK**, **Continue**, **Yes**,
and type the password `amethystora`, on a US (QWERTY) keyboard layout.
[Hardware](hardware.md#secure-boot) has the details.

## 2. Check that everything is on

```bash
ame security status
```

One screen of what the machine's security settings are doing, and what to do about anything that is off.
It changes nothing and asks for no password. The same report is **Security** in the app grid, which scans
for viruses too ([Security](viruses.md#scanning-for-viruses)).

## 3. Install your apps

The apps Amethystora comes with (Calculator, Text Editor, Extension Manager and the rest) install by
themselves the first time the machine is online, and appear in the app grid as each one finishes.
`ame apps flatpaks` installs them at once. Uninstall any you do not want; they stay uninstalled.

Then open **Bazaar**, the software centre, and install what you use from Flathub. The browser is Firefox;
Chrome and Brave are in Bazaar too. [Installing software](software.md) covers command line tools and the
rest.

![Install from Bazaar](media/bazaar-install.webm)

### Coming from another machine

- **Another Amethystora machine that was backed up:** `ame restore-setup` puts back its apps, containers,
  extensions and theme ([Backups](backups.md)).
- **Your setup in a file:** `ame setup apply <file>` does the same from it ([Your setup in a file](setup.md)).
- **Your files from an old computer:** [Moving in](moving.md).

## 4. Make it yours

- `Super+Ctrl+Shift+Space` picks a theme. It repaints the desktop, the terminal and the prompt at once.
  [Themes](themes.md).
- Windows tile by themselves on a strip that scrolls: open two or three apps and move between them with
  `Super+Left` and `Super+Right` ([Tiling windows](tiling.md)). For windows that float and overlap,
  `ame desktop layout classic` ([Classic windows](tiling.md#classic-windows)).
- `Super+Alt+Space` opens the Amethystora menu, which reaches everything in this manual from the keyboard.
- **Control** (`Super+Ctrl+Shift+C`, or **Control** in the app grid) has Amethystora's own settings in one
  window: the theme and wallpaper, the window layout, games, Android apps, the AI agent, the image and
  more. [Control](control.md).

## 5. Set up a backup

```bash
ame backup
```

A daily backup to somewhere ransomware on this machine cannot delete from; the command walks you through
choosing where. **Backups**, in the app grid, then shows what was backed up and puts files back.
[Security](backups.md).

## 6. Learn the keys

The [Hotkeys](../keybindings.md) page is worth a read. Keep it open on a second workspace for your first
week.

## Then forget about it

Updates arrive by themselves and are applied at the next restart. Shut the machine down when you are done
for the day and it stays current without you thinking about it. [Updates](updates.md) explains what happens
and how to undo one.
