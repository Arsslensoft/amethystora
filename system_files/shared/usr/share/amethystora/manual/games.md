# Games and Android apps

Neither comes set up, because each needs something the image keeps from apps that do not need it. One
command each sets them up, says first what it changes, and takes it all away again. The switches on
[Control](control.md)'s **Apps** page open the same commands in a terminal.

## Games

```bash
ame apps gaming        # asks first; ame apps gaming on sets it up without the menu
```

This installs **Steam** from Flathub, with **ProtonPlus**, which adds other versions of Proton, and two
tools Steam's games can use: the **MangoHud** overlay and Valve's **gamescope** compositor. It also offers
**Heroic**, for the Epic, GOG and Amazon stores, and **Lutris**, for games from anywhere else.

### What it grants

- **X11, to Steam and its launchers.** Every Flatpak is refused X11 here, because an app on X11 can read
  what you type into other X11 apps ([what Flatpak apps may not do](software.md#what-flatpak-apps-may-not-do)).
  Steam and the Windows games it runs through Proton draw on nothing else, so X11 is granted back to Steam
  and the launchers you add, for your account alone. The Security app's **Apps** page lists it
  ([App permissions](app-permissions.md)).
- **`/dev/uinput`, for Steam Input,** which makes every controller work in every game through a virtual
  controller. The image keeps it from apps, since a virtual keyboard can type into any window. Setting up
  gaming gives it back to whoever sits at the machine, and so to every app that may reach all of `/dev`, the
  Chromium browsers among them ([Switches](security-switches.md#switches), `ame security virtual-input`).
  Taking gaming away takes it back from the next restart, unless OpenTabletDriver still needs it.

### Launch options

In a game's **Properties** in Steam, under **Launch Options**:

| Launch option | Does |
| --- | --- |
| `mangohud %command%` | The overlay: frame rate, frame times, temperatures |
| `gamemoderun %command%` | The CPU's performance mode while the game runs, through GameMode |
| `gamescope -f -- %command%` | The game in its own compositor, full screen, which helps with games that dislike Wayland |

### Controllers

Game controllers need nothing set up: Xbox, PlayStation, Nintendo and Steam controllers, 8BitDo, and the
rest Steam knows of. Microsoft's wireless adapter for Xbox controllers needs Microsoft's firmware first,
`ame apps xbox-adapter on` ([Hardware](devices.md#game-controllers)).

### More launchers, or none

**Add another launcher** adds Heroic or Lutris later, and **Take it all away** uninstalls them all, with
their games, and takes back what was granted:

```bash
ame apps gaming launchers
ame apps gaming off
```

## A faster CPU scheduler for games

The kernel decides which program runs on which CPU core and for how long, and its own scheduler suits nearly
everything. A sched-ext scheduler is a small program the kernel hands that job to, and one made for a kind
of work can do it better:

```bash
ame system scheduler
```

| Scheduler | For |
| --- | --- |
| `kernel` | Everything: the kernel's own, and the default |
| `lavd` | Games, smooth while other work runs: made for gaming handhelds and laptops |
| `bpfland` | A desktop that stays responsive while something heavy builds or renders |
| `flash` | Steady timing, for audio and music work |

Control's **System** page offers the same four. The choice holds after a restart, and
`ame system scheduler kernel` puts the kernel's own back.

### If it goes wrong

If a sched-ext scheduler stops, the kernel takes over again by itself, so the worst one can do is make
things slower. If the kernel will not take the one chosen, the command shows what the loader said and leaves
the kernel's own in place.

## Android apps

```bash
ame apps android       # asks first; ame apps android on sets it up without the menu
```

**Waydroid** runs Android, LineageOS 20, in a container beside your desktop, and the Android apps you
install appear in the app grid like any other. Setting it up downloads Android from Waydroid's servers,
about a gigabyte.

### Getting apps

It comes without Google's apps and Play Store, whose apps come with Google's own terms: get apps from
F-Droid, or install an `.apk` you downloaded:

```bash
waydroid app install ~/Downloads/some-app.apk
```

### Starting and stopping it

Open **Waydroid** from the app grid; the first start takes a minute. Its container service runs as root, so
it is on only once you set it up, and you can stop it when you are not using it:

| Command | Does |
| --- | --- |
| `ame apps android start` | Start Android, and open it |
| `ame apps android stop` | Stop it, until you start it again |
| `ame apps android off` | Remove Android, with every app and everything saved in it |

Waydroid cannot draw with NVIDIA's driver, so on the NVIDIA images Android draws in software: fine for most
apps, slow for games.
