# Paramètres

```bash
ame security settings
```

- **Ce qu’il advient de ce que trouve une analyse antivirus** : `report` (par défaut) le laisse où il
  est, `quarantine` le déplace dans `/var/lib/amethystora/quarantine`, où rien ne peut l’ouvrir ni
  l’exécuter, et `delete` le supprime. Cela s’applique à l’analyse hebdomadaire, aux analyses que vous
  lancez dans l’application et à l’analyse en temps réel. Chaque fichier est vérifié de nouveau avant
  que quoi que ce soit ne lui soit fait. `ame security scan quarantine` liste ce qui est en
  quarantaine, et `ame security scan restore` remet un fichier en place et peut dire à l’antivirus de
  laisser tranquille ce fichier précis à l’avenir.
- **Surveillance en temps réel** : `off` (par défaut) ou `on`. Activée, la veille de sécurité lit chaque
  modification au moment où le journal d’audit l’enregistre, et chaque fichier écrit dans un dossier
  personnel est analysé au moment où il est fermé. Cela coûte un peu de processeur pendant l’écriture
  de fichiers, et l’analyse commence une fois les premières signatures de virus téléchargées.
- **Protection du réseau** : `off` (par défaut), `watch` ou `block`.
  [Protection du réseau](network-security.md#network-protection).

Les trois sont conservés dans `/etc/amethystora/security.conf`. La commande applique un changement
immédiatement ; une modification du fichier à la main s’applique au prochain redémarrage.
