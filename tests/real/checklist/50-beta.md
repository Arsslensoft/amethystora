# Beta: Fedora 45, GNOME 51 and rounds 5 and 6

Walk on a real machine running the beta image (`ghcr.io/arsslensoft/amethystora:beta`, Fedora 45, GNOME Shell 51), as a
person would: look, click, press the keys. Unless an item says otherwise the machine is in its shipped state. Where an
item changes a setting, put it back with the command in its flags before moving on. The ideas' specs are
`plans/ui-ideas/spec-<group>.md`; the manual pages named are in `/usr/share/amethystora/manual/` (or `Super+F1`).

The VM lab passed these features' suites in a software-rendered VM with no real GPU, sound card, phone, DDC screen,
IBus, Wi-Fi or fingerprint reader: what is listed here is what only a real machine shows, and what the lab could not
settle (section "Left open by the lab"). Mark each line as the other files do: `→ PASS`, `→ FAIL: what was seen`.

## Getting the beta

- [ ] B-GET-01 Switch to the beta. How: `ame system rebase amethystora beta` (or the image you run, -dx/-nvidia-open/-nvidia-lts), restart. Expect: the signature is checked (no unsigned warning); the machine starts on Fedora 45 (`cat /etc/os-release`: VERSION_ID=45) with GNOME Shell 51 (`gnome-shell --version`).
- [ ] B-GET-02 The image is the organisation's. How: `rpm-ostree status` or `bootc status`. Expect: the booted image is `ghcr.io/arsslensoft/amethystora...:beta`, signature-verified; `/etc/containers/policy.json` names ghcr.io/arsslensoft with cosign.pub.
- [ ] B-GET-03 Nothing failed at boot. How: `systemctl --failed; systemctl --user --failed` after the first login. Expect: none (grub-boot-success no longer fails on Fedora 45's read-only /boot).
- [ ] B-GET-04 Extensions all running. How: `gnome-extensions list --enabled --details | grep -E "@|State"`. Expect: the five extensions are `...@arsslensoft.github.io`, each ACTIVE; none `@iarsslen` left, and no ERROR state.
- [ ] B-GET-05 No faults of ours in the shell's log. How: use the desktop ten minutes, lock and unlock twice, then `journalctl --user -b -o cat _COMM=gnome-shell | grep -E "JS ERROR|already disposed|Amethystora UI"`. Expect: nothing naming amethystora-ui (GNOME's own lines aside).
- [ ] B-GET-06 Update and roll back. How: Updates › Update now when a newer beta is out, then Restart now; later `ame update` to roll back to the previous deployment. Expect: the update finishes, Restart now offers a soft reboot when the kernel did not change, the machine comes up in its session; rollback works.
- [ ] B-GET-07 Real GPU. How: look at the glass, the dock's swell, the transitions on a theme switch. Expect: smooth on the machine's GPU; no "drawn without a graphics driver" notice (that one is for software rendering only). [needs: a GPU with its driver]

## Look (R5-01..04, R5-25, R6-31)

- [ ] B-LOOK-01 Make a theme. How: Control › Appearance › Make a theme: start from a shipped theme, change the accent and background, save as "Mine". Expect: the theme appears in the list and is applied; a palette with too little contrast is refused with the reason.
- [ ] B-LOOK-02 Bring a theme from elsewhere. How: download a kitty theme `.conf` (or Alacritty `.toml`), double-click it in Files. Expect: Amethystora offers to import it; the colours come in; an `include` line is left out and said.
- [ ] B-LOOK-03 A look in one file. How: `ame desktop theme share`, then open the `.amethystora-look` file on another account. Expect: colours, wallpaper, style, icons and font come across after a question.
- [ ] B-LOOK-04 Drawn wallpapers. How: Control › Appearance › Drawn wallpapers: Facets, then Dots, Shuffle; change the accent (`ame desktop accent ...`). Expect: each drawn in the theme's colours at the screen's size; a new accent redraws the wallpaper and the desktop shows the new drawing (it stays after two minutes). [changes setting: put back with `ame desktop pattern off`]
- [ ] B-LOOK-05 Light by day, dark by night. How: `ame desktop day-night on` with a theme that has a light pair. Expect: light at the day's start, dark at night's, on time; a hand switch in between is kept until the next boundary; the shell stays responsive. [changes setting: put back with `ame desktop day-night off`]
- [ ] B-LOOK-06 A font made for reading. How: `ame desktop font "Atkinson Hyperlegible Next"`, `ame desktop mono-font "Atkinson Hyperlegible Mono"`. Expect: the interface and kitty use them at once. [changes setting: put back with the theme's own font]

## Bar and motion (R5-05..07, R5-22..24, R5-26, R6-08)

- [ ] B-BAR-01 Arrange the bar. How: Control › Top bar: drag the clock to the left, hide the meters, Reset. Expect: the bar follows at once; Reset brings GNOME's order back.
- [ ] B-BAR-02 Bar at the bottom. How: `ame desktop top-bar position bottom`. Expect: the bar along the bottom, the dock lifted above it, menus and quick settings opening upwards; windows maximise above it. [changes setting: put back with `ame desktop top-bar position top`]
- [ ] B-BAR-03 Hidden until touched. How: `ame desktop top-bar autohide on`, push the pointer against the edge. Expect: the bar slides in, goes when you leave; maximised windows get the whole height. [changes setting: put back with `ame desktop top-bar autohide off`]
- [ ] B-BAR-04 Bar size. How: `ame desktop top-bar size compact`, then `roomy`. Expect: 28 and 40 px bars, the pill and icons scaled to fit; HiDPI correct. [changes setting: put back with `ame desktop top-bar size regular`]
- [ ] B-BAR-05 Clock your way. How: `ame desktop clock format 12h`, `ame desktop clock seconds on`, `ame desktop clock zone Asia/Tokyo`. Expect: the clock and the zone ("Tokyo 06:41") beside it, ticking with the clock. [changes setting: put back with `ame desktop clock zone off`, `ame desktop clock seconds off`, `ame desktop clock format 24h`]
- [ ] B-BAR-06 Animation speed. How: `ame desktop animation-speed snappy`, then `off`. Expect: faster motion, then none; Reduce motion still wins. [changes setting: put back with `ame desktop animation-speed normal`]
- [ ] B-BAR-07 Text size. How: `ame desktop text-size 125`. Expect: text larger in the shell and apps (Amethystora's included), the screen's scale unchanged. [changes setting: put back with `ame desktop text-size 100`]
- [ ] B-BAR-08 Open and close motions. How: `ame desktop window-motion pop`, open and close a few windows. Expect: windows pop in and out; GNOME's own for dialogs. [changes setting: put back with `ame desktop window-motion gnome`]
- [ ] B-BAR-09 A slow computer is noticed. How: on a machine without a GPU driver (or under 8 GB), log in. Expect: one notice offering the light desktop, once per account, not at every login even if dismissed. [needs: a slow machine]

## Windows (R5-08..12, R6-22, R6-23)

- [ ] B-WIN-01 Drop-down terminal. How: `Super+Alt+Return`, again. Expect: kitty slides down from the top, on every workspace and above; the second press slides it away; the same window comes back.
- [ ] B-WIN-02 Scratchpad (classic layout). How: `ame desktop layout classic`, window menu › Send to the scratchpad, then `Super+Alt+Return`. Expect: the window shows and hides with the key. [changes setting: put back with `ame desktop layout tiling`]
- [ ] B-WIN-03 Keep on top. How: `Super+Alt+P` on a window. Expect: a thin accent ring and a pin; the window stays above; clicking the pin lets go.
- [ ] B-WIN-04 See-through. How: `ame desktop windows see-through on`, `Super+Alt`+scroll over a window. Expect: 40–100 % in 5 % steps, a small readout; no workspace switch. [changes setting: put back with `ame desktop windows see-through off`]
- [ ] B-WIN-05 A frame in the theme's colours. How: `ame desktop windows frame on`, move the focused window around quickly. Expect: a gradient frame round the window in use that moves with it smoothly (no flicker or lag). [changes setting: put back with `ame desktop windows frame off`]
- [ ] B-WIN-06 Window settings GNOME hides. How: Control › Windows: focus follows the pointer, new windows centred. Expect: each takes effect at once. [changes setting: put back as they were]
- [ ] B-WIN-07 A size for recording. How: window menu › Size for recording › 1920×1080. Expect: the window takes exactly 1920×1080 pixels, centred; sizes larger than the work area are greyed out.
- [ ] B-WIN-08 Reopen the app you just closed. How: close Files, press `Super+Alt+T`. Expect: Files opens again on the same workspace and place.

## Keys and typing (R5-13..15, R6-12, R6-15)

- [ ] B-KEY-01 Every shortcut in one place. How: Control › Keyboard › Shortcuts: search "previous", set Previous track to `Super+Left` with the tiling on. Expect: the clash with the tiling is shown; the shortcut keeps `Super+Left` and the media key works (the tiling gives that combination up); Reset brings GNOME's back. [changes setting: put back with Reset]
- [ ] B-KEY-02 Keyboard settings GNOME hides. How: Control › Keyboard: Caps Lock as Escape, a Compose key. Expect: both work in a text field. [changes setting: put back as they were]
- [ ] B-KEY-03 Hold a key for accents. How: `ame desktop accents on`, hold `e` in a text field, press 2. Expect: a strip é è ê ë ē above the field; 2 types è. Works with IBus on. [changes setting: put back with `ame desktop accents off`]
- [ ] B-KEY-04 Paste as plain text. How: copy formatted text from a web page, `Super+Alt+V` in a document. Expect: the text alone, no formatting; Ctrl+V unchanged.
- [ ] B-KEY-05 Each window keeps its layout. How: Control › Keyboard: per-window layouts on; switch to another layout in one window, go to another. Expect: each window keeps its own. [needs: two input sources] [changes setting: put back off]

## Tools (R5-16..19, R6-13, R6-14, R6-25)

- [ ] B-TOOL-01 Pick a colour. How: `Super+Alt+C`, click a pixel; then type `colour:` in search. Expect: a loupe; the hex copied; Shift-click rgb, Ctrl-click hsl; the last colours listed in search. HiDPI: the loupe follows exact pixels. [needs: HiDPI screen if available]
- [ ] B-TOOL-02 Mute the microphone everywhere. How: `Super+Alt+M` during a call (or with a recorder running). Expect: the mic is muted at the source, a crossed mic stays in the bar; the same key unmutes. [needs: a microphone]
- [ ] B-TOOL-03 What is using this? How: on a USB stick that will not eject (open a file on it in a terminal), Files › Scripts › What is using this?. Expect: the programs holding it, with Quit; after Quit it ejects. [needs: a USB stick]
- [ ] B-TOOL-04 Convert and shrink. How: `ame desktop files-convert on`, Files › Scripts on a photo › as WebP, half size; on a PDF › pages as pictures. Expect: new files beside the originals, nothing overwritten. [changes setting: put back with `ame desktop files-convert off`]
- [ ] B-TOOL-05 Text actions and QR. How: copy text, open the clipboard menu, UPPER CASE; copy a link, show its QR code; scan it with a phone. Expect: a new clipboard item in upper case, original kept; the QR scans to the link. [needs: a phone]
- [ ] B-TOOL-06 Share this Wi-Fi. How: quick settings › Wi-Fi › Share. Expect: a QR code only when asked; a phone joins by scanning it; hidden while the screen is shared. [needs: Wi-Fi and a phone]

## Apps (R5-27, R5-28, R6-16..18, R6-27, R6-30, R6-33)

- [ ] B-APP-01 Hide or rename an app. How: app grid › right-click an app › Rename "Mail", then Hide. Expect: the grid and search use the new name; hidden apps are gone from grid and search, still in the dock if pinned; Control › App grid shows them back.
- [ ] B-APP-02 Change an app's icon. How: right-click › Change icon › a picture. Expect: the grid and dock use it; Reset gives back the app's own.
- [ ] B-APP-03 The app grid in order. How: Control › App grid order: A–Z, by use. Expect: the grid follows, folders kept. [changes setting: put back as it was]
- [ ] B-APP-04 What starts with you. How: Control › Start-up: turn one off, log out and in. Expect: it does not start; turning it on again brings it back.
- [ ] B-APP-05 New document from a template. How: Files › right-click › New Document › Markdown. Expect: a new file from the template, in your language's Templates folder.
- [ ] B-APP-06 Downloads keeps itself tidy. How: `ame desktop tidy-downloads on`, then `ame desktop tidy-downloads now` with an old installer and an old photo in ~/Downloads; then `ame desktop tidy-downloads undo`. Expect: the installer in the Trash, the photo in a month folder, undo puts both back. [changes setting: put back with `ame desktop tidy-downloads off`]
- [ ] B-APP-07 Your backup disk, recognized. How: after `ame backup setup` on a USB disk, plug it in. Expect: "Back up now?"; Back up now runs it and says when done. [needs: a backup disk]
- [ ] B-APP-08 What can this app reach? How: app grid › right-click a Flatpak › What can it reach?. Expect: Security opens on its Apps page with that app.

## Sound and screens (R6-01..05, R6-26)

- [ ] B-SND-01 Clear voice. How: sound menu › Clear voice on, record a voice memo with music playing. Expect: the music's echo removed from the recording; the mic idle when nothing records. [needs: speakers and a microphone]
- [ ] B-SND-02 Sound presets. How: `ame desktop sound-presets on`, sound menu › Speech, then Bass. Expect: the sound changes audibly; GNOME's output list does not show the filters. [changes setting: put back with `ame desktop sound-presets off`]
- [ ] B-SND-03 Pause when the headphones come off. How: play music on headphones, unplug them (or turn the Bluetooth set off). Expect: playback pauses with a notice; Resume plays again; picking speakers by hand does not pause. [needs: headphones]
- [ ] B-SND-04 More sound sets. How: `ame desktop sounds glass`, then wood, retro. Expect: notifications and the volume tick in each set (GNOME 51's screenshot shutter stays GNOME's own). [changes setting: put back with `ame desktop sounds soft`]
- [ ] B-SND-05 External screens' brightness. How: `ame desktop screen-brightness on` (asks for the password), open quick settings. Expect: a slider per DDC/CI screen that changes it; off takes the i2c access away again. [needs: an external DDC/CI screen] [changes setting: put back with `ame desktop screen-brightness off`]
- [ ] B-SND-06 Your devices' batteries. How: connect a Bluetooth mouse or headset, open the battery menu. Expect: each device with its level; one notice at 15 % or less. [needs: a Bluetooth device with a battery]

## Phone (R5-29..31)

- [ ] B-PHN-01 Ring my phone. How: with the phone paired, phone menu › Ring, or search "ring". Expect: the phone rings even when silent. [needs: a paired Android phone with KDE Connect]
- [ ] B-PHN-02 Music from either side. How: `ame desktop phone music on`, play music on the phone. Expect: the phone's player in the pill with play, pause and next; the computer's player on the phone. [needs: a paired phone] [changes setting: put back with `ame desktop phone music off`]
- [ ] B-PHN-03 The phone as a clicker. How: `ame desktop phone clicker on`, start Present, use the phone's presenter. Expect: Next, Previous and Blank work only while presenting. [needs: a paired phone] [changes setting: put back with `ame desktop phone clicker off`]

## Restart and updates (R5-20, R5-21, R5-34, R6-24)

- [ ] B-UPD-01 Pick up where you left off. How: with apps open on several workspaces, update and Restart now. Expect: after the restart the pill offers the apps back; Open puts them on their workspaces and places.
- [ ] B-UPD-02 Restart while I'm away. How: `ame update night-restart 03:00` with an update staged, lock the screen, leave it plugged in overnight. Expect: restarted at about 03:00 only because locked and on mains; not if something inhibits logout. [changes setting: put back with `ame update night-restart off`]
- [ ] B-UPD-03 Saving data on a phone's hotspot. How: join a metered network (a phone's hotspot), wait for the update timers. Expect: the pill shows Saving data; image, Flatpak and container updates wait; Update now in Updates still runs. [needs: a metered network]
- [ ] B-UPD-04 A new network's zone. How: join a network never seen before. Expect: a card naming its firewall zone, with a way to name it home. [needs: a new network]

## Time and terminal (R6-09..11, R6-20, R6-21)

- [ ] B-TIM-01 Join from the pill. How: add a calendar event in 5 minutes with a video-call link (Calendar app or an online account). Expect: five minutes before, the pill shows it with a countdown and Join; Join opens the link. [needs: a calendar account or local calendar]
- [ ] B-TIM-02 Remind me, from search. How: search "remind me at 17:00 to call the garage", Enter. Expect: a Notes task in the inbox due 17:00; Notes asks first if it is locked.
- [ ] B-TIM-03 Time anywhere. How: search "time in Tokyo", then "15:00 Paris in Tunis". Expect: the answers inline.
- [ ] B-TIM-04 Told when a long command ends. How: in kitty, `sleep 15`, switch to another window. Expect: a notification when it ends. [changes setting: none]
- [ ] B-TIM-05 A missing command, found. How: `ame desktop missing-command on`, open a new terminal, type `cowsay`. Expect: where to get it (Homebrew, a container, Flathub). [changes setting: put back with `ame desktop missing-command off`]

## Desktop finish (R5-32, R5-33, R5-35, R6-06, R6-07, R6-19, R6-28, R6-29)

- [ ] B-FIN-01 Named spaces in the overview. How: name workspace 2 "Mail", give it a picture (tiling settings › workspace backgrounds), open the overview. Expect: each space's name along its thumbnail, its picture under its windows.
- [ ] B-FIN-02 The digest's lists. How: digest on (`ame desktop digest on`), `ame desktop digest through` for one app and `wait` for another; get notifications from both (real apps, e.g. Thunderbird and a chat app). Expect: the one let through shows at once, the other only in the digest, even with the digest off. [changes setting: put back with `ame desktop digest through reset`, `ame desktop digest wait reset` and `ame desktop digest off`]
- [ ] B-FIN-03 "+3" on the Downloads stack. How: download three files. Expect: a "+3" on the dock's Downloads stack, gone once opened.
- [ ] B-FIN-04 A resting screen. How: `ame desktop resting-screen on`, leave the machine idle until just before the lock. Expect: a large clock and the wallpaper drifting, windows hidden; any key or move brings the desktop back; the lock comes as before. [changes setting: put back with `ame desktop resting-screen off`]
- [ ] B-FIN-05 Living wallpapers. How: `ame desktop living-wallpaper aurora` (or facets, rain, dusk). Expect: drawn only while visible, paused on battery and while a window covers it; the GPU stays near idle. [changes setting: put back with `ame desktop living-wallpaper off`]
- [ ] B-FIN-06 Trim a recording. How: record the screen (`Ctrl+Shift+Alt+R`), stop, the notice's Trim: cut the start and end, save as WebM. Expect: the trimmed file plays; no file is left in `/run/user/$UID/` afterwards (the lab saw one left).
- [ ] B-FIN-07 Dock size and glass strength. How: `ame desktop dock size 64`, `ame desktop glass-strength blur 150`. Expect: bigger icons, stronger blur; readable. [changes setting: put back with `ame desktop dock size reset` and `ame desktop glass-strength reset`]
- [ ] B-FIN-08 A widget gallery. How: Control › Widgets › Add: the clock, then now playing. Expect: each on the bar (or desktop) at once, working.

## Later (R5-36..38, R6-32, R6-34..36)

- [ ] B-LAT-01 Click anything with the keyboard. How: `Super+Alt+K` in Settings, type the letters of a button. Expect: letters on every button and link; typing them clicks that button and nothing is typed into a text field (the lab saw the letter go into the field: check here first). Escape cancels.
- [ ] B-LAT-02 Search files by meaning. How: `ame desktop search meaning on`, wait for the index, search "letter about the tax refund" for a document worded differently. Expect: the file found; nothing leaves the machine. [changes setting: put back with `ame desktop search meaning off`]
- [ ] B-LAT-03 Dictation. How: `ame desktop dictation on`, click into a text field, `Super+Ctrl+Shift+D`, speak a sentence, the key again. Expect: the words typed into the field; a password field refuses dictation (with IBus running). [needs: a microphone] [changes setting: put back with `ame desktop dictation off`]
- [ ] B-LAT-04 Captions in your language. How: `ame desktop captions language fr` (downloads the French model after a question), play a French video. Expect: French captions. [needs: network] [changes setting: put back with `ame desktop captions language en`]
- [ ] B-LAT-05 Notes and bookmarks in search. How: `ame desktop search notes on` with Notes unlocked; `ame desktop search bookmarks on`; search a note's word and a bookmark's title. Expect: both found; nothing from Notes while it is locked. [changes setting: put back off]
- [ ] B-LAT-06 A guest login that forgets. How: `ame security guest-login on`, log out, log in as Guest at the login screen, make a file, log out, log in as Guest again. Expect: a fresh guest each time, nothing kept; sudo, polkit and SSH refuse the guest. [changes setting: put back with `ame security guest-login off`]

## Left open by the lab

What the VM lab could not settle on 2026-11-01, each to be seen here first (details in `plans/ui-ideas/work/test-*.md`):

- [ ] B-OPEN-01 The Manual's moving cards. How: a new account's first login, `Super+F1`, a page with cards (Getting around). Expect: the cards play while the Manual is in front (the lab saw them stay paused in a new account; they play by hand).
- [ ] B-OPEN-02 The theme's reveal at login. How: log out and in twice. Expect: the Transitions reveal at each login, none when animations are off.
- [ ] B-OPEN-03 Quick settings after a lock. How: lock and unlock, open quick settings, the volume and the pill. Expect: all work; no "already disposed" lines for pill/clock.js in the shell's log.
- [ ] B-OPEN-04 The bar's menus after many locks. How: lock and unlock ten times, open each bar menu. Expect: each opens once, with its items once; nothing duplicated.
- [ ] B-OPEN-05 Search finds pictures in Files. How: put a picture in ~/Pictures, wait a minute, search its name. Expect: Files' results list it.
- [ ] B-OPEN-06 Multi-screen bars and dock. How: with two screens, a fullscreen window on screen 2, then push against its top; maximise on screen 1. Expect: screen 2's bar hides and peeks; screen 1's dock hides for the maximised window. [needs: two screens]
- [ ] B-OPEN-07 The installer and the sealed image. How: install the beta from its ISO, and once from the live ISO's sealed installer, on a spare disk. Expect: both start, ask the LUKS passphrase, and sign the account in. [needs: a spare disk]
