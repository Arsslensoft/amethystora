# Questions et réponses

## Pourquoi ne puis-je pas utiliser `dnf install` ?

Parce que le système est une image, et que l’image est la même sur chaque machine. C’est ce qui rend
les mises à jour sûres et les retours en arrière instantanés. Les applications viennent de Flathub, les
outils de Homebrew, et tout le reste d’un conteneur ; [Installer des logiciels](software.md) indique ce
qui va où. Dans un conteneur Fedora (`amepkg containers new --template fedora`), `dnf install`
fonctionne exactement comme vous le connaissez.

## Puis-je utiliser un autre navigateur ?

Le navigateur est Firefox, exactement tel que Fedora le construit : Amethystora n’y ajoute ni
paramètres ni extensions. Chrome, Brave et tous les autres navigateurs sont dans Bazaar. Pour en faire
le navigateur par défaut, ouvrez **Paramètres → Applications → Applications par défaut** ; les
[applications web](web-apps.md#which-browser-runs-them) s’y ouvrent alors aussi.

## Où est Brave ?

Amethystora était auparavant fourni avec Brave. Il est maintenant dans Bazaar, comme les autres
navigateurs. Ce que vous y aviez se trouve toujours dans `~/.config/BraveSoftware`, et Firefox peut le
récupérer : **Paramètres → Général → Importer les données d’un navigateur**.

## Est-ce Fedora ?

En dessous, oui : Amethystora est construit sur le bureau atomique de Fedora et sur GNOME, et suit les
versions de Fedora. Ce qu’il ajoute, c’est le bureau autour, le durcissement, les outils, et une image
testée et signée avant de vous parvenir.

## Puis-je jouer à des jeux ?

Oui. `ame apps gaming` installe Steam avec ce dont ses jeux ont besoin, et accorde à Steam l’accès à
X11 par lequel il affiche, que toutes les autres applications se voient refuser. De nombreux jeux
Windows fonctionnent via Proton dans Steam. [Jeux et applications Android](games.md) indique ce que la
commande configure, et comment ajouter Heroic ou Lutris.

## Puis-je exécuter des applications Android ?

Oui, dans Waydroid : `ame apps android` le configure.
[Jeux et applications Android](games.md#android-apps).

## Puis-je exécuter des logiciels Windows ?

Souvent. Installez **Bottles** depuis Bazaar, qui exécute les programmes Windows dans leurs propres
préfixes. Wine a besoin de X11 : accordez-le donc à Bottles dans Flatseal. Pour le reste, une machine
virtuelle Windows dans l’[image pour développeurs](developer.md) convient à presque tout.

## Comment exécuter une AppImage ?

Installez **Gear Lever** depuis Bazaar. Il place les AppImage dans la grille d’applications et les
tient à jour.

## Comment installer un `.deb` ou un `.rpm` ?

Ouvrez-le dans Fichiers. Il va dans un conteneur fait pour son type de paquet, après une analyse
antivirus et un examen de sa provenance ; la même application depuis Flathub, s’il y en a une, est
proposée d’abord. [Un fichier de paquet](containers.md#a-package-file).

## Où se trouvent mes paramètres ?

Dans votre dossier personnel : `~/.config` et `~/.local` pour les applications et le bureau,
`~/.config/amethystora` pour le thème et le reste des paramètres propres à Amethystora, et `~/.var/app`
pour chaque Flatpak. Sauvegardez votre dossier personnel et vous aurez sauvegardé tout ce qui est à
vous.

## Pourquoi six espaces de travail, toujours ?

Pour qu’un numéro désigne toujours le même endroit. Avec des espaces de travail qui vont et viennent,
`Super+3` dépend de ce que vous aviez ouvert il y a une heure ; avec six espaces fixes, le navigateur sur
le 1 et le terminal sur le 2 sont toujours exactement là où vos doigts les attendent.

## Pourquoi ne puis-je pas modifier les fichiers de `/usr` ?

`/usr` est l’image, en lecture seule à dessein, et remplacée en entier à chaque mise à jour. Les
paramètres qui concernent toute la machine vont dans `/etc`, où vos modifications sont conservées et
fusionnées à chaque mise à jour. Les paramètres de votre compte vont dans votre dossier personnel.

## Puis-je faire confiance à l’image ?

Elle est construite publiquement sur GitHub à partir des sources de
[arsslensoft/amethystora](https://github.com/arsslensoft/amethystora), signée avec une clé dont la moitié
publique est livrée dans chaque image, et n’est acceptée par votre machine que si cette signature est
valide.
