# Ajustes

```bash
ame security settings
```

- **Qué pasa con lo que encuentra un análisis de virus**: `report` (el valor predeterminado) lo deja
  donde está, `quarantine` lo mueve a `/var/lib/amethystora/quarantine`, donde nada puede abrirlo ni
  ejecutarlo, y `delete` lo elimina. Se aplica al análisis semanal, a los análisis que inicia en la
  aplicación y al análisis en tiempo real. Cada archivo se vuelve a comprobar antes de hacer nada con
  él. `ame security scan quarantine` lista lo que hay en cuarentena, y `ame security scan restore`
  devuelve un archivo a su sitio y puede indicar al analizador que, a partir de entonces, deje en paz
  ese archivo concreto.
- **Vigilancia en tiempo real**: `off` (el valor predeterminado) u `on`. Cuando está activada, el
  vigilante de seguridad lee cada cambio en cuanto lo anota el registro de auditoría, y cada archivo
  escrito en una carpeta personal se analiza al cerrarse. Consume algo de CPU mientras se escriben
  archivos, y el análisis empieza cuando se han descargado las primeras firmas de virus.
- **Protección de red**: `off` (el valor predeterminado), `watch` o `block`.
  [Protección de red](network-security.md#network-protection).

Los tres se guardan en `/etc/amethystora/security.conf`. El comando aplica un cambio de inmediato; una
modificación manual del archivo se aplica en el siguiente reinicio.
