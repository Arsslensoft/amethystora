# Bienvenue dans Amethystora

Amethystora est un bureau que l’on configure une fois, puis que l’on utilise, tout simplement. Il est
construit sur Fedora et GNOME, il se met à jour tout seul en arrière-plan, et une mise à jour qui
tourne mal s’annule en un redémarrage.

Ce n’est pas un tas de paquets qu’il faut tenir en ordre. Tout le système d’exploitation est une seule
image, construite, testée et signée avant même d’arriver sur votre machine, et remplacée d’un bloc
quand une nouvelle sort. Vos fichiers, vos paramètres et vos applications sont à côté, intacts. C’est
pourquoi il n’y a rien à nettoyer, aucune mise à niveau inachevée à rattraper, et aucune
réinstallation à faire tous les deux ans.

Sur ces fondations repose un bureau conçu pour se piloter au clavier, aussi beau que rapide : des
fenêtres qui se rangent sur une grille, six espaces de travail toujours là où vous les avez laissés,
un changement de thème qui repeint tout d’un coup, et un agent IA qui sait comment fonctionne ce
système.

Deux choix méritent d’être faits dès le premier jour. Si vous préférez des fenêtres qui flottent et se
chevauchent, comme sous Windows et macOS, une notification vous les propose peu après votre première
connexion, et `ame desktop layout classic` permet de basculer à tout moment
([Fenêtres classiques](tiling.md#classic-windows)). Si l’ordinateur est destiné à un enfant, le
[contrôle parental](logins.md#parental-controls) limite son compte aux applications et aux horaires
que vous choisissez.

## Comment fonctionne ce manuel

Lisez-le du début à la fin la première fois ; chaque page se termine par un lien vers la suivante.
Ensuite, utilisez la recherche : `Ctrl+K` ou `/` mène au champ de recherche, et les résultats
conduisent directement à la section dont vous avez besoin.

- **Pour commencer** prépare une nouvelle machine : les premières choses à faire, et comment
  installer.
- **Le bureau** couvre tout ce que vous touchez chaque jour : les déplacements, les raccourcis
  clavier, les thèmes, le terminal, les applications web et l’agent.
- **Logiciels** explique comment installer n’importe quoi, des applications aux chaînes de
  compilation pour développeurs, sans abîmer le système.
- **Le système** couvre les mises à jour, la sécurité et le matériel.
- **Aide** indique où chercher quand quelque chose ne va pas.

Cette page s’ouvre d’elle-même la première fois que vous vous connectez, et seulement cette fois-là.
`Super+F1` ouvre ce manuel depuis n’importe où, tout comme « Manual » dans le menu Amethystora
(`Super+Alt+Space`). Depuis un terminal, `amethystora-manual` l’ouvre et
`amethystora-manual keybindings` mène directement à une page.

## Ce qui est à vous et ce qui est à l’image

Une seule idée explique l’essentiel du comportement d’Amethystora :

| Où | À qui | Ce qui lui arrive |
| --- | --- | --- |
| `/usr` | L’image | En lecture seule. Remplacé en entier à chaque mise à jour. |
| `/etc` | Les deux | Les valeurs par défaut de l’image, fusionnées avec vos modifications à chaque mise à jour. |
| `/var` et `/home` | À vous | Jamais touchés par une mise à jour. |
| `~/.config`, `~/.local` | À vous | Vos paramètres, par compte. |

Les applications viennent de Flathub, les outils en ligne de commande de Homebrew, et les
environnements de développement vivent dans des conteneurs. Aucun d’eux ne modifie l’image : c’est
pourquoi une mise à jour ne peut jamais les casser, et pourquoi ils ne peuvent jamais casser une mise
à jour. [Installer des logiciels](software.md) les présente un par un.

> **En bref :** gardez le système d’exploitation intact, gardez votre travail à côté, et redémarrez
> quand vous avez fini votre journée. C’est tout l’entretien qu’il y a à faire.
