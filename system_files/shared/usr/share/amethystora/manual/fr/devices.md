# Périphériques

## Imprimantes et scanners

La plupart des imprimantes sont détectées sur le réseau et fonctionnent directement, sans pilote. Des
pilotes pour les imprimantes HP et pour de nombreuses imprimantes Brother et laser plus anciennes sont
inclus pour les autres. Ajoutez une imprimante dans **Paramètres → Imprimantes**.

## Claviers, souris et clés de sécurité

- **Souris de jeu :** le service qui les configure est inclus ; installez **Piper** depuis Bazaar pour
  modifier les boutons, l’éclairage et la résolution.
- **Réaffecter des touches :** `ame security input-remapper` active Input Remapper
  ([Sécurité](devices-security.md#key-remapping) explique pourquoi il est désactivé au départ).
- Les **clés de sécurité FIDO2** peuvent remplacer votre mot de passe :
  [Sécurité](logins.md#security-keys).

## Caméras

Les caméras virtuelles (celle d’OBS, par exemple) fonctionnent sans rien installer, grâce au module
loopback intégré à l’image.
