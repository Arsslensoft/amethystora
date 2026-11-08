# Workspaces

There are always six workspaces, so `Super+3` always means the same place. A common way to use them: a browser on 1,
a terminal on 2, chat on 6.

![Go to a workspace](media/workspace-super-number.webm)

| Key | Does |
| --- | --- |
| `Super` and a number | Go to that workspace |
| `Super+Shift` and a number | Take the focused window to that workspace |
| `Super+Page Up` / `Super+Page Down` | The previous / next workspace |
| `Super+Shift+Ctrl+Left` / `Right` | Move the window to the next monitor |
| `Super+Ctrl+Alt+Left` / `Right` | Swap the workspace with the one beside it, windows and name included |

### Names

A workspace shows its number and, in lighter type, the name of the app with most windows on it. Give it a name of your
own with a right click on it; **Use the app's name** in that menu goes back to the automatic one, and
`ame desktop space-names off` turns automatic names off.

### In the overview

To move a window to another workspace, open the overview and drag the window onto the workspace in the top bar, or
onto its picture at the top of the overview. Each picture carries the workspace's name along its bottom edge (its
number when it has none), and a workspace with a wallpaper of its own (the tiling's **Workspaces** settings) shows it
there, under its windows.

## Saved sets

Right-click a workspace in the top bar and choose **Save this workspace**: its apps and where their windows are become
a set named after the workspace. Click it under **Sets** in the same menu to open it again.

### Opening a set

- Its apps start and each window goes back to its place as it appears, on the workspace you are on if it is empty, or
  else on the next empty one.
- Windows of those apps already there are put in place rather than started twice, and nothing is closed. An app that
  can have only one window brings it over from wherever it is.
- While a set opens, a window you open on another workspace stays there: only windows that appear on the set's
  workspace, or while you are on it, are put in place.
- With the tiling on, the windows come back in the same order and widths.

### What a set keeps

The apps and their places, not what was open in them: apps that reopen their own documents (most browsers and editors)
open them again; others start empty, and a browser that brings back its own windows may open one more than the set
had. A set keeps at most sixteen windows, the ones you used last, and up to twelve sets are kept, in
`~/.local/state/amethystora/sets.json`. The cross on a set forgets it. `ame desktop sets off` takes Sets out of the
menu and keeps what was saved.

## Putting a workspace away

Halfway through something and need the desk clear? Right-click the workspace in the top bar and choose **Put this
workspace away**. The pill asks first, naming the workspace ("Put "Tax return" away?"); say **Put away** and its
windows close the way their close button closes them, so an app with unsaved work asks you first.

![Put a workspace away](media/put-workspace-away.webm)

### Bringing it back

The pill keeps a quiet mark, "Tax return · put away", and its card lists what is put away, each with **Bring back** and
**Forget**. Bring it back tomorrow or in a month: the apps start and each window goes to its place, as a
[saved set](#saved-sets) opens. Typing its name in the search box offers **Bring back** too. Like a set, it keeps the
apps and their places, never what was open in them. Up to eight workspaces can be put away at once.

### Kept in the Vault

What is put away (its name and its apps) is **sealed in your [Vault](vault.md)**, in
`~/.local/state/amethystora/put-away.avs`, which only your Vault opens. Nothing asks for the Vault by itself: at login
the mark says only "Put away", and a click reads the list, or says "Open Vault" when the Vault is closed. Put away
while the Vault is closed or not set up, a workspace is kept in memory until you log out, and the card says so, with
**Keep it** to seal it once the Vault is open. `ame desktop put-away off` takes the row out of the menu.
