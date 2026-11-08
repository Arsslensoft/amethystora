# Others at your computer

Lending the computer for a moment, and a simpler desktop for someone who wants one.

## Hand it over

Someone asks to look something up on your computer, or a child wants one game. **Hand this over…**, in a window's
menu, keeps that window alone on the screen: full screen, no other apps, no notifications, the other screens covered,
and the shortcuts that would leave it go to the app instead. A chip at the top says how to end it.

### Getting it back

Lock the screen, from the chip or with `Super+L`, and unlock it with your password: the window goes back to how it
was. If the app's last window closes, the screen locks. The bar at the top, with the notifications and the power menu,
does not come down meanwhile. An app that takes over the keyboard, such as a virtual machine or a remote desktop, can
keep `Super+L` from reaching the desktop: `Super+Esc` gives the shortcuts back, and the chip, the screen locking by
itself and the power button always work.

### What it is not

It is a courtesy, not a security boundary. The app can still open what it can open (a browser reaches the web, and its
file dialogs reach your files), and an account without a password unlocks without one. For longer, or for someone you
do not trust, use a [guest login](logins.md#guest-login). `ame desktop windows hand-over off` takes it out of the menu.

## A simple desktop

For someone who wants the computer to be simple, set up once by the family's technical one: `ame desktop windows
simple-desktop on`, or **A simple desktop** in [Control](control.md), gives this account big tiles on the wallpaper for
the internet, email, photos and files, large text, a larger pointer, one window at a time, and a plain top bar with a
**Home** button that puts the windows down and shows the tiles.

### The tiles

Choose them yourself, six at most, by their launchers: `ame desktop windows simple-tiles org.gnome.Calculator.desktop
firefox.desktop`. With none, it picks the apps this account opens those four kinds of thing with. `ame desktop windows
simple-tiles` on its own asks for them, as **The simple desktop's tiles** in Control does. The switch and the tiles
travel in [your setup file](setup.md).

### Going back

`Super+Alt+Home` goes back, with each setting as it was unless you have changed it since. Nothing is taken out of the
system: everything is still there, and every other account is as it was.

- If it cannot keep the settings it replaces, it does not turn on and says so, so that what it changes can always be
  given back.
- Turned off while the desktop is not running (from a text console, or with the screen locked), the settings come back
  as soon as the desktop is.
- It is not a lock: for a child or a shared machine, see Parental Controls.
