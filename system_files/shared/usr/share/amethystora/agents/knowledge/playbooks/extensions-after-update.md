# GNOME extension errors after an update

## Symptom

After an image update and a restart: an extension is gone from the top bar or shows "Error" or "Out of
date" in Extension Manager, one the user turned off is back on, windows tile again after the user chose
floating ones, a theme change freezes the screen for a moment, or a top-bar widget shows an error.

## Look

- `state health` (finding `shell-errors`), `state updates` (the deployments).
- `logs` identifier `gnome-shell`, grep `JS ERROR|Extension`; boot `-1` for the start before the update.
- `gnome-shell --version`
- `gnome-extensions list --enabled`, then `gnome-extensions info UUID` (its State: ACTIVE, ERROR,
  OUT OF DATE)
- `journalctl --user -b -t gnome-shell -g 'JS ERROR|Extension' --no-pager`
- `rpm-ostree db diff` (did `gnome-shell` change?)
- `gsettings get org.gnome.shell disable-user-extensions`, `gsettings get org.gnome.shell.extensions.amethystora-ui.tiling enabled`

## Causes on Amethystora

1. **A new GNOME, and an extension the user installed lags behind.** A Fedora release in the image brings
   a new GNOME Shell, which refuses an extension whose `metadata.json` does not name it. Tell: `gnome-shell`
   changed in `rpm-ostree db diff`; the extension is in `~/.local/share/gnome-shell/extensions` with State
   OUT OF DATE or ERROR. The image's own, in `/usr/share/gnome-shell/extensions`, are built for its GNOME.
2. **The image turned its own extensions back on.** Once per image version,
   `/usr/share/amethystora/user-setup.hooks.d/15-extensions.sh` puts every extension the image ships back
   in the account's list and clears `disable-user-extensions`, so one turned off by hand, or after an
   error, comes back. The classic layout is not an extension turned off: it is the `enabled` key of
   `org.gnome.shell.extensions.amethystora-ui.tiling`, which an update leaves as it is. Tell: that key is
   `true` although the user wanted floating windows; the extension back in `list --enabled`.
3. **Amethystora's own extensions.** `amethystora-transitions@arsslensoft.github.io` reveals a new theme or
   wallpaper and holds the screen for five seconds at most; `amethystora-widgets@arsslensoft.github.io` runs
   the widgets in `~/.config/amethystora/widgets`. Tell: their uuid in the JS errors. A widget whose
   command fails shows that in the bar, not as a GNOME error.
4. **New extensions are blocked** (`block-user-extensions`, off as shipped). Extension Manager cannot
   install or update from extensions.gnome.org; the image's extensions are not affected. Tell:
   `gsettings get org.gnome.shell allow-extension-installation` is false, and
   `gsettings writable org.gnome.shell allow-extension-installation` false; `state security-settings`.
5. **A bug in the update itself.** Errors from an extension of the image that began with the new
   deployment. Tell: the same errors absent at boot `-1`; `rpm-ostree db diff`.

## Fix

### 1. An out-of-date extension

`gnome-extensions disable UUID` stops it now. Update it in Extension Manager once its maker supports the
new GNOME, then log out and in (restart): on Wayland, an extension's new code loads only at login. Never set
`disable-extension-version-validation`: an extension built for another GNOME can crash the shell, which on
Wayland ends every open app.

### 2. The image's extensions came back

- Floating windows: `change` `layout` `classic` (`ame desktop layout classic`), which turns the tiling off
  in its own key and stays through updates; never disable the Amethystora UI extension instead.
- Another of the image's extensions: turn it off again in Extension Manager, or
  `gnome-extensions disable UUID`; it lasts until the next image update.

### 3. Amethystora's own

- Transitions: `change` `transition` `none` (`ame desktop transition none`).
- Widgets: `amethystora-widgets run ID` runs one as the bar does and checks what it prints;
  `amethystora-widgets off ID` hides it; `amethystora-widgets restart` starts every widget again. The
  `amethystora-widgets` skill explains the format.
- Report an error of either at https://github.com/arsslensoft/amethystora/issues, with `gnome-shell --version`
  and the error lines, if the user agrees.

### 4. Blocked installs

The user runs `ame security block-user-extensions off` (root). An extension runs inside the desktop and
sees every key and pixel.

### 5. The update

The previous deployment, until a fixed image is out: `sudo rpm-ostree rollback` (root) (restart); on a sealed
image, `sudo bootc rollback` (root) (restart).

## Verify

- `gnome-extensions info UUID`: ACTIVE, or INACTIVE for one turned off.
- `/usr/libexec/amethystora-doctor --only shell-errors` counts the whole boot: after logging out and in,
  read `logs` identifier `gnome-shell` with `since` the login time instead, or run it after the next restart.

## Undo

- `gnome-extensions enable UUID`.
- `change` `layout` `tiling` (`ame desktop layout tiling`); `change` `transition` back to the style
  `state settings` showed before, or `undo` with the change's record.
- `amethystora-widgets on ID`.
- Blocked again: `ame security block-user-extensions on` (root).
- A rollback: the next `ame update` brings the newest image back (restart).
