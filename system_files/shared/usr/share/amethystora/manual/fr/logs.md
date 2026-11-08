# Les journaux

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
