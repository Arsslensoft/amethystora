# Notes et tâches

**Notes**, dans la grille d’applications, garde vos notes et vos listes de tâches dans une seule
fenêtre. Tout reste sur cet ordinateur. Rien n’est synchronisé, et l’application ne se connecte jamais
à Internet. Elle remplace Joplin et Planify, et reprend ce que vous y avez déjà (voir
[Vous venez de Joplin ou de Planify](#coming-from-joplin-or-planify)).

La première fois que vous l’ouvrez, elle propose de chiffrer vos notes avec une phrase de passe. C’est
le choix qu’elle recommande, et le reste de cette page suppose que vous l’avez fait.

## Notes

Les notes s’écrivent en Markdown et sont rangées dans des carnets, qui peuvent contenir d’autres
carnets. L’éditeur montre le texte et la note mise en forme côte à côte. **Ctrl+E** bascule entre le
texte, les deux, et la note mise en forme seule.

- **Listes de cases à cocher :** `- [ ] like this`. Cochez les cases dans la note mise en forme.
- **Pièces jointes :** collez une image, déposez un fichier sur la note, ou utilisez le trombone. Les
  images s’affichent dans la note. Cliquez sur toute autre pièce jointe pour en enregistrer une copie.
- **Étiquettes :** tapez dans la ligne d’étiquettes sous le titre et appuyez sur **Entrée**. Les
  étiquettes sont listées dans la barre latérale.
- **Déplacer une note :** choisissez un autre carnet en haut de la note, ou faites glisser la note sur
  un carnet de la barre latérale.
- **Versions précédentes :** l’horloge en haut d’une note. Une version est conservée chaque fois que
  vous revenez modifier une note après dix minutes sans y toucher, jusqu’aux 30 dernières. En restaurer
  une conserve aussi comme version le texte qu’elle remplace.
- **Corbeille :** une note que vous supprimez va à la corbeille, où vous pouvez la restaurer ou la
  supprimer définitivement.
- **PDF :** le menu **⋯** d’une note l’exporte en PDF.

## Tâches

**Tâches**, en haut de la barre latérale, bascule vers vos listes de tâches :

| Vue | Affiche |
| --- | --- |
| Boîte de réception | Les tâches qui ne sont dans aucun projet |
| Aujourd’hui | Ce qui est prévu aujourd’hui, et tout ce qui est en retard |
| À venir | La semaine à venir, jour par jour, et tout ce qui suit |
| Épinglées | Les tâches que vous avez épinglées |
| Terminées | Ce que vous avez terminé, le plus récent d’abord |

Les projets regroupent les tâches et peuvent être divisés en sections. Les libellés traversent les
projets. Cliquez sur une tâche pour régler sa date et son heure, sa répétition, son rappel, sa
priorité, ses libellés et ses notes, et pour ajouter des sous-tâches.

Le champ **Ajouter une tâche** comprend quelques mots d’anglais courant. Il retire ces éléments du
titre :

| Tapez | Pour |
| --- | --- |
| `today`, `tomorrow`, `friday`, `next week`, `2026-10-31` | La date |
| `5pm`, `17:30` | L’heure, avec un rappel à cette heure-là |
| `every day`, `every 2 weeks`, `monthly` | Une tâche qui se répète |
| `p1` à `p4` | La priorité, `p1` étant la plus haute |
| `@errand` | Un libellé, créé s’il est nouveau |
| `#Garden` | Un projet existant |

Terminer une tâche qui se répète la reporte à sa date suivante.

### Rappels

Une tâche avec une heure peut vous faire un rappel, de **À l’heure prévue** à **1 jour avant**. Les
rappels apparaissent sous forme de notifications tant que Notes est ouvert. Quand vous fermez la
fenêtre, Notes reste ouvert en arrière-plan jusqu’à ce que les rappels à venir se soient déclenchés.
Vous pouvez désactiver cela dans **Paramètres**. Vos notes sont verrouillées pendant ce
fonctionnement ; seuls les titres et les heures des rappels sont conservés, en mémoire.

## Vous venez de Joplin ou de Planify

Ouvrez **Paramètres** et utilisez **Importer**. Importer de nouveau le même fichier met à jour ce qui en
provient au lieu de l’ajouter deux fois.

- **Joplin :** dans Joplin, choisissez **Fichier**, **Tout exporter**, **JEX**. Importez le fichier
  `.jex`. Carnets, notes, étiquettes et pièces jointes sont repris. Les tâches de Joplin deviennent des
  tâches dans la boîte de réception.
- **Planify :** dans Planify, ouvrez **Préférences**, **Sauvegardes**, et créez une sauvegarde. Importez
  son fichier `.json` depuis
  `~/.var/app/io.github.alainm23.planify/data/io.github.alainm23.planify/backups`. Projets, sections,
  tâches, sous-tâches, libellés, dates et répétitions sont repris.
- Les **fichiers Markdown** vont dans un carnet nommé **Imported**.

Une fois tout repris, vous pouvez supprimer les anciennes applications dans Bazaar, ou avec
`flatpak uninstall net.cozic.joplin_desktop io.github.alainm23.planify`.

## Raccourcis

| Touche | Rôle |
| --- | --- |
| **Ctrl+N** | Une nouvelle note, ou une nouvelle tâche dans **Tâches** |
| **Ctrl+F** | Rechercher dans les notes ou les tâches |
| **Ctrl+1** / **Ctrl+2** | Notes / Tâches |
| **Ctrl+E** | Le texte, les deux, ou la note mise en forme |
| **Ctrl+B**, **Ctrl+I**, **Ctrl+K** | Gras, italique, lien |
| **Ctrl+L** | Verrouiller |
| **Ctrl+,** | Paramètres |
