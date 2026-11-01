# À propos

Amethystora est créé et maintenu par Arsslen Idadi ([@arsslensoft](https://github.com/arsslensoft)). C’est un
logiciel libre sous licence Apache 2.0, développé ouvertement sur
[github.com/arsslensoft/amethystora](https://github.com/arsslensoft/amethystora). Les tickets, les idées et
les pull requests y sont les bienvenus.

Amethystora est basé sur [Universal Blue](https://universal-blue.org), © les contributeurs d’Universal Blue, sous licence Apache 2.0.

## Ce manuel

Les pages sont des fichiers Markdown situés dans `/usr/share/amethystora/manual`, un par page (leurs
traductions françaises sont dans `/usr/share/amethystora/manual/fr`), et la page des raccourcis
clavier est `/usr/share/amethystora/keybindings.md`, le même fichier que celui qu’affiche
`ame desktop keybindings`. Elles sont fournies avec l’image : le manuel décrit donc toujours le système
que vous utilisez, et il fonctionne sans connexion réseau.

```bash
amethystora-manual                 # open it where you left off
amethystora-manual updates         # open a page by its file name
amethystora-manual --list          # the pages there are
```

Dans le manuel : `Ctrl+K` ou `/` lance une recherche, `Alt+Left` et `Alt+Right` reviennent en arrière
et avancent, `Ctrl+=` et `Ctrl+-` changent la taille du texte, et `Ctrl+0` la réinitialise.

## Sur les épaules de géants

Amethystora est construit sur [Fedora](https://fedoraproject.org) et [GNOME](https://www.gnome.org), et
sur le travail de nombreux autres projets : le noyau Linux et systemd, bootc et rpm-ostree, Flatpak et
Flathub, Homebrew, Distrobox et Podman, et chaque extension GNOME listée dans
[Se repérer](desktop.md#extensions). Les icônes sont
[candy-icons](https://github.com/EliverLara/candy-icons) d’Eliver Lara, et le thème GTK Amethystora
est une recoloration de son thème Sweet. Les fonds d’écran des thèmes sont dessinés pour Amethystora.

Fedora et le logo Fedora sont des marques de Red Hat, Inc. Amethystora n’est ni fourni, ni pris en
charge, ni approuvé par le projet Fedora ou par Red Hat. Fedora elle-même se trouve sur
[fedoraproject.org](https://fedoraproject.org).

Les licences de tout ce que contient l’image se trouvent dans `/usr/share/licenses`. Quelques parties
ne sont pas des logiciels libres et relèvent des conditions propres à leurs fabricants : les
micrologiciels des périphériques, le pilote DisplayLink pour les stations d’accueil, et le pilote NVIDIA
sur les images NVIDIA. `/usr/share/licenses/amethystora/NOTICE` les liste.

## Code source

Chaque paquet de l’image est listé dans `/usr/share/licenses/amethystora/SOURCES`, avec sa licence, le
paquet source à partir duquel il a été construit et l’endroit où se trouve ce code source. Le code
source d’un paquet Fedora se trouve dans le système de construction de Fedora, à l’adresse indiquée,
ou à une commande de distance :

```bash
dnf download --srpm <package>    # the source of the version Fedora has now
```

Quelques paquets sont construits par d’autres (RPM Fusion, negativo17, Fedora Copr), qui ne publient
que la version actuelle de chacun. Lorsque la licence d’un tel paquet exige que son code source soit
fourni, le paquet source à partir duquel cette image a été construite se trouve dans l’image elle-même,
dans `/usr/src/amethystora`.

Tout le reste est construit à partir du [dépôt d’Amethystora](https://github.com/arsslensoft/amethystora)
au commit indiqué par `BUILD_ID` dans `/usr/lib/os-release`. Pendant au moins trois ans après la
dernière publication d’une image, le code source complet de tout ce qu’elle contient et dont la licence
exige que le code source soit disponible (GNU GPL et LGPL, MPL, CDDL, licence du codec AAC de
Fraunhofer) est également fourni à quiconque le demande via le
[gestionnaire de tickets](https://github.com/arsslensoft/amethystora/issues) ; l’offre écrite se trouve
dans `/usr/share/licenses/amethystora/NOTICE`.
