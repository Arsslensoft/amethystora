# Protéger vos notes

## Chiffrement

Quand le chiffrement est activé, vos notes, tâches et pièces jointes sont écrites sur le disque
chiffrées avec AES-256-GCM, sous une clé aléatoire que seule votre phrase de passe, ou une
[clé d’accès](#passkeys) que vous ajoutez, peut ouvrir. La phrase de passe est renforcée avec scrypt,
qui coûte 128 Mio de mémoire par essai : essayer des phrases de passe l’une après l’autre est donc
lent.

Il reste sûr face aux ordinateurs quantiques. Ce qu’un ordinateur quantique casse, c’est la
cryptographie à clé publique (RSA, courbes elliptiques), et il n’y en a aucune ici. Face à un
chiffrement comme AES, le mieux qu’un ordinateur quantique puisse faire est de diviser par deux la
longueur de la clé. AES-256 garde donc une robustesse de 128 bits, et c’est pourquoi AES-256 est ce que
le NIST et la CNSA 2.0 de la NSA retiennent pour l’ère des ordinateurs quantiques.

> **Personne ne peut retrouver une phrase de passe oubliée.** Ni Amethystora, ni un administrateur,
> ni qui que ce soit. Conservez-la en lieu sûr, et gardez une [sauvegarde](#backing-up).

Vos notes se verrouillent :

- après 10 minutes sans utiliser Notes (**Paramètres**, **Verrouiller en votre absence**),
- quand l’écran se verrouille, que l’ordinateur se met en veille ou que la fenêtre se ferme,
- quand vous appuyez sur **Ctrl+L** ou sur **Verrouiller** dans la barre latérale.

**Paramètres** permet aussi de changer la phrase de passe, et de désactiver ou de réactiver le
chiffrement.

### Clés d’accès

Une clé de sécurité FIDO2 (YubiKey, Thetis, ou un modèle à empreinte digitale comme la YubiKey Bio)
peut ouvrir vos notes, pour que vous n’ayez pas à saisir la phrase de passe à chaque fois. La phrase de
passe continue de fonctionner, et c’est elle qui ouvre vos notes si la clé est perdue : une clé d’accès
est une seconde porte d’entrée, jamais la seule.

Pour en ajouter une, branchez la clé, ouvrez **Paramètres** et choisissez **Ajouter une clé d’accès**.
Saisissez votre phrase de passe et le code PIN de la clé, ou laissez le code PIN vide pour une clé qui
lit votre empreinte digitale. La clé clignote deux fois ; touchez-la à chaque fois. Dès lors, l’écran de
verrouillage propose **Ouvrir avec une clé d’accès**, qui demande le code PIN de la clé, ou votre
empreinte digitale, et un contact.

- **La clé a besoin d’un code PIN**, et un modèle à empreinte digitale d’un doigt enregistré.
  Définissez-les dans Firefox à l’adresse `about:webauthn`. Un simple contact n’ouvre jamais vos
  notes : quelqu’un qui trouve la clé ne peut pas l’utiliser sans le code PIN ou votre doigt, et la clé
  bloque son code PIN après huit essais erronés.
- **Ajoutez une deuxième clé en secours**, une à la fois. Chacune est listée dans **Paramètres**, où
  **Retirer** l’empêche d’ouvrir vos notes.
- **Si une clé est perdue**, retirez-la. Quelqu’un qui détient la clé, connaît son code PIN et possède
  une ancienne copie de votre dossier de notes pourrait encore ouvrir cette copie. Désactiver puis
  réactiver le chiffrement donne une nouvelle clé à vos notes, qu’aucune clé d’accès retirée n’ouvre ;
  ajoutez ensuite de nouveau les clés que vous avez encore.
- Une [sauvegarde](#backing-up) demande toujours la phrase de passe : les clés d’accès n’en font pas
  partie.

La clé ne garde rien de vos notes. Elle détient un secret qui ne la quitte jamais et, une fois le code
PIN ou l’empreinte vérifiés, elle calcule une valeur (HMAC-SHA256, le `hmac-secret` de FIDO2) sous
laquelle est chiffrée une seconde copie de la clé de vos notes. Ce qui est sur le disque reste chiffré
avec AES-256 uniquement.

## Sauvegarder

Comme rien n’est synchronisé, gardez une copie ailleurs. Dans **Paramètres** :

- **Sauvegarder dans un fichier** écrit un seul fichier `.amnotes` qui contient tout, pièces jointes
  comprises, chiffré comme vos notes. Il s’ouvre avec la phrase de passe que vous aviez au moment de sa
  création, sur n’importe quel ordinateur : choisissez-le sous **Importer**.
- **Exporter en Markdown** écrit chaque note dans un fichier Markdown, avec un dossier par carnet, et
  chaque projet sous forme de liste de cases à cocher. Ces fichiers ne sont **pas** chiffrés.

Vos notes elles-mêmes se trouvent dans `~/.local/share/amethystora-notes`.
