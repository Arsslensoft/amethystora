# Connexions

## Clés de sécurité

Une clé de sécurité FIDO2 (YubiKey, Thetis, ou un modèle à empreinte digitale comme la YubiKey Bio)
peut vous connecter, déverrouiller l’écran et approuver `sudo` et les demandes d’administration à la
place de votre mot de passe :

```bash
ame security key
```

Choisissez **Add a fingerprint key** pour un modèle à empreinte digitale, afin que la clé vérifie votre
empreinte et pas seulement un contact. La clé a d’abord besoin d’un code PIN, et un modèle à empreinte
digitale d’un doigt enregistré : définissez-les dans Firefox à l’adresse `about:webauthn`, ou avec
`ykman fido fingerprints add` sur une YubiKey Bio. Enregistrez une deuxième clé en secours. Votre mot de
passe continue de fonctionner chaque fois qu’aucune clé enregistrée n’est branchée.

## Connexion par empreinte digitale

Sur une machine dotée d’un lecteur d’empreintes, un doigt peut vous connecter, déverrouiller l’écran et
approuver `sudo` et les demandes d’administration, et votre mot de passe continue de fonctionner à
côté :

```bash
ame security fingerprint
```

**Add a finger** en enregistre un ; enregistrez-en un deuxième en secours. **Paramètres › Système ›
Utilisateurs** fait la même chose sous **Connexion par reconnaissance d’empreintes**. Deux choses demandent
encore le mot de passe : la phrase de passe du disque au démarrage, et le trousseau, qu’une connexion
par empreinte laisse verrouillé jusqu’à ce qu’une application en ait besoin. Les lecteurs qui
fonctionnent sont listés sur [fprint.freedesktop.org](https://fprint.freedesktop.org/supported-devices.html).

## Contrôle parental

Pour le compte d’un enfant, faites-en un compte standard, pas un administrateur, dans **Paramètres ›
Système › Utilisateurs**, puis ouvrez **Contrôle parental** depuis la grille d’applications, ou depuis
la page de ce compte dans Paramètres. Il détermine quelles applications installées le compte peut
ouvrir, s’il peut installer des applications et jusqu’à quelle classification d’âge, et, à partir de
GNOME 50, combien de temps par jour il peut utiliser l’ordinateur et à partir de quelle heure le soir il
ne le peut plus. Quand le temps est écoulé, l’écran se verrouille.
