# Round 7: with other people

Walk on a real machine running an image with these features, as a person would. The spec is
`plans/ui-ideas/r7/spec-people.md`. Where an item changes a setting, put it back with the command in its flags. A real lock
screen, a real camera and a real login screen exist only here: the harness had none of them.

## R7-64 Hand it over

- [ ] R7-PEOPLE-01 Offered. How: right-click a browser window's title bar. Expect: **Hand this over…** above Close; the dialog names the app, says "This is a courtesy, not a lock" and names a guest login; **Not now** changes nothing.
- [ ] R7-PEOPLE-02 Held. How: **Hand it over** on a window, tiling and classic layout. Expect: the window full screen; a chip at the top "Only Firefox · hand it back with your password", which dims after 5 s and comes back under the pointer; `Super`, `Alt+Tab`, `Super+Return`, `Super+Space`, `Print` and `Super+Alt+T` do nothing visible and reach the app; volume keys still work.
- [ ] R7-PEOPLE-03 Nothing leaves it. How: while held, F11 in the browser; a three-finger swipe; send a notification with `notify-send hello`; open a link from a terminal window that was already running elsewhere. Expect: the window stays full screen, the workspace comes back, no banner shows, focus returns to the window.
- [ ] R7-PEOPLE-50 The top edge, Today and the menus. How: while held, with the bar's peek on, push the pointer against the top edge; on a touchpad swipe four fingers left; press `Super+V`; in Looking Glass `Main.panel.statusArea.quickSettings.menu.open()` then `.close()`, then `notify-send hello`. Expect: no bar comes down, no calendar, notification list or power menu opens, and no banner shows after the menu has closed either.
- [ ] R7-PEOPLE-51 Over a full-screen video. How: hand over a window that is playing a video or a game full screen, on a real graphics card. Expect: the chip is drawn over it and clicking it locks. [needs: a real GPU]
- [ ] R7-PEOPLE-52 The keys that end it. How: while held, `Super+L` with an Arabic, Russian or Greek layout active; again after setting the lock key to `Super+K` in Settings, Keyboard; again in a window that takes the shortcuts (a virtual machine or remote desktop viewer), after `Super+Esc`. Expect: it locks each time; the volume, mute and brightness keys work, and the calculator, mail, browser and search keys do nothing; the dialog said what an app that takes the keyboard can do, and the chip is the way out there.
- [ ] R7-PEOPLE-53 Another window, another screen. How: two windows of the app on two screens, hand one over, close it. Expect: the other becomes the held one, and the black cover is on the screen it is not on. [needs: two screens]
- [ ] R7-PEOPLE-04 Its own windows. How: while held, Ctrl+N and Ctrl+O in the browser. Expect: the app's new window and its file dialog work; closing the held window leaves the other full screen.
- [ ] R7-PEOPLE-05 Ending. How: click the chip, unlock with the password; again with `Super+L`. Expect: a real lock screen, then the desktop as before, the window back to its old size, and the banner that waited shows. [needs: a password on the account]
- [ ] R7-PEOPLE-06 Closing ends it. How: hand over, close the app's last window. Expect: the screen locks.
- [ ] R7-PEOPLE-07 Two screens. How: hand over with a second screen on. Expect: the other screen is black and takes no clicks; unplugging it leaves the held window alone. [needs: two screens]
- [ ] R7-PEOPLE-08 Off. How: `ame desktop windows hand-over off`. Expect: no line in the window menu, and the shortcuts as ever. Put back: `ame desktop windows hand-over on`.
- [ ] R7-PEOPLE-09 Where no lock can end it. How: `gsettings set org.gnome.desktop.lockdown disable-lock-screen true`. Expect: no line in the window menu. Put back: `gsettings reset org.gnome.desktop.lockdown disable-lock-screen`.

## R7-67 Hide what's private

- [ ] R7-PEOPLE-10 Covers. How: open a page or text file with an e-mail address, a card number (4111 1111 1111 1111), a phone number and an IBAN (GB82 WEST 1234 5698 7654 32); `Print`, frame it, press the button beside Pin. Expect: a card at once ("Reading the picture…"), then solid black boxes over each, and "4 things covered. Check before you send." with a line each beneath it: an address, a card number, a phone number, an account number.
- [ ] R7-PEOPLE-11 Never a blur. How: **Copy**, paste into an image editor and zoom in. Expect: flat black, with nothing of the digits. The same with the capture button and `Ctrl+S`.
- [ ] R7-PEOPLE-12 Uncover. How: **Uncover**. Expect: the boxes go. Press the button again; pick one box with the Move tool and delete it. Expect: only that one goes.
- [ ] R7-PEOPLE-54 A window and a recording. How: with boxes on, press the **Window** tab, or the recording switch. Expect: the card and the boxes are gone (a capture then claims nothing); back on Selection, the button reads again.
- [ ] R7-PEOPLE-55 Taking it while it reads. How: press the button and, at once, the capture button or Enter. Expect: no picture is taken; the card says "Still reading the picture. Take it once the boxes are on."; once the boxes are on, the capture takes them.
- [ ] R7-PEOPLE-56 Pressing twice. How: with boxes on, press the button again, and again while it reads. Expect: the old boxes stay while it reads and are replaced by the new reading; a press while it reads does nothing.
- [ ] R7-PEOPLE-57 A card, or an account, with words beside it. How: a page with `4111 1111 1111 1111 12/25 123` and, on another line, `DE89 3704 0044 0532 0130 00 BIC COBADEFF`. Expect: the card number and the account number are covered.
- [ ] R7-PEOPLE-58 Nothing left running. How: press the button on a large area and close the UI at once (Escape); a moment later `pgrep -a tesseract; pgrep -af amethystora-cover-private`; while it reads, `grep core /proc/$(pgrep -f amethystora-cover-private | head -1)/limits` and `systemd-cgls --user | grep run-`. Expect: nothing is left; the core limit is 0; the helper is in a scope of its own, not GNOME Shell's.
- [ ] R7-PEOPLE-59 The bar's width. How: with Gradia installed (the default), Pin and Hide what's private on, open the bar; then `ame desktop screenshot pin off` and open it again. Expect: four buttons on the right and none under the shutter, then three, with the bar as GNOME draws it.
- [ ] R7-PEOPLE-13 Not private. How: a page with dates, prices and version numbers only. Expect: "Nothing found to cover. It can miss things, so check before you send."
- [ ] R7-PEOPLE-14 Dense screen and dark themes. How: the same on a 200 % scale, and on a dark and a light theme. Expect: the boxes sit on the text. [needs: a dense screen]
- [ ] R7-PEOPLE-15 Nothing kept. How: `Print` a window showing `canary.people67@example.org`, press the button, then close the UI without taking it. `grep -rl canary.people67 ~ /tmp "$XDG_RUNTIME_DIR" 2>/dev/null; journalctl --user --since -5m | grep canary.people67`. Expect: nothing found.
- [ ] R7-PEOPLE-16 Off. How: `ame desktop screenshot private off` while the UI is open, then reopen. Expect: no button, and the screenshot bar as before. Put back: `ame desktop screenshot private on`.

## R7-65 A desktop for your parents

- [ ] R7-PEOPLE-17 On. How: `ame desktop windows simple-desktop on`, or Control. Expect: at once, four big tiles (the internet, email, photos, files, each with the app this account uses), large text, a larger pointer, the top bar plain with **Home**, a notice saying `Super+Alt+Home` goes back.
- [ ] R7-PEOPLE-18 One window at a time. How: click a tile; open another. Expect: each opens maximized; **Home** puts them down and shows the tiles; a tile of a running app brings it back; `Super` alone opens nothing.
- [ ] R7-PEOPLE-19 Log in again. How: log out and in, and lock and unlock. Expect: it stays, starting on the tiles rather than the overview.
- [ ] R7-PEOPLE-20 Goes back. How: first change the text size by hand to 110 and the pointer's size, then turn it on, `Super+Alt+Home`. Expect: the full desktop, tiling back, the bar back, the text at 110; a key you changed while it was on stays yours.
- [ ] R7-PEOPLE-60 Turned off while it was not running. How: with it on, lock the screen, then from a text console or SSH `ame desktop windows simple-desktop off`, and unlock. Expect: within seconds the text size, pointer, tiling, `Super` and the bar are as they were before.
- [ ] R7-PEOPLE-61 Nowhere to keep it. How: `chmod 500 ~/.local/state/amethystora`, then turn it on. Expect: a notice that it could not be turned on, nothing changed and the switch off. `chmod 700` it back.
- [ ] R7-PEOPLE-62 Reset while on. How: `ame system reset-settings account` with it on, then `ame system reset-settings undo account`. Expect: both leave the simple desktop off and the account's own text size and pointer as the reset saved and undo brought back; no `~/.local/state/amethystora/simple.json` is left.
- [ ] R7-PEOPLE-63 A smaller window stays so. How: with it on, make a window smaller, then lock and unlock. Expect: it keeps its size. Control's **The simple desktop's tiles**, **Choose…**, asks in a terminal for launchers.
- [ ] R7-PEOPLE-21 Own tiles. How: `ame desktop windows simple-tiles org.gnome.Calculator.desktop org.gnome.TextEditor.desktop`. Expect: those two, live; none: `ame desktop windows simple-tiles`.
- [ ] R7-PEOPLE-22 In the setup file. How: with it on, `ame setup save`; on a second account, `ame setup apply`. Expect: it asks, then the second account is simple too; the first is unchanged by it.
- [ ] R7-PEOPLE-23 Another account. How: log in as another account with it on in the first. Expect: nothing changed there.

## R7-71 If it's lost

- [ ] R7-PEOPLE-24 Writing it. How: `ame system if-found`. Expect: it says who can read it and that it adds no way in, asks, then asks an administrator's password. Control's **If it's lost, it says whose it is** opens the same in a terminal.
- [ ] R7-PEOPLE-25 Lock screen. How: `Super+L`, wake the screen. Expect: the line at the bottom of the screen (an account that was already logged in shows it from its next login, since dconf tells a running session of nothing new in the system's database), on the clock's view and on the password prompt, in every account; it is not covered by the prompt or the on-screen keyboard's place. A line with `<b>` shows it as text.
- [ ] R7-PEOPLE-64 Only the administrator's words. How: as the account, `gsettings set org.gnome.shell.extensions.amethystora-ui.found line 'not the owner'` (with the extension's schemas), then lock. In the agent, ask it to run `ame system if-found hello` and `amethystora-if-found clear`. Expect: the lock screen shows the administrator's line or none, never that one; both commands are refused by the agent's tools.
- [ ] R7-PEOPLE-26 Login screen. How: log out, or restart. Expect: GDM's banner shows the line. [needs: GDM]
- [ ] R7-PEOPLE-27 Encrypted disk. How: restart a machine with an encrypted disk. Expect: the disk's prompt shows no line, as the manual says, and the login screen does.
- [ ] R7-PEOPLE-28 Off. How: `ame system if-found off`. Expect: gone from the login screen, and from an account's lock screen at its next login. `ls /etc/dconf/db/*.d/ | grep if-found` finds nothing. `ame system reset-settings` keeps a line that is set.

## R7-44 How do I look and sound?

- [ ] R7-PEOPLE-29 In the card. How: an event in Calendar within five minutes with a video-call link; open the pill's card; **How do I look and sound?** Expect: the card closes and a small window opens at the top in the middle, on top, floating in tiling too, with the camera, its light on, and a moving level as you speak. [needs: a camera, and a calendar event]
- [ ] R7-PEOPLE-30 What it says. How: with a Bluetooth headset as output only, and then as input. Expect: "Your headset's would lower your sound", and then "Using it lowers your headset's sound"; muted, "Nothing is heard" after five seconds. [needs: a Bluetooth headset]
- [ ] R7-PEOPLE-31 It closes. How: close the window; again and lock the screen; again and start a screen share; again and wait three minutes. Expect: each time the window goes and the camera's light goes out at once.
- [ ] R7-PEOPLE-65 Which camera. How: on a laptop with an infrared sensor beside its camera, or with v4l2loopback loaded and a virtual camera first. Expect: the colour camera is shown, never the infrared or virtual one; with only those, "No camera was found". [needs: an infrared camera or v4l2loopback]
- [ ] R7-PEOPLE-66 It leaves. How: while open, `pkill -TERM -f amethystora-mirror`; and `pkill -STOP -f amethystora-mirror` first, then the close. Expect: the light goes out at once the first time, and the second time the process is ended four seconds later.
- [ ] R7-PEOPLE-32 While shared. How: start a screen share, then open the meeting's card. Expect: no **How do I look and sound?**
- [ ] R7-PEOPLE-33 Blocked. How: `ame security block-webcam on`, restart if it asks, then open it. Expect: "No camera was found", no light. Put back with `off`. [needs: a camera]
- [ ] R7-PEOPLE-34 Nothing kept. How: while open, watch `lsof -p $(pgrep -f amethystora-mirror)`; afterwards `find ~ /tmp "$XDG_RUNTIME_DIR" -newermt '-5 minutes' -type f 2>/dev/null`. Expect: no recording, nothing new from it.
- [ ] R7-PEOPLE-35 Off. How: `ame desktop pill mirror off`. Expect: the meeting's card as before. Put back: `ame desktop pill mirror on`.

## R7-30 Your year

- [ ] R7-PEOPLE-36 The page. How: `ame desktop your-year now` as an administrator and as a standard account. Expect: the days since the install (not since the last update: check after an update, and that `stat -c %W /var` is the install's date), snapshots, the last backup and what the watch told you "lately"; no count of updates; lines it cannot read are left out for the standard account; "Worked out here. Nothing was sent."
- [ ] R7-PEOPLE-37 The card. How: set the date to 20 December, or in Looking Glass: `Main.panel.statusArea.dateMenu` is opened after `Main.extensionManager.lookup('amethystora-ui@arsslensoft.github.io').stateObj.modules.get('today').year.offerForTest(2026)`. Expect: a card first in Today; **Open** shows the page at once and fills it in within seconds; closed, the card has gone until next year (`your-year-seen`). **Not this year** the same.
- [ ] R7-PEOPLE-38 Nothing kept. How: after opening it, `grep -rl "days\b" ~/.local/state/amethystora 2>/dev/null; gsettings list-recursively org.gnome.shell.extensions.amethystora-ui.today` (with the extension's schemas). Expect: only the year in `your-year-seen`.
- [ ] R7-PEOPLE-39 Off. How: `ame desktop your-year off`. Expect: no card, and the menu as before. Put back: `ame desktop your-year on`.
