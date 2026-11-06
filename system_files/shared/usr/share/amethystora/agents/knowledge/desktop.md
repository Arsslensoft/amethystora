# GNOME's own settings, from the shell

Written for AI agents. Amethystora's own settings go through `state settings` and `change`; GNOME's go through
these commands in the agent's own shell, as the user. Read before writing, change one thing, read it back;
`gsettings reset SCHEMA KEY` undoes a change.

## Appearance and text

- Dark style: `gsettings set org.gnome.desktop.interface color-scheme prefer-dark` (or `default`). On
  Amethystora the theme decides it: `amethystora-theme toggle` switches to the theme's light or dark pair,
  which is better than setting the key alone.
- Text size, to make the text bigger or smaller everywhere: `gsettings set org.gnome.desktop.interface
  text-scaling-factor 1.25` (1.0 is the normal size; Large Text in Settings, Accessibility, is the same 1.25).
- Font: `ame desktop font "<family>"|theme`, not `gsettings set ... font-name`, which the next theme switch writes over.
  The monospace font (terminal, code, Text Editor) is `ame desktop mono-font "<family>"|theme`, not
  `monospace-font-name`.
- Animations: `gsettings set org.gnome.desktop.interface enable-animations false`; the theme transitions stop
  with them.
- Comfort: `ame desktop comfort reduce-transparency|reduce-motion|increase-contrast on|off` (the glass solid; GNOME's
  Reduced Motion; GNOME's High Contrast), and `ame desktop comfort` alone shows all of them and the power saver.
- The desktop's own sounds are the sound theme `amethystora` (`/usr/share/sounds/amethystora`, the Soft set):
  `ame desktop sounds soft|off` (off is GNOME's `event-sounds` false and the theme `amethystora-silent`, whose silent step stops the volume keys' sound; apps still make their own). The login chord
  and the power-plug sound are played by Amethystora UI's `sounds` module.
- Rounded corners on a laptop's own screen (not an external monitor): `ame desktop screen-corners on|off`, the key
  `org.gnome.shell.extensions.amethystora-ui.bezel` `round-corners`.
- The lock screen's large clock and cards are GNOME Shell's own stylesheet, rebuilt when the image is built
  (`/usr/libexec/amethystora-login-background`): nothing on the machine changes them.
- Quick settings' tiles are moved and hidden by holding one (arrange mode), kept in
  `org.gnome.shell.extensions.amethystora-ui.quick` `tile-order` and `tile-hidden` (ids such as `wifi`, `dnd`,
  `battery`, `theme`): `ame desktop quick-settings reset` puts every tile back, `ame desktop quick-settings theme
  on|off` shows or hides the theme swatches. The battery's 80% switch is UPower's charge limit, nothing else.

## Accent, wallpaper, icons and style

- Accent colour: `ame desktop accent amethyst|iris|sky|lagoon|jade|moss|amber|ember|coral|rose|orchid|slate|theme`
  (the account's `~/.config/amethystora/accent`, which every theme switch reads); `gsettings set
  org.gnome.desktop.interface accent-color purple` only moves GNOME's nearest of nine, which the shell and the apps no
  longer take.
- The first wallpaper by the time of day: `ame desktop daylight on|off` (on unless turned off; the desktop's
  `picture-uri` is then `~/.config/amethystora/current/backgrounds/<theme>-daylight.xml`, GNOME's own slideshow).
- Colours from the wallpaper: `ame desktop colours vivid|soft|mono|off` (`~/.config/amethystora/wallpaper-colours.toml`,
  written by `/usr/libexec/amethystora-wallpaper-colours`).
- An accent per workspace: `ame desktop accent-per-workspace on|off`, and `ame desktop accent-per-workspace 3 coral`
  for one workspace's.
- Icons: `ame desktop icons glass|glass-light|neon|neon-light|duotone|line|clay|muted|theme`, not
  `gsettings set ... icon-theme`, which the next theme switch writes over.
- The shape of the top bar, menus, notifications, dialogs, buttons and windows: `ame desktop style
  islands|floating|hairline|capsule|terminal|segments|neon|theme`.
- The glass behind the top bar, dock, menus and notifications: `ame desktop glass frost|prism|clear`.

## Night light and the screen

- Night light: `gsettings set org.gnome.settings-daemon.plugins.color night-light-enabled true`, and its
  warmth `night-light-temperature` (uint32, for example `uint32 3500`).
- Screen blank after idle: `gsettings set org.gnome.desktop.session idle-delay uint32 600` (seconds; 0 never).
- Displays, scaling and refresh rate are GNOME Settings' (`gnome-control-center display`): nothing on the
  command line sets them reliably on Wayland.

## Notifications and Do Not Disturb

- Do Not Disturb: `gsettings set org.gnome.desktop.notifications show-banners false` (true to end it).
- Rest reminders are GNOME's own (`org.gnome.desktop.break-reminders`): `ame desktop breaks
  on|eyesight|movement|both|off`; their times are in Settings, Wellbeing.

## The top bar and the calendar

- The top bar and the dock start on the main screen only. `ame desktop top-bar screens on` gives every other screen a
  bar with its workspaces and the clock, and `ame desktop dock screens every` a dock of its own. Each meter on the bar
  is switched by `ame desktop top-bar meter processor|memory|network on|off`; off reads nothing. Pushing the pointer against the top edge over a fullscreen window brings
  the bar down: `ame desktop top-bar peek on|off`.
- The calendar menu lists today's and overdue tasks from Notes while Notes is open and unlocked:
  `ame desktop today tasks on|off`. Notes says them on the session bus (`org.amethystora.Notes`,
  `/org/amethystora/Notes/Today`) to GNOME Shell alone.
- The weather beside the clock sends the place chosen in Weather to a weather service and is off until chosen:
  `ame desktop weather on|off`. Never turn it on without the user's word.

## Power

- Power profile: the image has no `powerprofilesctl` (tuned-ppd answers power-profiles-daemon's interface).
  Read it with `busctl --system get-property org.freedesktop.UPower.PowerProfiles
  /org/freedesktop/UPower/PowerProfiles org.freedesktop.UPower.PowerProfiles ActiveProfile`, set it with
  `busctl --system set-property org.freedesktop.UPower.PowerProfiles /org/freedesktop/UPower/PowerProfiles
  org.freedesktop.UPower.PowerProfiles ActiveProfile s power-saver` (or `balanced`, `performance`), or in the
  quick settings' power menu. While it is `power-saver` the glass is solid and animations take half their time.
- Suspend when idle on mains power: `gsettings set org.gnome.settings-daemon.plugins.power
  sleep-inactive-ac-type nothing` (or `suspend`).

## Keyboard and input

- Layouts: `gsettings get org.gnome.desktop.input-sources sources`, and to set: `gsettings set
  org.gnome.desktop.input-sources sources "[('xkb', 'us'), ('xkb', 'fr')]"`.
- Keybindings: `/usr/share/amethystora/keybindings.md` lists the image's; check it before binding a key.
  Custom ones live under `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/` (the image uses
  `custom20` and up).
- Touchpad: `gsettings set org.gnome.desktop.peripherals.touchpad tap-to-click true`, `natural-scroll`.
- `Super+.` is the emoji picker, Amethystora UI's `emoji` module (`ame desktop emoji tone`). IBus's own emoji
  `hotkey` in `org.freedesktop.ibus.panel.emoji` is empty in the image: IBus reads a text field's keys before
  GNOME's bindings and would take `Super+.` and `Super+;` for its panel. Do not set it back.

## Radios and the network

- Wi-Fi on or off: `nmcli radio wifi on|off`; networks: `nmcli device wifi list`, `nmcli connection show`.
- Connect: `nmcli device wifi connect SSID --ask` asks the password in a terminal: leave it to the user.
- Airplane mode and Bluetooth power: `rfkill list`, `rfkill unblock bluetooth`; Bluetooth devices:
  `bluetoothctl devices`, pairing is the user's (it shows a code).

## Sound

- Outputs and inputs: `wpctl status`; the default: `wpctl set-default ID`; volume: `wpctl set-volume
  @DEFAULT_AUDIO_SINK@ 50%`; mute: `wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle`.

## The dock and windows

- The dock's apps: `gsettings get org.gnome.shell favorite-apps`.
- Windows tile (Amethystora UI's tiling module) unless the user chose classic windows: `ame desktop layout
  tiling|classic` (the `layout` entry of `state settings`), which sets `enabled` in
  `org.gnome.shell.extensions.amethystora-ui.tiling`. Its other settings (`window-gap`, the margins,
  `cycle-width-steps`, `open-window-position`, `animation-time`, the edge and gesture keys, the window rules in
  `winprops`) are keys of the same schema, applied at once; `ame desktop tiling-settings` opens their window.
  With two screens or more each screen shows a workspace of its own, and the one under the pointer is GNOME's
  active workspace; the others are drawn from live pictures of their windows. Meanwhile the tiling sets
  `org.gnome.mutter workspaces-only-on-primary` false, putting the account's value back with one screen or classic
  windows. `dynamic-workspaces` stays the account's: with it on, GNOME removes no workspace a screen shows and keeps
  one more than there are screens. The image's six fixed workspaces are under `[org.gnome.mutter:GNOME]` in
  `zz1-amethystora-modifications.gschema.override`, since GNOME Shell sets that key for the GNOME desktop alone. Read
  a session's value with `XDG_CURRENT_DESKTOP=GNOME gsettings get ...`.
  Its keys are in `...tiling.keybindings`; a combination
  it shares with a GNOME shortcut is taken from it while the tiling runs and given back when it is off
  (four keys' lists are changed in dconf, and what was taken is in `~/.local/state/amethystora/tiling.json`, with a
  copy `tiling.json.bak` beside it for when the file cannot be read; never edit either by hand).

## The phone

- The phone link is Amethystora's own program, `amethystora-connect` (a user service), speaking KDE Connect's
  protocol, so the KDE Connect app on an Android phone pairs with it. It starts off: `ame desktop phone on|off`
  (the `phone` entry of `state settings`), and it is open only on the networks named home.
- Pairing is the user's: the phone's name and a verification key are on both screens and the user accepts.
  Never pair, accept or unpair for them.
- The clipboard is shared only if chosen, one way at a time: `ame desktop phone clipboard send|receive on|off`.
  Leave these to the user: receiving lets a phone put text where any program can paste it.
- Files from a phone arrive in `~/Downloads/Phone/NAME`, and pictures in `~/Pictures/Phone` (no folder per phone)
  while the key `photo-folder` of `org.amethystora.Connect` is `pictures`, as shipped:
  `ame desktop phone photos pictures|downloads`. The phone in the top bar is Amethystora UI's `connect` module, shown only while a paired phone
  is connected: one phone's battery and signal bars (`SignalStrength`, `NetworkType` on the device's D-Bus object,
  from the phone's connectivity report, which needs the phone app's permission to read the phone state).
- `ame desktop phone ring` rings the paired phones, even when silent (Found it on the phone stops it). Music from either
  side (`ame desktop phone music on|off`) and the phone as a slide clicker while Present presents (`ame desktop phone
  clicker on|off`) are off as shipped; each switch makes a connected phone reconnect once. The clicker sends only Page
  Down, Page Up and B, and only while presenting.

## Files' scripts menu

- Files shows the executable files of `~/.local/share/nautilus/scripts` under Scripts, for a selection only. The
  user-setup hook `24-files-menu.sh` writes, once per account, **Folder colour** (seven entries) and **Back in time**
  there, each marked `# amethystora-files-menu` in its second line, and lists them in
  `~/.local/state/amethystora/files-menu.json`; a deleted entry stays deleted.
- A folder's colour is `metadata::custom-icon-name` = `folder-amethyst|sky|jade|amber|coral|slate`
  (`gio info -a metadata::custom-icon-name DIR`), set by `/usr/libexec/amethystora-folder-colour COLOUR -- DIR...`;
  each icon pack draws the six. **Back in time** is `/usr/libexec/amethystora-back-in-time -- PATH`, which opens
  `amethystora-backups FOLDER` for a folder in the home only. **Earlier versions** is the same helper with `--versions`,
  which opens `amethystora-backups --versions=FILE` (`ame desktop earlier-versions`). **Sign a PDF** is
  `/usr/libexec/amethystora-pdf-sign -- FILE...`, the Vault's `pdf sign` behind a card that says a reader calls the signer unknown
  until it trusts this machine's root; a person's own approval for each signature, never the agent's.
- A file nothing opens: `zz-amethystora-open-anything.desktop` claims every kind no launcher in the image claims, and every
  other launcher comes before it, so anything installed takes its kinds from it. It looks the type up in Flathub's list already on the
  machine (`ame desktop open-anything off` hides it for the account). `Space` in Files is GNOME's previewer,
  `org.gnome.NautilusPreviewer`, a Flatpak of the default list that reads the home folder only.

## Copying text out of a picture on the screen

- The reading is Gradia's: `Print`, drag an area, then **Extract Text** or `Ctrl+E` in GNOME's screenshot UI
  (the button is Amethystora UI's screenshot module, in `amethystora-ui@arsslensoft.github.io`, and needs the
  Flatpak `be.alexandervanhee.gradia`). The text is copied at once. It works on an area only; the
  button is off in window, screen and recording modes.
- Offline after one prompted download: the first use opens Gradia's "OCR Language Models" page, and the
  chosen language's `tessdata_best` file comes from GitHub into
  `~/.var/app/be.alexandervanhee.gradia/data/tessdata`. Look there to see which languages are installed.
- The capture is kept in the screenshot folder like any screenshot. There is no `tesseract` command in the
  image and no `ame` command for this; do not install one for the user.
- Button missing: `flatpak info be.alexandervanhee.gradia` and `gnome-extensions info
  amethystora-ui@arsslensoft.github.io` (Extension Manager turns it back on; an image update does too).

## First start: the reveal, Pick a look, tips and the app grid

- The reveal at login (the boot splash's gem flying into the top bar) is `reveal` in
  `org.gnome.shell.extensions.amethystora-transitions`: `ame desktop transition reveal on|off`. It runs only while
  GNOME is starting, with animations on; nothing of it is left after the first second.
- The greeting at a new account's first login, before Pick a look, is Amethystora UI's hello module: `greet` in
  `org.gnome.shell.extensions.amethystora-ui.hello` (`ame desktop hello on|off`, `ame desktop hello show`). A layer of the
  shell, not a window, holding the keyboard as a modal dialog does until any key or click; it marks
  `~/.local/state/amethystora/hello.json`, once.
- Pick a look is a page of Control, `amethystora-control welcome`, opened once for a new account by
  `/usr/share/amethystora/user-setup.hooks.d/25-first-look.sh`; it writes `~/.local/state/amethystora/first-look.json`,
  and while that file exists the offer of classic windows (`/usr/libexec/amethystora-layout-offer`) is not made.
  Started from a unit, it has no activation token: Amethystora UI's firstlook module gives it the focus once GNOME's
  welcome dialog has closed, leaving the overview a first login starts in, unless the Tour was started or the person
  has moved to another window meanwhile.
- Tips are Amethystora UI's hints module: `enabled` in `org.gnome.shell.extensions.amethystora-ui.hints`
  (`ame desktop hints on|off`); what was shown is in `~/.local/state/amethystora/hints.json` (with a `.bak`), and
  `ame desktop hints reset` lets them come again. None in the first hour, one per feature ever, half an hour apart.
- The app grid's row of recent apps is `recent` in `org.gnome.shell.extensions.amethystora-ui.appgrid`
  (`ame desktop app-grid recent on|off`). The folders are GNOME's own (`org.gnome.desktop.app-folders`), the
  image's six by default; `ame desktop app-grid folders reset` puts them back, in a terminal, after asking.
