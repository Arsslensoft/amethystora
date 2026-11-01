# Dépannage

La plupart des problèmes sur un système à base d’image ont les trois mêmes réponses : découvrir ce qui
s’est passé, revenir en arrière si c’est une mise à jour qui l’a causé, et le signaler pour que la
prochaine image le corrige.

## Laisser l’agent regarder d’abord

**Diagnose a problem** dans le menu Amethystora (`Super+Alt+Space`), ou depuis un terminal :

```bash
amethystora-agent diagnose "the second monitor stays black after suspend"
```

Il lit les journaux et les rapports de plantage, ne modifie rien, et revient avec la cause et une
correction à envisager. [L’agent IA](ai-agent.md#diagnose-a-problem) en dit plus.

## Les journaux

Ouvrez **Journaux** depuis la grille d’applications, ou exécutez `amethystora-logs`. L’application
s’ouvre sur **Important**, les erreurs de tout ce qui tourne sur la machine depuis son démarrage. La
barre latérale répartit le reste entre votre session, le système, le noyau et le matériel, la sécurité
(connexions, `sudo`, ce que les règles d’audit ont relevé) et **Plantages**, chacun avec le nombre de
nouveautés depuis le dernier démarrage, là où c’est pertinent.

- **Rechercher dans les messages** parcourt ce que disent les messages, au fil de la frappe. Le bouton
  `.*` à côté traite plutôt la recherche comme une expression régulière.
- **Période** remonte au démarrage précédent, à n’importe quel démarrage que le journal conserve
  encore, à la dernière heure, au dernier jour ou à la dernière semaine, ou à tout. **Niveau** masque ce
  qui est moins grave que le niveau choisi.
- **Applications et services** liste tout ce qui a écrit dans le journal. Choisissez une application
  pour lire ce qu’elle a dit à chacune de ses exécutions, ou un service du système ou de votre session.
- Cliquez sur une entrée pour la lire en entier, avec tous les champs que le journal conserve.
  **Seulement …** restreint la vue à sa provenance, et **Autour de cette entrée** montre tout ce qui a
  été journalisé dans la minute qui précède et dans celle qui suit.
- **Suivre** ajoute les nouvelles entrées au fur et à mesure qu’elles sont écrites. **Exporter**
  enregistre ce qui est affiché dans un fichier texte ou JSON, et le bouton de terminal ouvre la même
  vue dans `journalctl`. La commande correspondant à la vue figure aussi en bas de la barre latérale.

Les administrateurs, c’est-à-dire les comptes du groupe `wheel`, voient tout. Tout autre compte ne voit
que sa propre session et ses applications, et les vues du système le signalent.

La même chose dans un terminal :

| Commande | Affiche |
| --- | --- |
| `journalctl -b -p warning` | Les avertissements et erreurs depuis ce démarrage |
| `journalctl -b -1 -p warning` | La même chose pour le démarrage précédent, après un plantage ou un gel |
| `journalctl --user -b` | Votre propre session : le bureau, vos applications |
| `systemctl --failed` | Les services qui n’ont pas réussi à démarrer |
| `coredumpctl list` | Les programmes qui ont planté |
| `ame system logs-this-boot` | Tout ce qui concerne ce démarrage |
| `ame system local-overrides` | Les fichiers de `/etc` que vous, ou quelque chose que vous avez lancé, avez modifiés |

## Une mise à jour a cassé quelque chose

Choisissez le système précédent au menu de démarrage pour confirmer que c’est bien la mise à jour.
« Mises à jour du système » propose alors de le garder, ce que fait aussi `sudo bootc rollback`, jusqu’à
ce qu’une image corrigée soit sortie. [Mises à jour](updates.md#rolling-back).

## Problèmes courants

### Un paquet que j’avais superposé a disparu

Il avait été superposé avec `rpm-ostree install`, puis la machine a changé d’image avec `bootc switch`,
qui construit le nouveau système à partir de l’image seule. Installez-le de nouveau, et changez
désormais d’image ou de flux avec `rpm-ostree rebase`
([Installer des logiciels](software.md#layering-the-last-resort)).

### Une application ne voit pas un dossier, un périphérique, ou ne s’ouvre pas du tout

Les applications Flatpak n’accèdent qu’à ce qui leur est autorisé, et sur Amethystora cela exclut X11
et les périphériques d’entrée. Ouvrez **Flatseal**, choisissez l’application, et accordez-lui ce dont
elle a besoin : un dossier sous **Filesystem**, ou **Système de fenêtrage X11** pour une
application qui ne parle pas Wayland.

### Une extension se comporte mal, ou le bureau a un aspect étrange

Désactivez l’extension dans le **Gestionnaire d’extensions**, puis déconnectez-vous et reconnectez-vous.
Si le bureau se comporte encore mal, réinitialisez les paramètres de GNOME aux valeurs par défaut de
l’image et réappliquez votre thème :

```bash
dconf reset -f /org/gnome/
amethystora-theme reload
```

Cela réinitialise tous les paramètres GNOME de ce compte, y compris le dock, vos raccourcis clavier
personnalisés et vos choix d’extensions : gardez-le pour quand rien d’autre n’a fonctionné.

### `Super+Ctrl+Space` ne change rien

Ce raccourci passe au fond d’écran suivant du thème actuel. Un thème à vous qui n’a qu’une seule image
n’a rien vers quoi passer, et une notification le signale : ajoutez-en d’autres
([Thèmes](themes.md#wallpapers)). Le sélecteur de thème est `Super+Ctrl+Shift+Space`.

### Le compte est verrouillé après des mots de passe erronés

Dix mots de passe erronés verrouillent le compte pendant dix minutes. Attendez, ou déverrouillez-le
depuis un autre compte administrateur avec `sudo faillock --user <name> --reset`.

### Une station d’accueil DisplayLink n’affiche rien

Avec Secure Boot activé, son pilote ne se charge qu’une fois la clé d’Amethystora enregistrée :
`ame security secure-boot`, puis redémarrez ([Matériel](hardware.md#secure-boot)).

### Changer de disposition de clavier

`Super+Space` est la recherche ici : la disposition de clavier suivante est donc sur
`Super+Shift+Space`. Ajoutez des dispositions dans **Paramètres → Clavier**.

### Le disque se remplit

```bash
ame system clean
```

Supprime les conteneurs, images et environnements d’exécution Flatpak inutilisés.
`flatpak uninstall --unused` et `podman system prune` en font une partie à la main.

## Signaler un problème

Signalez les bogues sur [GitHub](https://github.com/arsslensoft/amethystora/issues), et posez vos questions
dans les [discussions](https://github.com/arsslensoft/amethystora/discussions). `ame report` rassemble ce
dont un rapport a besoin dans un fichier de votre dossier personnel (l’image, vos groupes, les services
en échec et les erreurs journalisées depuis ce démarrage), vous montre tout, puis, si vous le voulez,
ouvre un nouveau ticket avec tout cela déjà rempli, sauf les erreurs. Rien n’est publié tant que vous
ne l’envoyez pas ; faites glisser le fichier dans le ticket si les erreurs peuvent aider. Sinon,
indiquez :

- la sortie de `rpm-ostree status`, qui nomme l’image exacte,
- ce que vous avez fait, ce que vous attendiez, et ce qui s’est passé à la place,
- les lignes pertinentes des journaux ci-dessus,
- si l’image précédente avait le même problème.
