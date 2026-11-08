# Permissions des applications

Chaque application Flatpak s’exécute dans un bac à sable, et ses permissions disent jusqu’où, au-delà
de celui-ci, elle peut accéder sans vous le demander. La page **Applications** de l’application
Sécurité, et `ame security apps` dans un terminal, listent pour chaque application ce à quoi elle peut
accéder, et indiquent ce qui en a été accordé sur cette machine plutôt que demandé par l’application
elle-même :

| Ce qu’elle peut faire | Ce que cela signifie |
| --- | --- |
| Communiquer avec tous les services de votre session ou du système, utiliser le service Flatpak ou le systemd de votre session, ou placer un programme là où il démarre à votre prochaine connexion | Elle peut sortir de son bac à sable : tout ce qui s’y exécute peut faire tout ce que vous pouvez faire |
| Utiliser X11, ou lire les périphériques d’entrée | Elle peut voir ce que vous tapez dans d’autres applications. L’image retire les deux à toutes les applications : une application ne les a donc que s’ils lui ont été rendus |
| Utiliser tous les périphériques | Caméras, clés de sécurité et manettes de jeu, sans demander |
| Lire et modifier vos fichiers, ou seulement les lire | Tout ce qui se trouve dans votre dossier personnel, ou chaque fichier que vous pouvez ouvrir |
| Utiliser vos clés SSH ou GPG, ou lire votre trousseau | Se connecter, signer ou déchiffrer en votre nom, ou lire chaque mot de passe que vous avez enregistré |

Ce qu’une application demande par un portail, comme un fichier que vous choisissez pour elle, est
demandé à chaque fois et n’est pas listé. Le rapport de sécurité signale quand une application peut
sortir de son bac à sable, et quand X11 ou les périphériques d’entrée ont été rendus à une application.
Si ce n’était pas voulu, modifiez-le dans **Flatseal**, ou retirez tout ce que vous avez accordé à une
application avec **Retirer ce que vous avez accordé** sur la page Applications, ou :

```bash
ame security apps reset
```

## Partager des fichiers sans ce qu’ils disent de vous

Les photos, les documents et les enregistrements contiennent plus que ce qu’ils montrent : où une photo
a été prise et avec quel téléphone, qui a écrit un document et quand. **Metadata Cleaner** montre ce
qu’un fichier dit de vous et en enregistre une copie sans ces informations. Ouvrez-le depuis la grille
d’applications et ajoutez les fichiers avant de les envoyer ou de les publier.
