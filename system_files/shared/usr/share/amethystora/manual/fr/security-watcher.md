# La veille de sécurité

Toutes les 15 minutes, ou au fil des événements avec la [surveillance en temps réel](security-settings.md)
activée, la veille examine :

- **Le journal d’audit** : les comptes, les règles `sudo`, ce qui démarre tout seul, les paramètres de
  sécurité et les modules du noyau chargés depuis une session, chacun avec le programme qui a fait la
  modification. Un programme qui ouvre directement le clavier est signalé la première fois que ce
  programme le fait ; les jeux et les outils de réaffectation des touches le font aussi.
- **Le journal** : un compte verrouillé après des mots de passe erronés, trois mots de passe `sudo` ou
  administrateur erronés ou plus, et les périphériques USB que la
  [protection USB](devices-security.md#block-usb-devices-you-did-not-plug-in) a bloqués.
- **`/etc` par rapport à l’image** : l’image garde sa propre copie de `/etc` dans `/usr/etc` ; un
  paramètre de sécurité modifié, ou un fichier de `/etc` qui remplace un de ceux de l’image dans
  `/usr/lib`, apparaît donc comme **Paramètres de l’image** dans le rapport.
- **Ce qui ne devrait pas exister sur ce système** : un programme setuid hors de `/usr`,
  `/etc/ld.so.preload`, un fichier ordinaire dans `/dev`, un module du noyau qui n’est pas fourni avec
  l’image ou n’est pas signé, et une interface réseau qui lit chaque paquet. C’est la **Recherche de
  rootkits** dans le rapport.
- **Ce qui accepte des connexions** : chaque programme qui écoute sur le réseau, que tous les appareils
  de votre tailnet peuvent joindre. Un nouveau est signalé une fois.
- **Ce que votre session trouve en premier** : un programme dans `~/.local/bin`, `~/bin` ou le dossier
  de Homebrew portant le nom d’une des commandes du système, que votre shell exécute à la place de la
  vraie. Un faux `sudo` ou `ssh` à cet endroit est une vieille ruse pour intercepter un mot de passe :
  ceux-là sont donc signalés le plus fort ; le dossier de Homebrew ne compte que pour eux. Et un lanceur
  de votre grille d’applications qu’aucun export de conteneur n’a créé. Chacun est signalé quand il
  apparaît, et de nouveau s’il change.
- **La protection du réseau**, quand elle est activée : chaque connexion qu’elle a bloquée, avec la
  façon de l’autoriser de nouveau, et ce qu’elle a reconnu sans bloquer quand la règle le juge grave.
  [Protection du réseau](network-security.md#network-protection).
- **Le rapport lui-même** : quand une vérification qui détecte la désactivation d’une protection,
  comme le pare-feu, les mises à jour signées ou le journal d’audit, cesse d’être au vert.

Tout ce qui est nouveau fait l’objet d’une notification, et attend dans le rapport jusqu’à ce que vous
le lisiez :

```bash
ame security events
```

`ame security events now` regarde d’abord. Si c’est vous qui avez fait la modification, ou installé
quelque chose qui l’a faite, il n’y a rien à faire.
