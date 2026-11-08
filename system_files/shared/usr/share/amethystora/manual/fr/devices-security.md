# Les périphériques et votre écran

## Bloquer les périphériques USB que vous n’avez pas branchés

```bash
ame security usb
```

Un périphérique USB peut prétendre être un clavier et taper tout seul, et un enregistreur de frappe
matériel peut se glisser entre un vrai clavier et la machine. La protection USB autorise ce qui est
branché au moment où vous la configurez et bloque tout nouveau périphérique jusqu’à ce que vous
l’autorisiez avec `ame security usb allow`. Elle est désactivée par défaut, car une machine qui
bloquerait son propre clavier au premier démarrage serait inutilisable.

## Réaffectation des touches

Input Remapper s’exécute en tant que root et lit chaque touche tapée dans chaque application. C’est ce
dont la réaffectation a besoin, et à tous les autres égards c’est un enregistreur de frappe : il est
donc livré désactivé. Si vous réaffectez des touches ou des boutons de souris, activez-le avec
`ame security input-remapper`.
