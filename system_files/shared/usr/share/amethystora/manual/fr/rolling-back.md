# Revenir en arrière

Le système précédent est toujours conservé. Si une mise à jour amène un problème :

- **Au menu de démarrage,** choisissez la deuxième entrée, qui est le système d’avant la mise à jour.
  Rien n’est modifié, et le redémarrage suivant revient au plus récent. Une notification à la connexion
  indique que vous êtes sur la version précédente, et un clic dessus ouvre « Mises à jour du système »,
  qui propose **Garder cette version**, ce qui en fait celle que la machine démarre et demande votre mot
  de passe, ou **Redémarrer sur la plus récente**.
- **Pour rester sur le système précédent** depuis un terminal, faites-en le système par défaut et
  redémarrez :

```bash
sudo bootc rollback
systemctl reboot
```

Vos fichiers et paramètres sont les mêmes dans les deux. La mise à jour suivante ramène la version la
plus récente, automatique ou non : pour rester où vous êtes jusqu’à la sortie d’un correctif,
désactivez les mises à jour automatiques dans « Mises à jour du système », ou
[restez sur la version](#holding-on-to-one-build) que vous utilisez. Ensuite,
[signalez le problème](troubleshooting.md#reporting-a-problem), et suivez de nouveau le flux quand une
image corrigée est sortie.

## Rester sur une version précise

Chaque version a aussi une étiquette qui lui est propre, comme `stable-44.20260922`, d’après le numéro
indiqué dans les notes de version. Passer à cette étiquette bloque la machine sur cette version, ce
qui est utile en attendant un correctif. Une machine bloquée sur une seule version ne reçoit aucune
mise à jour ; `ame security status` vous le rappelle, et affiche la commande qui permet de suivre de
nouveau le flux.
