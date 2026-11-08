# Privacy on the screen

What the desktop hides while you share your screen, what the shield tells you, and what it shows about your network.

## Share guard

While your whole screen is shared, in a video call or a remote desktop, the windows that hold private things are
covered: blurred so that no word can be read, and labelled **Hidden while you share**. A window that opens during the
share is covered before it is first drawn, and the live pill says once what is covered.

![Covered while you share](media/share-guard-blur.webm)

### What is covered

Notes, Security, Backups, Help (which lists what you deleted and removed), Vault and the password managers
(KeePassXC, Bitwarden, Secrets, Passwords and Keys, Proton Pass, 1Password, Authenticator), and Files windows showing
your [private folder](disk-encryption.md#a-private-folder-that-locks-with-the-screen).

The covers come at once when a share starts and go as soon as the desktop knows you share single windows only.
Nothing is covered while you only record the screen yourself.

### Showing one window

The desktop cannot leave a window out of a picture of the whole screen, so the cover is on your screen too. Hold a
click on a cover for a moment to show that window, with its menus and dialogs, until the share ends. While someone
else controls your computer through the share (a remote desktop), holding does nothing, since the hold could be
theirs.

```bash
ame desktop share-guard blur|card|off        # blurred (the default), a solid card, or nothing covered
ame desktop share-guard apps                 # pick the apps covered, in a terminal
ame desktop share-guard apps APP.desktop...  # name them by their launcher; `apps reset` puts back the list above
ame desktop share-guard private on|off       # the private folder's windows, covered or not
```

### What a cover is not

A cover keeps a window's contents from being shown by accident; it is not a lock. A notification, the window's title
in the top bar and the dock, and any app the list does not name are shown as ever. An app that hides passwords by
itself is safer still.

## The shield

**Shield**, a tile in quick settings, says how the machine's protection stands: *Protected*, or how many things need a
look, and whether the network you are on is home or away. It changes nothing: every line opens
[Security](security.md) on its page, where every switch is. It reads what it shows when you open it, and costs
nothing otherwise.

Its menu has the security report's result (for administrators, who may read the report the security watcher writes;
others see the rest of the menu), the network and its [firewall zone](security.md#on-from-the-start),
[encrypted DNS](network-security.md#encrypted-dns),
[USB protection](devices-security.md#block-usb-devices-you-did-not-plug-in), the private folder, open or closed, and
whether apps may ask for the camera and the microphone.

`ame desktop shield tile|mark|attention|off` puts it elsewhere: a tile; a shield in the top bar beside the system's
icons, green, amber when something needs a look; the same shield only while something needs a look; or nothing.

## Network by app

Off until you turn it on (`ame desktop network-apps on`, or Control). The network meter's menu then ends with the apps
that have a connection open, how many, and to where. Only your own programs are listed, and only while the menu is
open. **Network per app, in Security** opens the Apps page, where an app's network can be taken away.

- A place shows as a name only when the machine has a name for its address already (your hosts file, your own
  network's names, a reverse name it looked up before), so most places on the internet show as addresses. Nothing is
  asked anywhere to find a name, and nothing leaves the machine.
- It counts connections, not bytes: how much each app sends would need an administrator's rights.

## A new network's zone

The first time you join a network, a notification names the firewall zone it is in and what that means:

- the default zone: phone pairing is closed and nothing on this computer is shared;
- the home zone: phones can pair;
- the away zone: nothing that starts on that network reaches the computer.

**Open Security** opens its Network settings, where a network can be named home
([home networks](security.md#on-from-the-start)). The notice changes nothing itself, and comes once per network: the
networks this account has joined are remembered by NetworkManager's id for them, never by name.
`ame desktop zone-notice off` turns it off.
