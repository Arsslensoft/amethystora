# Files

`Super+Shift+F` opens a Files window and `Super+E` your home folder. Folders open on a grid, where the icon theme's
folders show as the artwork they are. Many of the extras below are in Files' **Scripts** menu, which shows only for
something selected: for the folder you are in, right-click it in the folder above.

## Folder colours

Select one or more folders, right-click, and pick **Scripts › Folder colour ›** Amethyst, Sky, Jade, Amber, Coral or
Slate (listed alphabetically). The folder takes that colour at once, and a stack of it in the dock takes it as you open
the stack or bring the pointer to the dock. Your icon pack draws the colour, so it follows when you change the pack.

**No colour** takes it away, and gives Documents, Music, Pictures and the other special folders their emblem back. A
picture you set as a folder's icon yourself is left alone, and files in the selection are left as they are. Stars are
Files' own: the star beside a file in the list view, and **Starred** in its sidebar.

## Back in time

Right-click a folder (or a file) in your home folder and pick **Scripts › Back in time**: [Backups](backups.md) opens
on that folder's timeline, where you see it as it was and put back what is gone. For the folder you are in, right-click
it in the folder above, or use **Open With › Backups**.

![Back in time](media/files-back-in-time.webm)

Folders outside your home folder are not in the snapshots or the backup, and the menu says so.

## Earlier versions

Right-click a file in your home folder and pick **Scripts › Earlier versions**: [Backups](backups.md) opens on that
file's own versions, each with when it was made.

- **Look** opens one read-only, beside the file you have.
- **Keep this one too** puts it next to your file with its date in its name; your file is never touched.

It shows what the hourly snapshots and the backup hold, those on this machine at once and those in the backup when it
answers, and says when either is off. A file in the [private folder](disk-encryption.md#a-private-folder-that-locks-with-the-screen) is in neither. `ame desktop
earlier-versions off`, or **Earlier versions in Files** in [Control](control.md), takes the entry away.

## A file nothing opens

Open a kind of file nothing here opens and a card says what it is, in plain words, and which app on Flathub opens it,
with **Install and open**.

- Each app says whether Flathub has checked who made it: any app can claim a kind of file, and the one you install is
  started on this file with whatever it asks for.
- The file stays on this computer. Only its type is looked up, in the list of apps Flatpak keeps here, so nothing is
  sent until you press the button.
- A kind no app on Flathub claims, or a file that does not say what it is, is said too, and **Open With** in Files can
  still try any app on it.
- An app you install takes those files from then on, without the card.

 `ame desktop open-anything off`, or **A file nothing opens says what it is** in [Control](control.md), brings back
GNOME's own message for your account.

## Look at a file with Space

Select a file and press `Space` for a preview: pictures, PDFs, text and video. The arrow keys walk the folder, the
button in the preview opens the file in its app, and `Space` or `Esc` closes it. It is GNOME's previewer, one of the
apps that [install the first time the machine is online](software.md#apps-bazaar-and-flathub). It runs only while it
shows and keeps nothing, and it can read your home folder only, so a file on a USB stick does not preview.

## Copy without hidden data

Select one or more files and pick **Scripts › Copy without hidden data**: beside each, a copy named `NAME (clean).EXT`
without what the file carries besides its content, and a notification says what was taken out (*Removed: place,
camera, time*). The original is never changed. mat2 makes the copy, and takes out:

- **Photos and pictures** (JPEG, PNG, TIFF, WebP, HEIC, GIF, SVG): where they were taken, the camera and
  lens, the times, the author, the program and comments. The picture is saved anew.
- **PDF**: the author, title, subject, keywords, the programs that made it, its dates and its XMP data. Its
  text stays selectable and its links work; comments, form fields and files attached inside may stay.
- **Office documents** (LibreOffice's and Microsoft's, and EPUB): the author, who saved it last, how many
  times and for how long it was edited, the program and the dates. A document with a part mat2 does not
  know is not cleaned at all, rather than half.
- **Music and sound** (MP3, FLAC, Ogg, WAV, AIFF): tags, comments and embedded pictures. **Video** (MP4,
  AVI, WMV): its metadata.
- **Archives** (zip, tar): their own details and each file's inside.

What the content itself shows or says stays: a face, a street sign, words in a picture, a name typed in a
document, and the file's own name. A file of another kind is left as it was, and the notification says so.

## Sign a PDF

Select a PDF and pick **Scripts › Sign a PDF**: a card first, then a signed copy named `NAME (signed).pdf` beside the
original, which is never touched. The signature is digital, made with a certificate of yours that stays in the
[Vault](vault-certificates.md#signing-a-pdf), and the Vault asks you to approve each one. Several PDFs are signed one
after the other, each approved by you.

**Before you sign, know what a reader makes of it:** no authority issued the certificate, this computer did, so a PDF
reader calls the signer unknown until it trusts this machine's root certificate. The readers on this account trust it
once you have signed. On another computer it stays unknown until you give it the root (`ame vault root`) and it chooses
to trust it. With no vault, a closed one, or no certificate for documents yet, the card says so and offers the command
that fixes it.

## What is using this?

When a stick will not eject or a file will not go, select it (or the stick's folder) and pick **Scripts › What is
using this?**: a window lists the programs of yours that hold it, and what each holds (*its folder is open here*,
*report.txt is open*), each with **Quit**. Only your own programs are listed: never another account's. GNOME's own
**Volume is busy** dialog gains **Show what is using it**, and so does the dock's notice when a drive cannot be
ejected.

## Convert and shrink

Offered and off: `ame desktop files-convert on` (or **Convert pictures in Files** in [Control](control.md)) adds
**Scripts › Convert**, with **As PNG**, **As JPEG**, **As WebP**, **Half the size**, **1920 pixels wide** and **Into one
PDF**. `ame desktop files-convert off` takes the menu away.

- Each writes a new file beside the picture, never over one: `holiday.png`, `holiday (half size).jpg`,
  `holiday (1920 px).jpg`, and for several pictures one PDF named after the first, a page each.
- A picture is turned upright first, and its metadata (where a photo was taken among it) is not carried over. A JPEG
  is saved over white where the picture was transparent.
- Pictures Files cannot show cannot be converted either: an iPhone's HEIC needs a decoder Fedora does not ship.

These are scripts in `~/.local/share/nautilus/scripts`. The first five are written once for each account: delete one
and it stays gone. Convert is there exactly while it is on.

## New documents

Right-click inside a folder in Files and pick **New Document**: an **Empty text**, a **Markdown note**, a **Shell
script** or a **Python script**, with its first line written, ready to be named. They are the files in your Templates
folder, written once when it was empty, in your language; change them, add your own or delete them, and Files offers
what is there. A script made this way is a text file until you make it executable yourself.

## Downloads keeps itself tidy

Off until you turn it on, in Control's **Apps** page or with `ame desktop tidy-downloads on`. Once a day:

- installers in Downloads (`.exe`, `.msi`, `.deb`, `.rpm`, `.AppImage`, `.flatpakref` and the like) last opened more
  than a week ago go to the [trash](#the-trash);
- other files last opened and changed more than a month ago go into a folder for their month, such as `2026-09`.

The newest stay where they are. Only the files in Downloads itself are moved, never a folder, a hidden file or one
still downloading, and never over another file. Nothing is deleted: the trash keeps what it is given for 30 days. A
notice says what moved, with **Undo**, and `ame desktop tidy-downloads undo` puts the last tidying back later.

```bash
ame desktop tidy-downloads on          # off, undo, or now to tidy at once
```

## The trash

The trash empties itself of what you deleted more than 30 days ago. GNOME checks it every hour while you are logged in,
the first time an hour after you log in, and does the same to the trash of a removable drive plugged in at the time. A
file emptied from the trash is still in the [hourly snapshots and the backup](backups.md), where you have them on.

Choose another age, or never, in [Control](control.md)'s **Empty the trash**, in GNOME Settings under **Privacy**, or in
a terminal:

```bash
ame desktop privacy trash 7     # what was deleted more than a week ago
ame desktop privacy trash off   # only when you empty it yourself
```

**Empty Trash** in Files empties all of it at once, whatever the age.
