# Disques et dossiers verrouillés

## Déverrouiller le disque avec le TPM

```bash
ame security disk-unlock
```

Au lieu que vous saisissiez la phrase de passe du disque à chaque démarrage, le TPM de la machine
fournit la clé, mais seulement tant que Secure Boot est activé et que les clés de signature sont celles
enregistrées lors de la configuration. Ajoutez un code PIN si vous voulez que le disque exige aussi
quelque chose que vous connaissez. Votre phrase de passe continue de fonctionner : un TPM réinitialisé
ne vous bloque donc jamais dehors. Cela nécessite un disque chiffré lors de l’installation.
