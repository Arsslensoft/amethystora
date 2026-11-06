# Round 7, later: Sign a PDF, Commands show progress, Any language, Look good in every call, Your photos, Another screen, Help from someone you trust

Walk on a real machine running the image with round 7's second wave, as a person would. The spec is `plans/ui-ideas/r7/spec-later.md`.
Mark each line as the other files do: `→ PASS`, `→ FAIL: what was seen`, `→ NOT DONE: needs ...`. Everything here is offered and off
as the image ships, except Sign a PDF's entry in Files, which is there for everyone and does nothing until asked.

## R7-42 Sign a PDF

- [ ] R7-LATER-01 The entry. How: in Files, right-click a PDF, **Scripts**. Expect: **Sign a PDF** among the others (on a new account the hook writes it; on an old one it arrives with the hook's version 5). Nothing else changed in the menu.
- [ ] R7-LATER-02 The card first. How: set the Vault up and make a certificate for documents (`ame vault cert new "My signature" pdf`), open it, pick **Sign a PDF** on a PDF. Expect: a card that names the file, says the Vault asks to approve each signature, that the original is never touched, and that **a reader calls the signer unknown until it trusts this machine's root**, with `ame vault root`; **Cancel** signs nothing. [needs: an open vault with a PDF certificate]
- [ ] R7-LATER-03 Signing. How: press **Sign** on the card. Expect: the Vault's approval question for the signature, then a notification naming `NAME (signed).pdf` beside the original; `pdfsig "NAME (signed).pdf"` shows a signature that validates against this machine's root; the original is byte for byte as it was. [needs: an open vault with a PDF certificate]
- [ ] R7-LATER-04 Papers. How: open the signed copy in Papers. Expect: the signature is shown; it is trusted on this account once a first signature has run (`ame vault pdf sign` trusts the root for signatures in the account's database), and unknown on another computer. [needs: Papers from Flathub]
- [ ] R7-LATER-05 Closed or missing. How: lock the vault (`ame vault lock`), pick **Sign a PDF**. Expect: "The vault is closed. Open it now?", **Yes** opens a terminal at `ame vault open`; with no vault, "Set one up now?" and `ame vault setup`; with no certificate for documents, how to make one. Nothing is signed in any of the three.
- [ ] R7-LATER-06 Not a PDF. How: pick it on a text file and on a link to a PDF. Expect: "That is not a PDF, so it was left as it is." and no card.

## R7-04 Commands show their own progress

- [ ] R7-LATER-07 Not built, and why. How: `kitty --version`; `printf '\e]9;4;1;42\e\\'` in kitty. Expect: kitty 0.48.2 draws its own progress bar at the top of its window and shows the percentage in its own tab bar; `kitten @ ls` has no progress field and the window title does not change, so nothing reaches the pill. The documented outside route (`on_program_status_change`, OSC 7501) is kitty 0.50.0, which Fedora 45 does not have yet. → PASS (headless, 2026-11-06): the bar was drawn at 42% in a screenshot of a headless shell; `kitten @ ls` JSON has no progress key; title stayed `bash`.

## R7-47 Any language, on your screen

- [ ] R7-LATER-10 Off until chosen. How: `Print` on a fresh account. Expect: no translate button in the bar; `~/.local/share/amethystora/translate` does not exist, and `ame security sends` has no line for it; `ls /usr/libexec/amethystora-bergamot` finds nothing (the translator is not in the image).
- [ ] R7-LATER-11 Building the translator. How: `ame desktop translate on`. Expect: it says the translator is built here once, from its source, in a Fedora container removed afterwards, about 20 minutes and about 250 MB from registry.fedoraproject.org, Fedora's mirrors and github.com, and asks; after **y** it builds (`podman images` shows no Fedora image afterwards unless there was one before) and `~/.local/share/amethystora/translate/engine/amethystora-bergamot --version` prints a version; the `licenses` folder beside it holds Marian's, Eigen's MPL and `umHalf.LICENSE`. [needs: network, podman, 20 minutes] → PASS in a throwaway container (2026-11-06): the build script ran on Fedora 45 in 18 minutes at three jobs and `nice 19`, and the program translated German to English at 238 MB; the recipe's own flow was not run on the real account.
- [ ] R7-LATER-12 Choosing languages. How: after the build, pick German to read from and your language. Expect: it names how many MiB and from which hosts (Mozilla's CDN and raw.githubusercontent.com only), asks, downloads; `ame security sends` then names both. A tampered model is covered by `test-translate.sh`. [needs: network]
- [ ] R7-LATER-13 A box. How: open a page of German text, `Print`, frame a paragraph, press the translate button. Expect: a card at once ("Reading the picture…"), then within about ten seconds the German, the translation, "Translated here. Nothing left this computer.", **Copy** and **Translate more**. [needs: the engine built and a language fetched]
- [ ] R7-LATER-14 Its memory. How: while a box is read, `systemd-cgls` of the user manager shows a scope `run-*.scope`; `watch -n0.2 "ps -o rss,comm -C amethystora-bergamot"`. Expect: the helper's scope has `MemoryMax=400M`, the engine stays under 300 MB, and no process is left once the card shows. [needs: the engine]
- [ ] R7-LATER-15 Cancelling. How: press the translate button, then close the screenshot bar before the card shows. Expect: `ps` shows no `amethystora-translate`, `tesseract` or `amethystora-bergamot` a second later. A second press while one is reading is ignored.
- [ ] R7-LATER-16 Nothing kept. How: translate a box holding `canary.later47`, close the card and the bar. `grep -rl canary.later47 ~ /tmp "$XDG_RUNTIME_DIR" 2>/dev/null; journalctl --user --since -5m | grep canary.later47`. Expect: nothing found.
- [ ] R7-LATER-17 Offline. How: with the network off, translate a box. Expect: it works; `ss -tp` shows nothing of it at any time. [needs: a language already fetched]
- [ ] R7-LATER-18 Off and remove. How: `ame desktop translate off`, then `remove`, then `remove all`. Expect: the button goes at once; `remove` leaves only `engine/`; `remove all` deletes the folder and the line in `ame security sends`.
- [ ] R7-LATER-19 Chinese, Japanese, Korean. How: `ame desktop translate languages`. Expect: not listed (their models use 364 to 399 MB, over the 300 MB limit; only the maintainer can raise the limit).

## R7-68 Look good in every call

- [ ] R7-LATER-20 Off. How: fresh account. Expect: no `amethystora-camera` process, no unit enabled, `~/.local/share/amethystora/camera` absent, and the camera list in an app as before.
- [ ] R7-LATER-21 The cost check. How: `ame desktop camera on`. Expect: it asks before downloading the 6 MiB model from media.githubusercontent.com, measures, and either turns on with "Here it takes N% of one processor core" or says it would take N% against the limit of 15% and turns on nothing. On this class of machine (4 cores, busy) the second is expected: 720p at 30 pictures a second through Python, onnxruntime and PIL measured 136% in a loaded lab and is offered only where it is under 15%.
- [ ] R7-LATER-22 Apps see a second camera. How: with it on, open a call app. Expect: a camera named after the virtual device (the image names it "OBS Virtual Camera", which is a name the maintainer may change in `98-v4l2loopback.conf`); picking it shows the picture with the background blurred and the light even. [needs: a camera, an app]
- [ ] R7-LATER-23 Only while used. How: `ps -C amethystora-camera -o rss,pcpu` with no app using the camera, then with one. Expect: idle, a small process at 0% that holds the virtual device and does not open the real camera (`fuser /dev/video*` shows no `amethystora-camera` on the real one); in use, under the unit's `CPUQuota=40%`; ten seconds after the app leaves the real camera is let go. [needs: a camera]
- [ ] R7-LATER-24 Blocking the webcam. How: `ame security block-webcam on` with it on. Expect: no real camera, so `amethystora-camera run` exits saying it needs one; nothing is read. [needs: a camera]
- [ ] R7-LATER-25 Never written. How: during a call, `find ~ /tmp "$XDG_RUNTIME_DIR" -newer /proc/$$ -name '*.png' -o -name '*.jpg' -o -name '*.raw'` and the journal. Expect: nothing. [needs: a camera]
- [ ] R7-LATER-26 Off again. How: `ame desktop camera off`, then `remove`. Expect: the unit is gone, the model and the settings file are deleted, the line in `ame security sends` goes.

## R7-63 Your photos, by what is in them

- [ ] R7-LATER-30 Off. How: fresh account. Expect: `photo: dog` in the search shows nothing of ours; no `amethystora-photos` unit or timer enabled; the folder absent.
- [ ] R7-LATER-31 Turning on. How: `ame desktop search photos on`. Expect: it says about 148 MiB from huggingface.co and its content network, that photos are read on mains power while you are away, and that what is worked out is sealed in the Vault; asks; downloads; enables `amethystora-photos.timer` and starts one reading.
- [ ] R7-LATER-32 Reading stops at input. How: with a few thousand photos in Pictures, on mains power, leave the machine alone for 30 s, watch `journalctl --user -u amethystora-photos -f`, then touch the mouse. Expect: it reads while you are away and ends within a second of the touch; the unit runs at `Nice=19` and in the idle I/O class; unplug the power and it does nothing at all. [needs: a laptop's battery for the last part; the rest on any machine]
- [ ] R7-LATER-33 Sealed. How: after a reading, `ls ~/.local/share/amethystora/photos/index` and `strings` one file; `grep -rl "$(ls ~/Pictures | head -1)" ~/.local/share/amethystora/photos`. Expect: files named `0000`, `0001`…, starting with `AVs1`, with no photo's name in the clear; the Vault's key `photo-index` exists only as the vault's own.
- [ ] R7-LATER-34 A search. How: open the search, type `photo: the dog at the beach`. Expect: within about two seconds the nearest photos, with "Pictures › folder · by what is in it" and Enter opening one in the default viewer. In a lab with three drawn pictures `a red circle` put the red circle first. [needs: a real photo library]
- [ ] R7-LATER-35 A closed vault. How: `ame vault lock`, search `photo: dog`. Expect: one row, "Your photos are sealed in the Vault / Open the Vault, then search again", opening the Vault when pressed; never a photo. The index does not grow while the vault is closed (`amethystora-photos index` ends with 75).
- [ ] R7-LATER-36 What it skips. How: put a picture in `~/Pictures/Private`, in `~/Pictures/.hidden` and a link to one. Expect: none is read or found.
- [ ] R7-LATER-37 Memory. How: `systemd-run --user --scope -p MemoryMax=450M` is the unit's limit; a reading of 30 photos of 12 megapixels peaked at 264 MB, a search at 182 MB. Expect: no reading killed for memory on 12-megapixel photos.
- [ ] R7-LATER-38 Off. How: `ame desktop search photos off`. Expect: the timer is gone, the folder, the models and the Vault's key for it are deleted, and `photo: dog` shows nothing.

## R7-26 Another screen, from what you have

- [ ] R7-LATER-40 Off. How: fresh account. Expect: nothing listens on 3389 (`ss -ltn`), `firewall-cmd --zone=amethystora-home --list-services` has no `ms-wbt`, `grdctl status` says RDP is off.
- [ ] R7-LATER-41 Turning on. How: on a network named home, `ame desktop second-screen on`. Expect: it says what it opens and asks; then prints the address, the user name and a new eight-digit code; `firewall-cmd --zone=amethystora-home --list-services` has `ms-wbt` (runtime, not permanent); `ss -ltn` shows 3389. [needs: a polkit prompt for the firewall, gnome-remote-desktop]
- [ ] R7-LATER-42 The second screen. How: from another computer, an RDP client with the address, `amethystora` and the code. Expect: a virtual monitor appears next to the real one (`gdbus call` or Settings › Displays shows it), windows can be dragged to it, and the other side's keyboard and pointer or touch work. [needs: a second computer or tablet with an RDP client]
- [ ] R7-LATER-43 A network that is not home. How: connect to a network not named home and try the client. Expect: it cannot connect (the zone does not have the service).
- [ ] R7-LATER-44 Off. How: `ame desktop second-screen off`. Expect: the monitor goes, the code no longer works, the port is closed, and `gsettings get org.gnome.desktop.remote-desktop.rdp screen-share-mode` is what it was before.
- [ ] R7-LATER-45 The agent. How: ask the agent to run `ame desktop second-screen on` and `grdctl rdp enable`. Expect: both refused as the user's; `ame desktop second-screen status` passes.

## R7-50 Help from someone you trust

- [ ] R7-LATER-50 Off. How: fresh account. Expect: as R7-LATER-40.
- [ ] R7-LATER-51 View only. How: `ame desktop help-me on`, connect from another computer. Expect: a new code each time; the other side sees the screen and its pointer does nothing; GNOME's own screen-sharing indicator is in the top bar; Notes, Security, Backups, Help and the Vault are covered while the whole screen is shared (the share guard). [needs: a second computer; the guard's cover is the one thing not seen working here]
- [ ] R7-LATER-52 Letting them in. How: `ame desktop help-me control on`, then `off`. Expect: the other side's mouse and keyboard work, then do not.
- [ ] R7-LATER-53 It ends. How: lock the screen. Expect: the connection ends, the code stops working, a notification says so, `ame desktop help-me status` says off. Also: wait an hour (or lower `LIMIT` in a copy), and `ame desktop help-me off` at once. [needs: a real lock]
- [ ] R7-LATER-54 Across the internet. Expect: it does not work, and the manual and the command say so: reaching a computer behind a router needs a relay, and Amethystora runs none. Nothing is sent to any server of ours.
