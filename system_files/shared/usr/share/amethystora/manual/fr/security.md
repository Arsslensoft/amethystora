# Sécurité

Amethystora est renforcé au-delà des réglages par défaut de Fedora, et le dit clairement : tout ce qui
suit peut être vérifié, et la plupart peut être modifié si cela vous gêne.

## Voir où vous en êtes

Ouvrez **Sécurité** depuis la grille d’applications, ou exécutez :

```bash
ame security status
```

Les deux montrent ce que font réellement les paramètres de sécurité en ce moment, avec ce qu’il faut
faire pour tout ce qui est désactivé, et aucun des deux ne modifie quoi que ce soit ni ne demande de mot
de passe. Dans l’application, ce qui requiert votre attention vient en premier, avec la commande qui le
corrige et un bouton qui l’exécute dans un terminal, et chacune des protections ci-dessous qui restent
désactivées tant que vous n’en voulez pas est à un bouton de distance. Pour un examen plus approfondi,
`ame security audit` fait passer à Lynis quelques centaines de vérifications et explique chaque
constat.

La page **Applications** de l’application liste ce à quoi chaque application Flatpak peut accéder
au-delà de son bac à sable, et ce qui en a été accordé sur cette machine
([Permissions des applications](app-permissions.md)).

La page **Conteneurs** de l’application liste ce qui est installé hors de Flatpak : les conteneurs
créés par `amepkg`, les applications de votre grille d’applications qui en proviennent, et ce qui a été
construit depuis l’AUR, chacun avec la date de sa dernière mise à jour. Le rapport le signale quand
l’un d’eux est resté deux semaines sans mise à jour. [Conteneurs](containers.md).

Quand quelque chose requiert votre attention, **Demander à l’agent** le confie à
l’[agent IA](ai-agent.md), qui l’examine dans un terminal et ne modifie rien tant que vous n’êtes pas
d’accord.

## Activé dès le départ

- **Les mises à jour doivent être signées.** Seules les images signées avec la clé d’Amethystora sont
  installées : une image altérée ou substituée est donc refusée.
  [Mises à jour](streams.md#signed-updates).
- **Le pare-feu refuse les connexions entrantes.** Sont autorisés à entrer la découverte des
  imprimantes et des périphériques, le partage de fichiers Windows, la configuration IPv6 et
  GSConnect ; votre tailnet Tailscale est de confiance. Pour laisser entrer autre chose, comme un
  serveur de développement que vous voulez joindre depuis votre téléphone, utilisez l’application
  **Pare-feu** ou `sudo firewall-cmd --add-port=8080/tcp` (ajoutez `--permanent` pour le conserver
  après un redémarrage).
- **Dix mots de passe erronés d’affilée** verrouillent un compte pendant dix minutes. Déverrouillez-le
  plus tôt depuis un autre compte administrateur avec `sudo faillock --user <name> --reset`.
- **Les échecs de connexion répétés depuis le réseau** font bloquer l’adresse : cinq échecs en dix
  minutes coûtent une heure, et chaque nouveau bannissement de la même adresse dure plus longtemps,
  jusqu’à une journée. Cela surveille le serveur SSH, la seule chose ici à laquelle on peut se connecter
  depuis le réseau, et laisse le tailnet tranquille. `ame security addresses` liste ce qui est bloqué,
  et `ame security addresses <address>` laisse de nouveau entrer une adresse.
- **Le serveur SSH est désactivé,** et refuse les connexions root quand vous l’activez.
- **Les applications ne peuvent pas espionner le clavier les unes des autres.** Les applications
  Flatpak se voient refuser X11 et les périphériques d’entrée
  ([Installer des logiciels](software.md#what-flatpak-apps-may-not-do)), et l’outil de réaffectation
  des touches, qui lit chaque touche, est désactivé tant que vous ne le demandez pas.
- **Le noyau est durci :** la mémoire est effacée au moment où elle est attribuée, les adresses du
  noyau sont masquées, les programmes ne peuvent pas lire la mémoire les uns des autres, et les modules
  rarement utilisés (anciens protocoles réseau et systèmes de fichiers, FireWire) ne peuvent pas se
  charger. `gdb -p` sur un processus que vous n’avez pas lancé nécessite `sudo`.
- **Le noyau refuse d’être réécrit pendant qu’il tourne.** Le verrouillage bloque le chargement de
  modules non fiables, l’écriture dans `/dev/mem` et le traçage de la mémoire du noyau avec BPF : une
  compromission de root s’arrête donc au redémarrage suivant. Il nécessite Secure Boot activé et la clé
  enregistrée ([Matériel](hardware.md#secure-boot)). Sur une machine où Secure Boot ne peut pas être
  activé, retirez-le avec `sudo rpm-ostree kargs --delete=lockdown=integrity`. Les images NVIDIA ne
  l’utilisent pas.
- **Ce qui survivrait à un redémarrage est surveillé, et vous êtes prévenu.** Le journal d’audit
  enregistre les modifications des comptes, de qui peut utiliser `sudo`, de ce qui démarre tout seul
  (vos entrées de démarrage automatique, vos propres services systemd, les fichiers de démarrage du
  shell) et des paramètres de sécurité, ainsi que chaque programme qui ouvre un périphérique d’entrée.
  La veille de sécurité le lit toutes les 15 minutes et indique ce qui a changé et quel programme l’a
  changé. [La veille de sécurité](security-watcher.md).
- **Une analyse antivirus s’exécute chaque semaine** sur vos dossiers personnels et temporaires, et
  **Lynis audite les paramètres chaque mois.** Par défaut, ni l’une ni l’autre ne supprime ni ne
  déplace quoi que ce soit : un faux positif qui retire un fichier que vous vouliez est pire que la
  plupart de ce qu’elles supprimeraient. `ame security scan` montre ce qu’a trouvé la dernière analyse,
  et `ame security scan now` en lance une. [Rechercher des virus](viruses.md#scanning-for-viruses) fait de même
  dans une fenêtre, et [Paramètres](security-settings.md) permet aux analyses de mettre en quarantaine ou de
  supprimer ce qu’elles trouvent.

## Ce qui vaut la peine d’être activé
