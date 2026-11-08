# Copias de seguridad

```bash
ame backup
```

Una copia de seguridad diaria con restic en un repositorio del que el ransomware de este equipo no
puede borrar nada. La copia solo añade al repositorio y nunca elimina instantáneas antiguas, así que
un repositorio de solo anexado en el otro extremo conserva todo lo copiado antes de un ataque. El
comando explica cómo configurar uno en rest-server, Borg, un almacenamiento de objetos con
inmutabilidad o una unidad USB que desconecte entre copias. `ame backup now` hace una copia de
inmediato, y `ame backup snapshots` lista lo que hay.

Cada copia guarda también su configuración junto a sus archivos: la imagen y el canal que sigue, sus
aplicaciones Flatpak y de dónde vinieron, sus paquetes de Homebrew, los contenedores que creó `amepkg`
con lo que instaló en ellos, las extensiones de GNOME que tenía activadas y su tema. No incluye
contraseñas, claves ni tokens. En un equipo nuevo, apunte `ame backup` al mismo repositorio con la
misma contraseña, y después:

```bash
ame restore-setup
```

Primero muestra lo que haría, y solo hace las partes que elija: cambiar a la misma imagen, mediante
`ame system rebase`, que solo acepta imágenes firmadas; instalar las aplicaciones y los paquetes de
Homebrew; volver a crear los contenedores e instalar lo que había en ellos; volver a activar las
extensiones; poner el tema. `ame restore-setup <snapshot>` elige una configuración anterior. Sus
archivos en sí se recuperan con restic.
