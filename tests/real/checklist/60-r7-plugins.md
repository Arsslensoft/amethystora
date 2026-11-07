# Round 7, plugins: a pill of your own, scripts, drafts, places, Made by the agent, a file to share, a sandbox

Walk on a real machine running the image with round 7, as a person would. The spec is `plans/ui-ideas/r7/spec-plugins.md`. Mark each line as the
other files do: `→ PASS`, `→ FAIL: what was seen`, `→ NOT DONE: needs ...`. "Headless" is a throwaway GNOME Shell 51 of its own on a stand-in home with the
branch's extension, the host a debug build with the image's plugins and seccomp filter, and bubblewrap really running each plugin (`systemd-run`'s scope stood
in for, since a stand-in home has no user manager): everything below that was seen that way says so.

## R7-01 A pill of your own

- [ ] R7-PLUGINS-01 In the pill. How: `ame plugin add amethystora.pomodoro`, `ame plugin on amethystora.pomodoro`, open the plugin's tile in quick settings and press it. Expect: the pill shows "Focus" with a ring and a countdown that counts in the shell, "Start a focus timer" before. [needs: the image with the host unit] → PARTLY SEEN (headless): the host ran the plugin in its sandbox and the shell drew its output as a tile; a plugin printing text, a ring and `until` showed "Build 3 of 5 · 02:03" in the pill and the count went down by itself
- [ ] R7-PLUGINS-02 The system stays in front. How: with a plugin in the pill, start a recording (or a timer, a link from a phone). Expect: the system's activity takes the pill and the plugin is a row of the card or the bubble, never in front of a call, a recording, the battery's question or an update waiting. [needs: a recording or a call] → NOT DONE: needs a recording; `ORDER` is held by test-plugins.sh and pill-logic
- [ ] R7-PLUGINS-03 Its card. How: click the pill with a plugin in it. Expect: the plugin's name, "A plugin", what it printed (rows, a bar, buttons, a switch), then what it may reach, then Pause; nothing in it can be typed into. → PASS (headless): name, kind line, text, a row, a progress bar from the left, two buttons, "It can reach: the network: api.github.com, notifications." and Pause
- [ ] R7-PLUGINS-04 Countdown costs nothing. How: with a countdown shown, `top -d1` the shell; then hide the bar (another window fullscreen). Expect: one tick a second while it is on screen and none while it is not. → NOT DONE: needs a CPU measurement on the real session
- [ ] R7-PLUGINS-05 Private. How: a plugin printing `"private": true`; share the screen (or start a recording). Expect: the pill and the card say "Private" while it lasts, and the words come back after. [needs: a screen share] → NOT DONE: needs a screen cast; the code path is `_hidden` through Capture
- [ ] R7-PLUGINS-06 No system look. How: a plugin printing `{"text":"Call from Mum","class":"critical","plate":"link"}`. Expect: the host refuses the unknown field and the pill shows the plugin's error, never a link plate; the card still names the plugin. → PASS (cargo `refuses_what_is_not_the_format`; `test-plugins-clamp.mjs`; test-plugins.sh greps no plate in the module)
- [x] R7-PLUGINS-07 Two plugins take turns. How: two plugins on the pill, one updates. Expect: the one that changed last is in the pill, the other is a row of its card. → PASS (headless, with a script and a plugin) → PASS (headless, scripts only, 2026-11-06): A, then B two seconds later, then A updated: the pill showed B, then A with B a row behind it. Changes in the same burst follow the folder's order

## R7-03 Any script can go live

- [ ] R7-PLUGINS-08 start, update, end. How: `ame pill start "Backup" --progress 0.1 --text Copying`, then `update "Backup" --progress 0.6`, then `end "Backup"`. Expect: "Copying" in the pill with a ring that fills, then gone. → PASS (headless, the helper through just, as `ame` runs it): the pill appeared with the text and the ring, and went at `end`
- [ ] R7-PLUGINS-09 It goes when the script does. How: in a terminal `(ame pill start Job --text x; sleep 5)`; wait. Expect: the pill is gone about two seconds after the subshell ends, with no `ame pill end`; also after `kill -9` of the script. → PASS (headless): gone within 6 s; the owner was found through `just`
- [x] R7-PLUGINS-10 A flood ends it. How: a loop of `ame pill update Job --text $i` a thousand times in a minute. Expect: the pill ends, and the pill says so once ("Job updated too often, so its pill ended"). → PASS (headless, image beta-45.20261106.1, 2026-11-06's shell): a loop of updates was ended after about 400 changes in 30 s; the pill said "Flood2 updated too often, so its pill ended" once, and the script's next update was told there is no such pill
- [x] R7-PLUGINS-11 Nothing hostile is read. How: in `$XDG_RUNTIME_DIR/amethystora/pill` make a FIFO, a link to `/etc/passwd`, a 9 KB file and a good one. Expect: only the good one shows and the shell stays responsive. → PASS (headless): only the good file was drawn, no hang → PASS (headless again, 2026-11-06): a FIFO, a link to /etc/passwd and a 9 KB file beside two good files: only the two good ones were drawn, the shell answered at once
- [x] R7-PLUGINS-12 At most eight. How: start nine pills. Expect: eight are in the pill and card rows; the ninth is not drawn. → PASS (headless, 2026-11-06): nine started within a second: eight read, the ninth left out

## R7-09 A bad one stops itself

- [ ] R7-PLUGINS-13 A crash loop. How: a plugin whose command is `exit 1`, interval 5. Expect: after five failed runs it is paused, the pill says "… was paused" once, the plugin's menu says "Paused: it failed several times in a row" and Control shows it off. → PASS (headless): a draft whose command is `exit 1` was paused by the real host after five runs (reason `crash`), the shell turned its bar button into a warning sign whose menu says "Paused: it failed several times in a row" and offers Resume; the notice and its Ask-the-agent button are drawn by `_paused` (seen with the notice path in the pill)
- [ ] R7-PLUGINS-14 Slow, memory, flood. How: a plugin that sleeps 40 s (three runs); one that allocates 300 MB (two runs); a script that updates 40 times in a minute. Expect: each is paused with its own reason in the plugin's menu. [needs: the real user manager] → NOT DONE
- [ ] R7-PLUGINS-15 Ask the agent to fix it. How: with the agent on, a plugin pauses. Expect: the notice has **Ask the agent to fix it**, which opens the agent in a terminal on `ame plugin fix ID`; with the agent off there is no such button. → NOT DONE: needs the agent on a real session
- [ ] R7-PLUGINS-16 A closed Vault waits. How: a plugin with a secret setting while the Vault is closed. Expect: "Waiting: its secret settings are in the Vault, which is closed", never "paused", and it runs by itself once the Vault is opened. [needs: the Vault] → PASS in part (headless, a stand-in Vault socket): with the Vault closed the host said "its secret settings are in the Vault, which is closed"; with a stand-in open, the secret was sealed (a 0600 file, no canary in the home), arrived in the sandbox on descriptor 10 and in no environment variable; a real Vault and the "resumes by itself" are NOT DONE

## R7-06 One list of everything made for you

- [ ] R7-PLUGINS-17 The list. How: Control, Desktop. Expect: **Plugins**, **Made by the agent**, **Ready to add**, **Show plugins** and **Plugins on the desktop**, each plugin with where it shows, whose it is and since when. → PASS (the list's JSON with a stand-in host: rows, tags, actions); the window itself is not drawn on this machine
- [ ] R7-PLUGINS-18 Fix only with the agent. How: agent off, then on. Expect: **Fix…** and **Change…** appear only while the agent is on. → PASS (the list with `agentic` false and true)
- [ ] R7-PLUGINS-19 Remove all. How: **Remove all…** under Made by the agent. Expect: a terminal lists what will go and asks. [needs: the agent on] → NOT DONE: needs a terminal on a real session

## R7-07 More places, one format

- [ ] R7-PLUGINS-20 A tile. How: a plugin with `contributes.tile` and a card with a switch. Expect: a tile in quick settings, on while its switch is, and a press asks the host for the switch's action. → PASS (headless): the tile showed its name and value and was drawn on
- [ ] R7-PLUGINS-21 A search answer. How: `Super+Space`, type the plugin's prefix. Expect: a "Plugins" section with the answer; the words you typed are not sent anywhere. → PASS (headless): "Plugins · what each last printed: CI build: 7"
- [ ] R7-PLUGINS-22 A dock badge. How: a plugin with `contributes.dock` naming Files that prints "7" and a progress. Expect: a 7 on the Files icon and a bar. → PASS (headless): the badge showed on Files' icon
- [ ] R7-PLUGINS-23 The bar and the desktop. How: the System plugin in the bar; Calendar and Clock on the desktop with *Plugins on the desktop* on. Expect: "42% · 35%" in the bar with its menu; two cards in the corner with the lines apart. → PASS (headless): all three drawn from sandboxed runs, the calendar in three lines
- [ ] R7-PLUGINS-24 Off is off. How: turn a plugin off. Expect: its pill, bar button, tile, search answer, badge and card go. → PASS (headless, `_list` with the plugin gone and the host ended)

## R7-05 Share one as a file

- [ ] R7-PLUGINS-25 Share and open. How: `ame plugin share local.my-ci`, then double-click the file. Expect: a terminal shows the plugin's name, what it asks for and why, and asks for each; nothing has run. [needs: a terminal on the real session] → PARTLY SEEN: `share` wrote a tar and `check` read it back; the type and the handler are held by 20-tests.sh
- [ ] R7-PLUGINS-26 Nothing runs before Keep. How: open one and answer no to everything. Expect: nothing installed, nothing in the pill. → NOT DONE: needs a terminal and an administrator's dialog

## R7-02 Say it, see it, keep it

- [ ] R7-PLUGINS-27 Make. How: `ame agent toggle on`, `ame plugin make "show my GitHub build in the pill"`. Expect: the agent opens with the draft's folder and what it may and may not do. [needs: an agent] → NOT DONE: needs an agent account
- [ ] R7-PLUGINS-28 A draft. How: `ame plugin new my-ci ci-build`, `ame plugin draft local.my-ci`. Expect: the pill shows "Draft · …" with Keep, Change and Throw away; nothing is allowed. → PASS (headless): the draft ran in the sandbox with no network, was marked by the agent, and its card showed the three buttons and "It can reach nothing outside its own sandbox."
- [ ] R7-PLUGINS-29 Keep. How: press **Keep**. Expect: a terminal reads what it asks for and asks for each permission, then an administrator's dialog; the draft is gone and the plugin is on. [needs: a terminal and polkit] → NOT DONE
- [ ] R7-PLUGINS-30 Throw away. How: **Throw away**. Expect: the draft, its state and what it kept are gone. → PASS (host `discard`, used by the button)
- [ ] R7-PLUGINS-31 The agent cannot. How: ask the agent to run `ame plugin keep ID`, `open FILE`, `add`, `grant`. Expect: refused. → PASS (cargo never::tests); the agent may run `draft`, `check`, `try`, `discard`, `ame pill`

## R7-10 Examples

- [ ] R7-PLUGINS-32 Examples start. How: `ame plugin examples`, `ame plugin new my-ci ci-build`, `ame plugin check local.my-ci`. Expect: five that reach a service and the image's own; the check passes and says what it asks for. → PASS (headless)
- [ ] R7-PLUGINS-33 A real build. How: keep `ci-build`, `ame plugin set ID repo`, allow `api.github.com`. Expect: the latest workflow run of that repository in the pill, and **Open the run** opens its page after a click. [needs: network, an administrator] → NOT DONE: needs a kept plugin and the network

## R7-08 Sandbox

- [ ] R7-PLUGINS-34 The six run in the sandbox. How: `ame plugin add amethystora.clock` (and calendar, system, hello, do-not-disturb, now-playing), turn each on. Expect: each does what the widget did, from a plugin. → PASS in part (headless): clock, calendar, hello and system ran in bubblewrap and drew; do-not-disturb and now-playing are held by the manifest and the broker's tests (no player here)
- [ ] R7-PLUGINS-35 The broker. How: inside a draft, ask for state, control, open. Expect: state is a sample; control and open are refused for a draft; a granted plugin's control and open need a click. → PASS (headless: a draft through the real socket; cargo: grants, clicks, the notice rate, a revoked grant)
- [ ] R7-PLUGINS-36 The old folder is dead. How: put a widget in `~/.config/amethystora/widgets`. Expect: it runs nothing and is not listed. → PASS (the extension and its runner are gone; 20-tests.sh)
- [x] R7-PLUGINS-37 Off costs nothing. How: with no plugin on, `pgrep -af amethystora-plugin-host`, `busctl --user | grep Plugins`, and `ame desktop plugins off`. Expect: no host, no name, no timer; with the key off the module watches nothing. → PASS in part (headless: no host while `enabled` is empty) → PASS in part (image beta-45.20261106.1, 2026-11-06), the real account with no plugin added: no `amethystora-plugin-host` process, no `org.amethystora.Plugins` on the bus, the unit disabled. A measured wakeup count is NOT DONE

## Constraints

- [ ] R7-PLUGINS-38 Nothing blocks the shell. How: `grep -rn "_sync(" modules/plugins* modules/search/plugins.js`. Expect: none. → PASS (test-plugins.sh)
- [ ] R7-PLUGINS-39 A lock and unlock. How: lock and unlock with a plugin in the pill. Expect: it is back, with no flash of an old state. [needs: a real lock] → NOT DONE: needs a real lock; the host holds every state and the module asks `List` again
- [ ] R7-PLUGINS-40 A canary. How: give a plugin a secret setting `CANARY-R7-PLUGINS` (`ame plugin set`), then `grep -r CANARY-R7-PLUGINS ~ /tmp /var/lib/amethystora 2>/dev/null; journalctl --user -b | grep CANARY`. Expect: nothing finds it in the clear. [needs: the Vault] → PASS in part: cargo's canary test and the headless run (a stand-in Vault socket) found nothing in the clear under the home; the real Vault, `/tmp`, `/var/lib/amethystora` and the journal are NOT DONE
