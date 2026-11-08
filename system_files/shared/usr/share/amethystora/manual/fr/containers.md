# Conteneurs

Pour tout ce qui attend une distribution traditionnelle (une chaîne de compilation, un `.deb` fourni
par un éditeur, un paquet présent uniquement dans l’Arch User Repository, un outil réservé à Ubuntu),
créez un conteneur. Il partage votre dossier personnel et votre affichage : les applications qu’il
contient ouvrent donc des fenêtres comme les autres, et il peut être jeté sans laisser de trace. Rien
de ce qui y est installé ne touche au système.

> **Un conteneur n’est pas un bac à sable.** Ce que vous y installez s’exécute en votre nom, avec votre
> dossier personnel, votre affichage, votre son et votre bus de session. Quand une application est sur
> Flathub, prenez-la là-bas : les applications Flatpak sont isolées, signées et mises à jour avec le
> système.

## Paquets d’autres distributions

`amepkg`, abréviation d’`amethystora-pkg`, crée des conteneurs à partir de modèles (Debian, Ubuntu,
Fedora, Arch, Alpine, openSUSE Tumbleweed et openSUSE Leap) et y installe des paquets avec leurs
propres gestionnaires de paquets :

```bash
amepkg containers new --template debian
amepkg debian install gimp
amepkg debian enter
```

La première ligne télécharge l’image que nomme le modèle, crée le conteneur et le met à jour.
`install` installe depuis les dépôts signés de Debian avec `apt`, et place chaque application que le
paquet a apportée dans la grille d’applications, marquée « (on debian) » ; `remove` les retire. Avant
d’installer une application, `amepkg` cherche la même application sur Flathub et la propose d’abord.

| Commande | Rôle |
| --- | --- |
| `amepkg containers list` | Les conteneurs, leurs modèles, la date de leur dernière mise à jour, leurs applications |
| `amepkg containers new` | Un nouveau conteneur : la commande demande le modèle et le nom |
| `amepkg containers rm <name>` | Supprime le conteneur et tout ce qui y est installé |
| `amepkg containers reset <name>` | Le supprime et le recrée à partir de son modèle ; son dossier personnel reste |
| `amepkg <name> install`, `remove` ou `purge <package>` | Installe ou supprime des paquets |
| `amepkg <name> search <words>`, `show <package>` ou `list` | Parcourt ses dépôts, ou ce qui est installé |
| `amepkg <name> update`, `upgrade`, `autoremove` ou `clean` | Actualise ses listes de paquets, le met à niveau, fait le ménage |
| `amepkg <name> enter` ou `run <command>` | Un shell à l’intérieur, ou une seule commande |
| `amepkg <name> start` ou `stop` | Le démarre ou l’arrête |
| `amepkg <name> export --app <app>` ou `--bin <path>` | Place une application dans la grille d’applications, ou une commande dans `~/.local/bin` ; `unexport` la retire |

Il ne liste et ne touche que les conteneurs qu’il a créés : vos propres conteneurs Distrobox ne sont pas
concernés. Sur Arch, `update` ne fait qu’actualiser les listes de paquets, et Arch ne prend pas en
charge l’installation à partir de listes plus récentes que les paquets déjà présents : c’est `upgrade`,
qui les actualise d’abord, qu’il faut exécuter.

## Un fichier de paquet

Double-cliquez sur un `.deb` ou un `.rpm` dans Fichiers, ou depuis un terminal :

```bash
amepkg install ~/Downloads/some-app.deb
```

Un `.deb` va dans un conteneur Debian, un `.rpm` dans un conteneur Fedora et un `.pkg.tar.zst` dans un
conteneur Arch, créé à partir de son modèle s’il n’en existe pas encore. Le fichier est d’abord analysé
à la recherche de virus, et rien n’est installé s’il correspond à un logiciel malveillant connu.
Ensuite, la commande indique ce que le fichier dit de lui-même : son nom et sa version, qui l’a fait,
s’il est signé, et s’il exécute ses propres scripts pendant l’installation. Un `.rpm` peut porter une
signature ; un `.deb` n’en a normalement pas, car Debian signe ses dépôts plutôt que chaque fichier :
un `.deb` venant d’un site web ne prouve donc pas qui l’a fait. Si la même application est sur Flathub,
ou si le paquet est dans les dépôts du conteneur, ceux-ci sont proposés d’abord : les deux sont signés
et tenus à jour, et un fichier venant d’un site web n’est ni l’un ni l’autre.

Un paquet installé à partir d’un fichier n’est mis à jour avec son conteneur que si le dépôt de son
éditeur l’accompagne, comme le configurent certains paquets d’éditeurs au moment de l’installation.

## Les tenir à jour

Chaque conteneur est mis à jour une fois par jour par un minuteur de votre propre compte
(`amethystora-pkg-upgrade.timer`). Comme les mises à jour automatiques du système, il attend tant que
la machine fonctionne sur une batterie faible, est occupée ou manque de mémoire. **Mettre à jour
maintenant** dans [Mises à jour du système](updates.md#updating-now) les met aussi à jour, juste après
le système, et montre comment cela s’est passé sous **Conteneurs** : un conteneur qui ne s’est pas mis
à jour ne fait jamais échouer la mise à jour du système. Dans un terminal :

```bash
amepkg upgrade-all
```

Le [rapport de sécurité](security.md#see-where-you-stand) le signale quand un conteneur est resté deux
semaines sans mise à jour, et la page **Conteneurs** de l’application Sécurité liste ce qui est
installé dans chacun.

## Modèles et gestionnaires de paquets

Un modèle nomme l’image à partir de laquelle un conteneur démarre, le gestionnaire de paquets avec
lequel il est géré, et les paquets avec lesquels chaque nouveau conteneur commence. Un gestionnaire de
paquets, ce sont les dix commandes qu’`amepkg` exécute pour lui, de `install` à `clean`. Créez les
vôtres :

```bash
amepkg templates new --name devbox --base quay.io/toolbx/ubuntu-toolbox:24.04 \
    --pkg-manager apt --packages "build-essential git"
amepkg templates list
amepkg managers list
```

Un modèle peut donner à ses conteneurs un dossier personnel à part (`--own-home`), qui sépare leurs
paramètres des vôtres, et les options `--unshare` de distrobox (`--unshare netns,ipc`), qui conviennent
aux outils en ligne de commande ; une application avec une fenêtre a généralement besoin de ce qu’elles
retirent. Un dossier personnel à part ne cache pas vos fichiers : ils restent accessibles par leur
chemin complet. Les vôtres sont conservés dans `~/.local/share/amethystora/pkg`, et ceux qui sont
intégrés ne peuvent être ni modifiés ni supprimés. `export` en écrit un dans un fichier, et `import` le
relit, ici ou sur une autre machine.
`import` lit aussi les fichiers YAML qu’Apx écrit pour ses piles et ses gestionnaires
de paquets.

Les images des modèles intégrés sont épinglées à un condensé (digest) livré dans l’image système
signée : un conteneur démarre donc exactement de l’image avec laquelle Amethystora a été construit et
testé, et les images Debian, Alpine, openSUSE Tumbleweed et openSUSE Leap sont en plus vérifiées par
rapport aux signatures de leurs éditeurs : celles de Leap par rapport aux propres clés d’openSUSE, qui
vérifient aussi toute image Leap que vous récupérez. L’image d’un modèle à vous est prise telle que son
registre la fournit : `templates list` la marque comme non vérifiée, et créer un conteneur à partir
d’elle demande d’abord confirmation, sauf si elle est épinglée avec `@sha256:…` ou provient d’un
registre dont cette machine vérifie les signatures.

## L’AUR

L’Arch User Repository est désactivé. Activez-le avec :

```bash
ame apps aur
amepkg containers new --template arch-aur
```

Cela ajoute le modèle `arch-aur`, dont les conteneurs reçoivent un dossier personnel à part, et le
gestionnaire de paquets `paru`, qui est construit depuis l’AUR dans le conteneur la première fois.
N’importe qui peut publier dans l’AUR et personne ne vérifie ce qui s’y trouve : avant que quoi que ce
soit ne soit construit, `amepkg` montre donc ce que l’AUR en dit : qui le maintient, combien de
personnes ont voté pour lui, quand il a été soumis et mis à jour pour la dernière fois, et s’il est
signalé comme obsolète, avec un avertissement pour un paquet de moins de 30 jours ou sans vote. paru
vous montre ensuite le PKGBUILD, le script qui le construit : lisez-le. Ce que les dépôts propres
d’Arch contiennent en provient en priorité. La mise à jour quotidienne laisse de côté ce qui vient de
l’AUR, car chaque nouvelle version est un nouveau script à lire : `amepkg <name> upgrade`, dans un
terminal, le met aussi à jour.

## Dans des scripts

Les listes affichent du JSON dont les champs restent stables, avec `--json` :

- `amepkg containers list --json` : `name`, `container` (son nom dans podman), `template`, `manager`,
  `image`, `status`, `home`, `init`, `unshare`, `exported_apps`, `exported_bins`, `packages`,
  `files`, `aur_packages`, `aur_pending`, `created`, `last_upgrade` (tous deux en secondes Unix) et
  `last_upgrade_result`
- `amepkg templates list --json` : `name`, `description`, `base`, `manager`, `packages`, `own_home`,
  `unshare`, `built_in`, `verified` et `pinned`
- `amepkg managers list --json` : `name`, `need_sudo`, `noconfirm`, les dix commandes et `built_in`

`containers new`, `templates new` et `update`, ainsi que `managers new` et `update`, demandent dans un
terminal ce qu’on ne leur a pas fourni. Avec `--no-prompt`, ils n’acceptent que des options et
échouent sur ce qui manque. Les scripts, lanceurs et services l’appellent `amethystora-pkg`, son nom
complet.

## Conteneurs à la main

Distrobox et Toolbox fonctionnent comme partout ailleurs :

```bash
distrobox create --name ubuntu --image quay.io/toolbx/ubuntu-toolbox:24.04
distrobox enter ubuntu
```

**DistroShelf** affiche tous les conteneurs, les vôtres et ceux d’`amepkg`, dans une fenêtre, et le
terminal ouvre un onglet dans n’importe lequel d’entre eux depuis le menu à côté de son bouton de
nouvel onglet. Une application installée dans l’un d’eux peut être placée dans la grille
d’applications depuis l’intérieur avec `distrobox-export --app <name>`.
