# Mises à jour

Les mises à jour sont automatiques et discrètes. Le système, vos applications Flatpak et vos outils
Homebrew sont vérifiés en arrière-plan et téléchargés pendant que vous travaillez. Le nouveau système
est préparé à côté de celui en cours d’exécution et prend le relais au prochain redémarrage, d’un seul
coup. Rien n’est jamais à moitié mis à jour : une mise à jour ne peut donc pas laisser la machine dans
un état où elle ne démarre plus.

> **La seule habitude à garder :** éteignez ou redémarrez quand vous avez fini votre journée. C’est à
> ce moment-là qu’une mise à jour préparée est appliquée.

## Mettre à jour maintenant

Ouvrez **Mises à jour du système** depuis la grille d’applications (ou **Check for updates** sous
**System** dans le menu Amethystora) et appuyez sur **Mettre à jour maintenant**. Cela met à jour le
système, vos applications Flatpak et vos outils Homebrew en une fois, et montre l’avancement de chacun,
sans demander de mot de passe. C’est la même mise à jour que celle qu’exécutent les mises à jour
automatiques : si l’une d’elles est déjà en cours, la fenêtre la suit au lieu d’en lancer une autre.
Vous pouvez fermer la fenêtre à tout moment ; la mise à jour continue.

Un nouveau système ne prend toujours le relais qu’au prochain redémarrage. Dès qu’il est prêt,
« Mises à jour du système » le signale et propose de redémarrer. La fenêtre affiche aussi la version en
cours d’exécution, celle qui attend le redémarrage et celle conservée pour
[revenir en arrière](#rolling-back), et permet de désactiver ou de réactiver les mises à jour
automatiques.

Si vous avez des [conteneurs](software.md#keeping-them-up-to-date) créés avec `amepkg`, **Mettre à jour
maintenant** les met à jour juste après le reste, sous **Conteneurs**. Un conteneur qui ne se met pas à
jour le signale à cet endroit, et la mise à jour du système compte tout de même comme réussie.

Dans un terminal, la même chose en une fois :

```bash
ame update
```

| Commande | Rôle |
| --- | --- |
| `ame update` | Tout mettre à jour maintenant |
| `amepkg upgrade-all` | Mettre à jour maintenant les conteneurs d’autres distributions |
| `ame system auto-updates` | Désactiver les mises à jour automatiques, ou les réactiver |
| `ame changelog` | Ce qui a changé dans les paquets depuis l’image que vous utilisez |
| `rpm-ostree status` | Le système en cours d’exécution, la mise à jour préparée et celui vers lequel revenir |
| `fwupdmgr get-updates` | Les mises à jour de micrologiciel de cette machine, de son fabricant |

Les notes de version de chaque version stable se trouvent sur
[GitHub](https://github.com/arsslensoft/amethystora/releases).

## Micrologiciels

Le micrologiciel de la machine et de ses périphériques (le micrologiciel UEFI, les stations d’accueil,
les SSD, les claviers, le lecteur d’empreintes) est mis à jour séparément du système, car une mise à
jour de micrologiciel peut exiger que la machine soit branchée et redémarrée. Chaque jour, fwupd
cherche de nouveaux micrologiciels sur LVFS, où les fabricants les publient, et la section
**Micrologiciels** de **Mises à jour** liste chaque périphérique concerné, de quelle version à quelle
version. **Mettre à jour les micrologiciels…** les installe dans un terminal, où fwupd dit ce qu’il
s’apprête à faire et demande avant de redémarrer la machine. **Vérifier maintenant** interroge de
nouveau LVFS sans attendre. Dans un terminal :

```bash
fwupdmgr get-updates      # what there is
fwupdmgr update           # install it
fwupdmgr security         # how well the firmware protects this machine
```

Le micrologiciel UEFI lui-même est écrit au prochain redémarrage, par le micrologiciel : laissez la
machine branchée jusqu’à ce qu’elle ait redémarré. Le rapport de sécurité affiche la note que fwupd
donne aux protections du micrologiciel, de HSI:0 à HSI:5 ; la note qu’une machine peut atteindre
dépend surtout de son fabricant.

## Revenir en arrière

Le système précédent est toujours conservé. Si une mise à jour amène un problème :

- **Au menu de démarrage,** choisissez la deuxième entrée, qui est le système d’avant la mise à jour.
  Rien n’est modifié, et le redémarrage suivant revient au plus récent. Une notification à la connexion
  indique que vous êtes sur la version précédente, et un clic dessus ouvre « Mises à jour du système »,
  qui propose **Garder cette version**, ce qui en fait celle que la machine démarre et demande votre mot
  de passe, ou **Redémarrer sur la plus récente**.
- **Pour rester sur le système précédent** depuis un terminal, faites-en le système par défaut et
  redémarrez :

```bash
sudo bootc rollback
systemctl reboot
```

Vos fichiers et paramètres sont les mêmes dans les deux. La mise à jour suivante ramène la version la
plus récente, automatique ou non : pour rester où vous êtes jusqu’à la sortie d’un correctif,
désactivez les mises à jour automatiques dans « Mises à jour du système », ou
[restez sur la version](#holding-on-to-one-build) que vous utilisez. Ensuite,
[signalez le problème](troubleshooting.md#reporting-a-problem), et suivez de nouveau le flux quand une
image corrigée est sortie.

## Flux

| Étiquette | Ce que vous obtenez |
| --- | --- |
| `stable` | Construite chaque semaine, avec le noyau aligné sur celui de Fedora CoreOS, qui a un peu de retard sur celui de Fedora afin que les régressions du noyau soient repérées d’abord. Pour tout le monde. |
| `stable-daily` | La même, reconstruite à chaque changement d’Amethystora plutôt que chaque semaine |
| `latest` | Le plus récent de Fedora et du noyau, reconstruit à chaque changement d’Amethystora |
| `beta` | Ce qui arrive ensuite. Attendez-vous à quelques aspérités. |

## Changer de flux et d’image

Pour passer à un autre flux ou à une autre image, changez et redémarrez :

```bash
sudo bootc switch --enforce-container-sigpolicy ghcr.io/arsslensoft/amethystora:latest
```

Remplacez `amethystora` par `amethystora-dx` pour le [mode développeur](developer.md), et `latest` par
le flux que vous voulez.

Si vous avez des [paquets superposés](software.md#layering-the-last-resort), changez plutôt avec
`rpm-ostree`, car `bootc switch` construit le nouveau système à partir de l’image seule et les
abandonnerait :

```bash
sudo rpm-ostree rebase ostree-image-signed:docker://ghcr.io/arsslensoft/amethystora:latest
```

## Rester sur une version précise

Chaque version a aussi une étiquette qui lui est propre, comme `stable-44.20260922`, d’après le numéro
indiqué dans les notes de version. Passer à cette étiquette bloque la machine sur cette version, ce
qui est utile en attendant un correctif. Une machine bloquée sur une seule version ne reçoit aucune
mise à jour ; `ame security status` vous le rappelle, et affiche la commande qui permet de suivre de
nouveau le flux.

## Mises à jour signées

Une mise à jour n’est acceptée que si elle porte la signature d’Amethystora, vérifiée avec la clé
`/etc/pki/containers/amethystora.pub`. Une image altérée, ou provenant de quelqu’un d’autre, est refusée
au lieu d’être installée. Un service vérifie à chaque démarrage que la vérification des signatures est
toujours activée, et la réactive si un changement d’image l’a désactivée.
