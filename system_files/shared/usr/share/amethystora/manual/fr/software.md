# Installer des logiciels

Il n’y a pas de `dnf install` ici, et il ne vous manquera pas. Chaque type de logiciel a sa place,
aucune ne modifie l’image du système, et aucune application ne peut donc jamais casser une mise à jour
ni être cassée par l’une d’elles.

| Ce que vous voulez | Utilisez | Comment |
| --- | --- | --- |
| Une application avec une fenêtre | Flatpak, depuis Flathub | Bazaar, la logithèque |
| Un outil en ligne de commande | Homebrew | `brew install <name>` |
| Tout un espace utilisateur Linux : `apt`, `dnf`, `pacman`, une chaîne de compilation | Un conteneur | `amepkg` ([Conteneurs](containers.md)) |
| Un `.deb` ou un `.rpm` venant d’un site web | Un conteneur fait pour lui | L’ouvrir dans Fichiers ([Un fichier de paquet](containers.md#a-package-file)) |
| Un site web comme application | Une application web | `amethystora-webapp install` ([Applications web](web-apps.md)) |
| Quelque chose qui doit être sur l’hôte lui-même | La superposition | `rpm-ostree install`, en dernier recours |

## Applications : Bazaar et Flathub

Ouvrez **Bazaar** et installez depuis [Flathub](https://flathub.org), où presque toutes les
applications de bureau Linux sont publiées. Les applications Flatpak se mettent à jour avec le
système, et en supprimer une ne laisse rien derrière elle. Depuis un terminal :

```bash
flatpak install flathub org.gimp.GIMP
flatpak list --app
flatpak uninstall --delete-data org.gimp.GIMP
```

Deux autres outils sont là pour les Flatpak :

- **Flatseal** affiche et modifie ce à quoi chaque application a le droit d’accéder : dossiers,
  périphériques, réseau.
- **Warehouse** gère les applications installées et nettoie ce qu’elles laissent derrière elles.

### Ce que les applications Flatpak n’ont pas le droit de faire

Tout Flatpak se voit refuser X11, `/dev/input` et le service Flatpak lui-même, quoi que demande son
propre manifeste. Ensemble, c’est par là qu’une application pourrait lire ce que vous tapez dans une
autre. Les applications écrites pour Wayland ne s’en aperçoivent pas. Une application qui ne parle que
X11 (Wine, certains jeux, certaines anciennes versions d’Electron) doit le récupérer, une application à
la fois, dans Flatseal ou avec :

```bash
flatpak override --user --socket=x11 <app-id>
```

Ce à quoi chaque application peut accéder au-delà de son bac à sable, ce qu’elle a demandé et ce que
vous lui avez accordé, figure sur la page **Applications** de l’application Sécurité, et dans un
terminal avec `ame security apps` ([Permissions des applications](app-permissions.md)).

## Outils en ligne de commande : Homebrew

```bash
brew install ripgrep fd bat
brew search <name>
```

Homebrew s’installe dans son propre préfixe, sans `sudo`, et est mis à jour avec le système. La page
[Le terminal](terminal.md#command-line-tools) en dit plus.

## Superposition : le dernier recours

`rpm-ostree install` ajoute un paquet à l’image sur cette machine. Ne l’utilisez que pour ce qui ne
peut vivre nulle part ailleurs, comme un pilote ou un service système :

```bash
sudo rpm-ostree install <package>
```

Redémarrez pour l’utiliser. Chaque paquet superposé ralentit chaque mise à jour, et le paquet n’est
conservé que tant que vous faites vos mises à jour avec `rpm-ostree`. Deux choses à savoir :

- **Changer d’image ou de flux :** `bootc switch` construit le nouveau système à partir de l’image
  seule et abandonne sans un mot les paquets superposés. Sur une machine avec des paquets superposés,
  changez plutôt avec
  `sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/<image>:<stream>`. Les mises
  à jour automatiques s’en chargent pour vous.
- **L’annuler :** `sudo rpm-ostree uninstall <package>` en retire un, et `sudo rpm-ostree reset`
  revient à l’image d’origine.

Ajouter un dépôt `dnf` ou exécuter `dnf install` sur l’hôte ne fonctionne pas : le système n’a pas de
base de données de paquets accessible en écriture.

## VPN

- **Tailscale** est installé, et vous pouvez l’utiliser sans `sudo` : `tailscale up` rejoint votre
  tailnet. Le pare-feu fait confiance au tailnet, où s’appliquent les règles d’accès de votre propre
  tailnet.
- Les configurations **WireGuard** de n’importe quel fournisseur peuvent être importées dans
  **Paramètres → Réseau → VPN**.
