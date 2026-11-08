# The dock

The dock holds your pinned apps, the running ones, the last few you used, your folders and the trash.

### Opening and pinning

- `Ctrl+Alt+1` to `Ctrl+Alt+9` switch to the pinned app in that place, and open it if it is not running. Hold `Super`
  for a second to see the numbers.
- Right-click an app to pin or unpin it, or drag it off the dock: it dissolves, and stays in the app grid.
- Right-clicking one of Amethystora's own apps also offers its pages: **Hotkeys** in the Manual, **Tasks** in Notes,
  **Crashes** and **Apps and Services** in Logs, **Backup Status** in Backups, **Virus Scan** in Security, and
  **Appearance** and **System** in Control.

### What an icon shows

An app's icon, on the dock and in the app grid, shows how many notifications it has waiting, a bar while it is busy
with something long, and is greyed out while the app is being updated. Do Not Disturb hides all of that. A terminal's
icon counts its open windows instead, from two.

## How the dock moves

The icons swell in a wave under the pointer. Choose **Lift** to raise only the icon under the pointer, or **Still**,
in [Control](control.md)'s **Dock** settings or in a terminal.

![The dock swells](media/dock-magnify.webm)

    ame desktop dock magnify lift      # or swell, off
    ame desktop dock amount 180        # how far Swell grows, 120 to 220 per cent
    ame desktop dock size 56           # the icons at rest, 40 to 64 pixels; reset for the dock's own choice

A dock too long for its screen draws its icons smaller than the size you picked, so that every one fits.

### Starting and running

While an app starts, a ring runs round its icon until its window opens (`ame desktop dock launch hop`, `breathe` or
`off` for another sign or none). A bar under an icon marks a running app, twice as long with two windows or more
(`ame desktop dock indicator dot` for a dot per window, `glow` for a light under it).

## What the dock holds

### Recent apps

After the running apps come the last three apps you used that are neither pinned nor running (`ame desktop dock
recent off` leaves them out). GNOME's own record of app use fills them at login and after the screen was locked,
since the dock keeps the order only in memory; with **Usage history** off in Settings › Privacy, there are none.

### Windows and folders of apps

- Rest the pointer on a running app to see its windows above it, and click one to bring it forward (`ame desktop dock
  previews off` shows only names).
- Right-click it, or hold it, to fan all its windows out over its menu: scroll or press Left and Right to go through
  them, and click one or press Space.
- Drop one pinned app on another and wait until it lights up to make a folder. Click the folder to open it, and its
  name to rename it; **Take the folder apart** in its menu puts the apps back.

### Stacks

Your Downloads folder is a stack: click it to see the twelve newest things in it, newest first, and **Open in Files**
for the rest. While files arrive in Downloads, the stack counts them (**+3**) and hops for each; the count, kept in
memory only, is gone once you open the stack.

![Downloads arriving](media/dock-downloads-arrive.webm)

Drag a folder from Files onto the dock to keep it there as a stack too, and drag a stack off the dock to take it away.
Files dropped on a stack are moved into that folder, or copied while you hold `Ctrl`, by Files, which shows how far it
got and can undo it. Files dropped on **Phone** go to your paired phone.

    ame desktop dock stack add ~/Documents
    ame desktop dock stack remove ~/Documents

### Apps being updated

While a Flatpak app is being updated, by Bazaar, `flatpak update` or the automatic updates, its icon on the dock and
in the app grid is at half strength. You can still start it: it runs the version it has, and the new one next time.
`ame desktop dock updating off` leaves the icons as they are.

## Where the dock lives

The dock is at the bottom of the main screen, and shows the apps of every window. Put it on the left or right, or on
every screen, where each screen's dock shows the apps whose windows are on that screen beside the pinned ones:

    ame desktop dock edge left          # or right, bottom
    ame desktop dock screens every      # or primary, the main screen only

`ame desktop dock autohide touch` puts it out of the way: windows use the whole screen, and the dock slides away
while one touches it. Push the pointer against that edge to bring it back. Where a screen continues on another past
that edge, GNOME cannot keep a strip free, so that screen's dock always hides that way.

`ame desktop dock workspace-apps on` keeps only the apps of the workspace you are on, besides the pinned ones. A click
on a pinned app whose windows are on another workspace takes you to them.
