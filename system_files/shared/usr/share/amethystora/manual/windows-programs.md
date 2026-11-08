# Windows programs

Double-click a Windows program in Files (an `.exe`, an `.msi`, a shortcut or a `.bat`) and a terminal opens
that asks what to do with it. Better ways to the same thing come first; the Windows program itself runs
with Bottles.

![A Windows program, offered its Linux app first](media/windows-program-offer.webm)

### Checked first

The file is scanned for viruses, and the terminal says who signed it: a name is only given as the
publisher's when the signature checks out, since anyone can sign a program in any name. Nothing is run if
the file matches known malware, or was changed after it was signed.

### Better ways, offered first

For some sixty well-known programs:

- **The same app for Linux**, from Flathub: Spotify, Discord, Zoom, Slack, VLC, OBS and many more. It
  updates with the system, runs in its sandbox, and opens straight away if it is installed already.
- **An app that does the same job**: LibreOffice or ONLYOFFICE for Microsoft Office, Pinta for Paint.NET,
  File Roller for 7-Zip and WinRAR, Papers for Acrobat Reader, GIMP, Krita and Inkscape for Adobe's apps.
- **Its web version, as a [web app](web-apps.md)**: Microsoft Teams, WhatsApp, Office, Google Drive.
- **Steam and the game launchers** are set up by `ame apps gaming` ([Games](games.md)), where Windows games
  run through Proton.

### Running it with Bottles

Otherwise, or if you want the Windows program itself, **Run the Windows program, with Bottles** installs
[Bottles](https://usebottles.com) from Flathub, makes a bottle named Windows the first time, and hands it
the file. Bottles runs Windows programs with Wine, and asks which bottle each one goes in.

- **X11, for Bottles alone.** Wine draws through X11, which every app is refused here (an app on X11 can
  read what you type into other X11 apps), so X11 is granted back to Bottles alone, for your account. The
  Security app's **Apps** page lists it, and `flatpak override --user --reset com.usebottles.bottles`
  takes it back.
- **Only the file you opened.** Bottles does not see your home folder: a program that needs the files next
  to it (an installer unpacked from a CD) has to be opened from Bottles itself, after you give it that
  folder in Flatseal.

### What will not run

Games whose anti-cheat runs in the Windows kernel (Valorant, League of Legends, FACEIT), Microsoft 365's
desktop apps and Adobe's Creative Cloud never run with Wine, and the handler says so. It says so for a
Windows driver too: Linux brings its own, and `ame system drivers` says which one each device uses. A
package from the Microsoft Store (`.msix`, `.appx`) is only matched against the list, since Wine cannot
install it.

For the rest, a Windows virtual machine runs nearly everything: the [developer image](developer.md) comes
with virtual machines, and WinBoat, which you install yourself, shows Windows apps from one as windows of
their own (the Windows licence is yours to bring).

### Finding a replacement

For a program the list does not know, [Flathub](https://flathub.org) and
[AlternativeTo](https://alternativeto.net) are the places to look.
