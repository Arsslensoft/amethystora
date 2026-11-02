# Round 7: the live pill

Walk on a real machine with the beta image. The spec is `plans/ui-ideas/r7/spec-pill.md`. Unless an item says otherwise the machine is in its shipped state;
where an item changes a setting, put it back with the command in its flags. Mark each line `→ PASS` or `→ FAIL: what was seen`. Throughout: `journalctl --user -b -o cat _COMM=gnome-shell | grep -E "JS ERROR|Amethystora UI"` names nothing of ours at the end. The canary check for Memory ideas: `grep -rIl CANARY ~ /tmp /var/tmp "$XDG_RUNTIME_DIR" 2>/dev/null; journalctl --user -b | grep -c CANARY`: nothing, and 0.

## With one activity it is as it was

- [ ] R7-PILL-01 One activity. How: start music alone; then a timer alone; then nothing. Expect: exactly the pill as before, centred in the bar, no bubble; with nothing, GNOME's clock; a click opens its card; a right-click the calendar.
- [ ] R7-PILL-02 A call and a recording win. How: start music and a timer, then start a screen recording (`Super+Ctrl+Alt+Shift+R`, or a call on a paired phone). Expect: the recording alone in the pill, no bubble; music and timer are rows of its card; when it ends they come back as they were. [needs: for the call, a paired phone]

## R7-16 Two at once

- [ ] R7-PILL-03 Split. How: music playing, then click 5 min under Start a timer in the calendar. Expect: the timer in the pill, the music as a small bubble beside it with the app's icon and a few characters; pill and bubble part in one motion and the whole stays centred; the top bar does not jump.
- [ ] R7-PILL-04 Each has its own card. How: click the pill, then the bubble. Expect: the timer's card, then the music's; a second click on the bubble closes it. The time left and the title are still right.
- [ ] R7-PILL-05 Rejoin. How: cancel the timer. Expect: the bubble slides back into the plate as it fades and the music takes the pill, in one motion.
- [ ] R7-PILL-06 A mark is no bubble. How: music playing, and a Focus profile on. Expect: no bubble; the card lists the profile.
- [ ] R7-PILL-07 Right to left. How: log in under Arabic with music and a timer. Expect: the bubble at the pill's left; a click on it still opens its card.
- [ ] R7-PILL-08 Off. How: `ame desktop pill two-at-once off` with music and a timer. Expect: only the timer shows, the music waits in its card, as before. [changes setting: put back with `ame desktop pill two-at-once on`]

## R7-17 It feels like it has weight

- [ ] R7-PILL-09 The spring. How: start and stop a timer a few times, quickly; open and close its card. Expect: the pill overshoots a little and settles, and a second change while it moves carries on from where it is, no jump, no stutter on this GPU.
- [ ] R7-PILL-10 It gives. How: press on the pill. Expect: it gives a little under the press and comes back.
- [ ] R7-PILL-11 Banners grow out of it. How: `notify-send Hello there`. Expect: the banner grows from under the pill and goes back in. Move banners off the middle (the dock's setting, or any way you have) and it is GNOME's own slide.
- [ ] R7-PILL-12 Reduce motion rules. How: Settings › Accessibility › Reduce Animation on; repeat. Expect: crossfades only, no travel, no give; `ame desktop animation-speed` changes the spring's time too; `ame desktop light-desktop on` has the plain curve.
- [ ] R7-PILL-13 Off. How: `ame desktop pill weight off`. Expect: the old plain motion, GNOME's banner, no give. [changes setting: put back with `ame desktop pill weight on`]

## R7-22 A timer you can feel

- [ ] R7-PILL-14 The ring. How: start 1 min. Expect: a ring round the pill itself drains once a second from the top, clockwise; it does not lag the plate. Fullscreen a window for 10 s, come back: right time, right ring.
- [ ] R7-PILL-15 Two side by side. How: search (`Super+Space`) `tea 4 min`, Enter; then `oven 1 min`, Enter. Expect: Tea in the pill with its name, Oven's time in the bubble with its own ring. A third: search has no row.
- [ ] R7-PILL-16 The end. How: let one end. Expect: the alarm's sound, a notification that says Oven is done, a gentle glow on the pill; under Reduce Animation one quiet rise and fall, no pulsing; the other timer goes on.
- [ ] R7-PILL-17 A lock. How: start 1 min, lock, wait two minutes, unlock. Expect: it rings at the unlock and says when it ended, as it always did.
- [ ] R7-PILL-18 Unseen. How: start a timer, then a screen recording, which keeps the place alone, for a minute. Expect: when the recording ends the timer shows the right time at once, and it rang on time if it ended meanwhile.
- [ ] R7-PILL-19 Off. How: `ame desktop pill timer off`. Expect: the small ring, one timer, `tea 4 min` in search gives nothing. [changes setting: put back with `ame desktop pill timer on`]

## R7-20 Everything moving

- [ ] R7-PILL-20 A download. How: download a large file in Firefox. Expect: its name in the pill with a whole ring, gone when done; its name is not in any file (canary: rename the download CANARY...).
- [ ] R7-PILL-21 A copy in Files. How: copy a large folder in Files, on GNOME 51. Expect: Files' own words (Copying Files) in the pill, gone when done. If nothing shows, read the app id gnome-session was given: while the copy runs, `for i in $(gdbus call --session --dest org.gnome.SessionManager --object-path /org/gnome/SessionManager --method org.gnome.SessionManager.GetInhibitors | grep -o "/org/gnome/SessionManager/Inhibitor[0-9]*"); do gdbus call --session --dest org.gnome.SessionManager --object-path $i --method org.gnome.SessionManager.Inhibitor.GetAppId; done`. It must be `org.gnome.Nautilus` (or with `.desktop`); write down what it is. An empty one means the portal passes none and Files is not seen, on purpose (the reason of a program with no name is never shown as Files'): then the id to use is a finding to put in `FILES` of `pill/moving.js`. Also: a program that asks for sleep without a name (`gnome-session-inhibit --reason "Copying secrets" --inhibit suspend sleep 60`) shows nothing in the pill. [needs: a large folder]
- [ ] R7-PILL-22 A Flatpak update. How: `flatpak update` something large, or Bazaar. Expect: the app's name, and its dock icon grey as before.
- [ ] R7-PILL-23 A backup. How: Backups › Back up now. Expect: Backup among what moves until it ends.
- [ ] R7-PILL-24 The phone. How: send a large file to a paired phone. Expect: the ring fills by the bytes; `N moving · 45%` with something else moving; the card says To your phone. A file the phone sends here is not in the pill (the program tells the shell of files going out only). [needs: a phone]
- [ ] R7-PILL-25 Sharing. How: start a download, wait until its name is in the pill, then share the screen (and again for a recording). Expect: at once `1 moving`, no file name, in the pill, and none in its card (it says Downloads); when the sharing ends the name is back.
- [ ] R7-PILL-26 Off. How: `ame desktop pill moving off`. Expect: nothing of them in the pill, and the dock's grey and count as before; `gnome-session-inhibit --list` has nothing of the shell's. [changes setting: put back with `ame desktop pill moving on`]
- [ ] R7-PILL-55 Nothing sticks. How: (a) start a big copy in Files and kill Files (`pkill nautilus`) before it ends; (b) send a file to the phone and `ame desktop phone off` while it goes; (c) leave a download at 90 % by stopping the connection. Expect: (a) the pill drops it at once, or with the answer of gnome-session within a second; (b) the item leaves the pill as the phone link stops; (c) it goes five minutes after the last write; and in every case `gnome-session-inhibit --list` no longer shows the shell's inhibitor and the machine suspends by itself again. A copy that really takes more than twelve hours is dropped at the twelfth, and the machine is not kept awake more than twelve hours in a stretch.
- [ ] R7-PILL-56 Moving over a lock. How: start a Flatpak update that takes minutes (`flatpak update`, or Bazaar), lock the screen at once, wait three minutes, unlock. Expect: while locked `gnome-session-inhibit --list` shows the shell's inhibitor; after the unlock the update is still in the pill if it is still going. With `ame desktop pill moving off` during a lock the inhibitor is gone at the unlock. [changes setting: put back with `ame desktop pill moving on`]

## R7-56 It doesn't sleep on your copy

- [ ] R7-PILL-27 Stays awake. How: Settings › Power: suspend after 15 minutes idle. Start a download that takes longer, leave it. Expect: the screen goes dark and locks, the machine does not suspend while it comes, and suspends by itself afterwards. The card said so. [needs: time]
- [ ] R7-PILL-28 The lid. How: a copy running, close the lid. Expect: it sleeps. [needs: a laptop]
- [ ] R7-PILL-57 Suspend is still yours. How: a download running (the card says it stays awake), then the system menu › Suspend (or `systemctl suspend`, as you). Expect: it suspends at once and asks for no password; `systemd-inhibit --list` meanwhile shows gnome-session's lock with the mode `block-weak`, which logind ignores for your own request (gnome-session 51 and systemd 262 on the beta image both carry that mode). If it asks for a password the answer is a FAIL.
- [ ] R7-PILL-29 Low battery. How: run down to 16 % with a download. Expect: at 14 % the pill drops "staying awake" and the machine may sleep; plug in, and with the download still going it stays awake again (the pill says so). [needs: a laptop]
- [ ] R7-PILL-30 Off. How: `ame desktop pill stay-awake off`. Expect: `gnome-session-inhibit --list` shows nothing of the shell's for it, and the machine suspends as usual. [changes setting: put back with `ame desktop pill stay-awake on`]

## R7-19 Hello, headphones

- [ ] R7-PILL-31 Headphones. How: connect Bluetooth headphones. Expect: two seconds of icon, name and battery on an accent plate; then the pill as it was. The card says "sound moved here" if it did. [needs: Bluetooth headphones]
- [ ] R7-PILL-32 A phone, a controller, a dock. How: each arriving. Expect: the same for each. [needs: each]
- [ ] R7-PILL-33 Quiet. How: connect them during a recording, in Focus, and in a fullscreen video. Expect: nothing. Lock and unlock with them connected: nothing. A mouse waking: nothing.
- [ ] R7-PILL-34 Off. How: `ame desktop pill hello off`. Expect: nothing arrives. [changes setting: put back with `ame desktop pill hello on`]

## R7-18 Drop things on it

- [ ] R7-PILL-35 It opens. How: drag a file from Files up to the pill. Expect: a row of tiles under it, Drop name.pdf in the pill; drag away and it closes; Esc ends it with nothing done.
- [ ] R7-PILL-36 Each target. How: drop on each: Copy without hidden data (a `name (clean).ext` beside it); Convert, with `ame desktop files-convert on` (a card asks which of six); Shelf, with `ame desktop shelf edge`; Send to phone [needs: a phone]; Ask the agent, with `ame agent toggle on`. Expect: each does what it does from its own place.
- [ ] R7-PILL-37 Only what is on. How: all of those off. Expect: only Copy without hidden data.
- [ ] R7-PILL-38 Not files. How: drag text from a browser. Expect: nothing opens; the Shelf and the dock's drops work as before.
- [ ] R7-PILL-39 Off. How: `ame desktop pill drop off`. Expect: nothing opens. [changes setting: put back with `ame desktop pill drop on`]

## R7-21 Music you can see

- [ ] R7-PILL-40 The colour. How: play albums with strong covers in a player that gives a local cover. Expect: the pill and its card take the colour; text reads; a grey or pale cover stays on the theme's plate; another track changes it once.
- [ ] R7-PILL-41 The bars. How: `ame desktop pill music-bars on` (it is off as shipped, and only the maintainer turns it on by default), music playing in the pill. Without it: four beats, no `pw-record`. Expect: bars follow the sound; silence is low; `pw-link -l` shows one link from that player's stream; nothing else records; no microphone dot, no GNOME microphone icon. **The limit is 1 % of one core**: measure over a minute with `pidstat -p $(pgrep -nx gnome-shell),$(pgrep -nx pw-record) 60 1`, with `ame desktop pill music off` as the baseline, and write down both numbers; over 1 % together is a FAIL, whatever else looks right (the lab renders in software and cannot say: there `pw-record` took 0.4 % for an ordinary player and 1.2 to 1.45 % for one at 256 samples, which is its own wake for every quantum; the shell's reading cost about 0.1 %, and its drawing is what a GPU must show to be small. Also try a quick player (a low-latency app) and write it down; the number decides whether `music-bars` may ever default to on. The meter is not skipped for a quick player: nothing in the shell says the graph's quantum cheaply) [changes setting: put back with `ame desktop pill music-bars off`]. Bubble or pause: `pgrep pw-record` finds nothing; so with the screen blanked (`xset`-less: wait for the idle blank, or `busctl --user set-property org.gnome.Mutter.DisplayConfig /org/gnome/Mutter/DisplayConfig org.gnome.Mutter.DisplayConfig PowerSaveMode i 3` and back to 0), and with the computer on its battery (the bars are the four beats; the colour and the scrub stay). The shell reads the stream about seven times a second, whatever the player's buffer (`strace -c -f -p $(pgrep -nx gnome-shell) -e trace=read 10` shows it, not forty-seven a second). A stream that is gone when the meter links ends it: with `pw-link -l` nothing links to the microphone (`build_files/shared/test-levels.sh` held this against a private PipeWire). [needs: a player with sound, a real GPU]
- [ ] R7-PILL-42 Scrub. How: drag the position in the card. Expect: the player goes there; a player that cannot seek keeps the line.
- [ ] R7-PILL-43 Reduce motion and saving. How: Reduce Animation, or power saving. Expect: no `pw-record`; the bars as before.
- [ ] R7-PILL-44 Off. How: `ame desktop pill music off`. Expect: theme colours and the line, whatever `music-bars` says (the bars are `music-bars`'s alone). [changes setting: put back with `ame desktop pill music on`]

## R7-23 A code from your phone

- [ ] R7-PILL-45 It asks first. How: `ame desktop phone codes on`. Expect: it says it reads each text, and asks. Control opens it in a terminal.
- [ ] R7-PILL-46 A code. How: have a text sent to the paired phone: `Your code is 482913`. Expect: Copy 482913 in the pill; the chip copies; it goes in 60 s; the clipboard is empty of it a minute after copying. **Unknown until now: whether a real phone tells the program of a new text by itself** (the `kdeconnect.sms.messages` packet for one new message, spec-plugins 8): if no code ever shows, run `dbus-monitor --session "type='signal',interface='org.amethystora.Connect.Device',member='MessagesChanged'"` while a text arrives; no signal means the phone does not push, and the answer is the finding. A second code while one shows does not replace it. Copy the same code twice in a minute: the clipboard still holds it after the first take-back would have run. [needs: a phone] [changes setting: put back with `ame desktop phone codes off`]
- [ ] R7-PILL-47 Nowhere else. How: after copying, open Amethystora Clipboard's list and `clip:` in search; with `ame desktop phone clipboard send on`, look on the phone; canary with a code you know. Expect: not in the history, not on the phone, not in the home, `/tmp` or the journal. [needs: a phone]
- [ ] R7-PILL-48 Hidden. How: a text arrives while the screen is shared; while locked; start sharing while one shows. Expect: nothing shown, and one showing goes; copy a code and lock the screen at once: after the unlock the clipboard is empty of it. [needs: a phone]
- [ ] R7-PILL-49 Other texts. How: `See you at 1830`, a price, a phone number, `Order 482913 has shipped`, `Pay 500000 before Friday`, `Call 98765432 for the code`, `Promo code SAVE20: get 1000 points`, `Il tuo codice è 482913` (a language that is not listed). Expect: nothing. A code needs a code word (English, French, German, Spanish, Arabic) beside it. [needs: a phone]
- [ ] R7-PILL-50 Off. How: with it off, a text arrives. Expect: nothing; `dbus-monitor --session` shows no `Code` signal and no `ListMessages` from the shell ever, on or off (the shell reads no texts; the program refuses the call). The agent cannot turn it on: `gsettings set org.amethystora.Connect codes true`, `dconf write /org/amethystora/Connect/codes true` and `dconf load` of its folder are refused by the agent's tools. [needs: a phone]

## Smaller things

- [ ] R7-PILL-63 After a suspend. How: start a timer for 3 minutes with a recording running (so the timer is not seen), suspend by the system menu for 4 minutes without locking (Settings › Privacy: automatic screen lock off), wake. Expect: it rings within two seconds of the wake, not minutes later. With `ame desktop pill live off` the same. [needs: time]
- [ ] R7-PILL-64 Churn in Downloads. How: `while :; do touch ~/Downloads/a.part; sleep 0.05; rm ~/Downloads/a.part; sleep 0.05; done` for ten seconds, with `perf`/the Performance HUD. Expect: the pill appears and changes at most once a second, no flicker. `dbus-monitor --session` shows no `Transfer` match rule from the shell with `ame desktop pill moving off`.
- [ ] R7-PILL-58 A banner updated while it shows. How: `notify-send -r 4242 Hello one`, then `notify-send -r 4242 Hello two` and again while the first still shows. Expect: the first grows out of the pill; the updates change its words in place, no jump to a small banner each time.
- [ ] R7-PILL-59 Time added to a timer you cannot see. How: with a recording running (the timer is then behind it), start 1 min, add 5 minutes in the calendar's row, and watch the clock. Expect: it rings after six minutes, not after one. Disable and enable the pill (lock and unlock) with two timers: neither rings twice or too early.
- [ ] R7-PILL-60 Timer words. How: search `tea 4 constructor`. Expect: no timer row.
- [ ] R7-PILL-61 A drag that jumps. How: drag a file to the pill so that the tiles show, then fling the pointer far away in one movement. Expect: they close, and `journalctl --user -b | grep -c "CRITICAL"` shows nothing new from the pill.
- [ ] R7-PILL-62 With a flat battery's activity in the union (after the merge with the sturdy group). How: let the battery fall to its last percent. Expect: the flat plate first; a call or a recording started meanwhile still comes before it and alone; Present sits beside it as a bubble.

## Costs

- [ ] R7-PILL-53 Idle. How: nothing happening for ten minutes, with GNOME's Performance HUD or `perf top`. Expect: no frames and no wakeups from the pill.
- [ ] R7-PILL-54 Locks. How: music and a timer, 50 locks by script or hand. Expect: GNOME Shell's memory up by at most 2 MB; no `JS ERROR`; the pill comes back as it was.
