# Sur le réseau

## Protection des navigateurs

```bash
ame security browser
```

Une extension voit chaque page que vous ouvrez, et c’est ainsi qu’arrivent la plupart des voleurs de
mots de passe. Avec la protection des navigateurs activée, les navigateurs n’installent que les
extensions d’une liste : les gestionnaires de mots de passe Bitwarden, 1Password, Proton Pass et
KeePassXC, et celles que vous ajoutez avec `ame security browser allow <id>`. Les pages web sont aussi
tenues à l’écart des périphériques USB, série, HID et Bluetooth. Elle couvre Firefox, Brave, Chrome,
Chromium et Edge, qu’ils soient installés maintenant ou plus tard, pour chaque compte de la machine.
Les autres navigateurs ne sont pas couverts.

Autorisez les extensions que vous utilisez avant de l’activer. Firefox supprime celles qui ne sont pas
sur la liste, avec leurs paramètres, à son prochain démarrage ; les autres navigateurs désactivent les
leurs. Firefox affiche l’ID de chaque extension dans `about:support`, sous Modules complémentaires, et
les autres sur leur page d’extensions avec le mode développeur activé. Les thèmes, dictionnaires et
paquets linguistiques de Firefox ne sont pas concernés.

Ce que vous avez autorisé se trouve dans `/etc/amethystora/browser-extensions`, un ID par ligne. Pour en
retirer un, supprimez sa ligne et exécutez de nouveau `ame security browser on`. Elle est désactivée
par défaut : Firefox est fourni exactement tel que Fedora le construit, et ce qu’il peut installer est
votre décision, pas celle de l’image.

## Protection du réseau

```bash
ame security settings network
```

Suricata inspecte chaque connexion que cette machine établit et reçoit au moyen des règles Emerging
Threats Open, qu’il télécharge chaque jour :

- **`watch`** signale ce qu’il reconnaît et ne bloque rien. Commencez par là : une semaine de ce mode
  montre ce que le blocage gênerait.
- **`block`** coupe aussi les connexions vers les logiciels malveillants connus et leurs serveurs de
  commande, les exploits, les pages d’hameçonnage et les mineurs de cryptomonnaie, ainsi que vers les
  adresses des listes de blocage Spamhaus et DShield. Ce que les règles reconnaissent avec moins de
  certitude est seulement signalé.

Chaque connexion bloquée fait l’objet d’une notification. **Réseau** dans l’application **Sécurité**
liste ce qui a été bloqué et signalé, tout comme `ame security connections`. Si une connexion venait de
vous et était voulue, **Autoriser**, ou `ame security connections allow <rule>`, exclut cette règle-là
sur cette machine, et `ame security connections block <rule>` la rétablit.

Quoi qu’il arrive, vos connexions continuent de fonctionner : pendant que Suricata démarre, redémarre,
prend du retard ou s’arrête, elles passent sans inspection plutôt que de ne pas passer du tout. Une
connexion chiffrée est inspectée jusqu’à sa poignée de main, le nom du serveur et le certificat, car ce
qui suit ne peut pas être lu. Son journal conserve ce qu’il a reconnu et rien d’autre, jamais une liste
des sites que vous visitez. Il inspecte les connexions propres à cette machine, pas ce qu’elle relaie
pour des machines virtuelles ou des conteneurs exécutés en tant que root, et occupe quelques centaines
de mégaoctets de mémoire. Elle est désactivée par défaut : bloquer quoi que ce soit est une décision
qui concerne votre propre trafic, et elle vous revient.
