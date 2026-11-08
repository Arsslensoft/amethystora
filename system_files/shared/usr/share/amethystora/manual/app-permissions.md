# App permissions

Every Flatpak app runs in a sandbox, and its permissions say how far beyond it the app can reach without
asking you. The **Apps** page of the Security app, and `ame security apps` in a terminal, list what each
app can reach, and which of it was granted on this machine rather than asked for by the app itself.

### What the list means

| What it can do | What that means |
| --- | --- |
| Talk to every service of your session or of the system, use the Flatpak service or your session's systemd, or put a program where it starts at your next login | It can leave its sandbox: whatever runs in it can do anything you can |
| Change the other apps installed for you | It can leave its sandbox too: it can change another app, or the launcher that starts it, which runs outside every sandbox. Every app is kept out of your own installation unless a permission names it |
| Load code chosen outside the app into it | Its environment sets a variable such as `LD_PRELOAD` that has it load a library, a plugin or a script from somewhere else. Set by an override rather than by the app, it is how a program gets inside an app, the one you type a password into, without changing the app's files |
| Use X11, or read the input devices | It can see what you type into other apps. The image takes both from every app, so an app only has them if they were granted back |
| Use every device | Cameras, security keys and game controllers, without asking |
| Read and change your files, or only read them | Everything in your home folder, or every file you can open |
| Use your SSH or GPG keys, or read your keyring | Sign in, sign or decrypt as you, or read every password you saved |
| Ask your Vault | Ask [the Vault](vault.md) for a login, a code or a signature, which you are asked to approve each time |

What an app asks for through a portal, such as a file you choose for it, is asked each time and is not
listed.

### Taking it back

The security report says when an app can leave its sandbox, and when one was given back X11, the input
devices or the Flatpak service (as the apps the image installs that need it are). If you did not mean it
to, change it in **Flatseal** (`ame security apps flatseal` opens it), or take back everything you granted
one app with **Take back what you granted** on the Apps page:

```bash
ame security apps reset
```

### Taking an app's network away

The **Network** list on the same page takes the network away from one app, as an override of your
account's own (`flatpak override --user --unshare=network`): it cannot then send anything it reads.
**Give its network back** undoes it. Either applies from the app's next start.

## Sharing files without what they say about you

Photos, documents and recordings carry more than what they show: where a photo was taken and with which
phone, who wrote a document and when.

- In Files, **Scripts › Copy without hidden data** makes a copy of each file picked without it
  ([what it takes out](files.md#copy-without-hidden-data)).
- **Metadata Cleaner** shows what a file says about you and saves a copy without it. Open it from the app
  grid and add the files before you send or publish them.

While your screen is shared, the [share guard](screen-privacy.md#share-guard) covers the windows of Notes,
Security, Backups, Help and the password managers, and the [shield](screen-privacy.md#the-shield) in quick
settings says how all of this stands and opens this app.
