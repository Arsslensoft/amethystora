# Virus y ransomware

## Analizar en busca de virus

**Análisis de virus**, en la aplicación **Seguridad** (o `amethystora-security scan`), analiza cuando
usted lo pida:

- **Su carpeta personal**, salvo las cachés.
- **Un archivo o una carpeta**: elíjalo, o suéltelo en cualquier parte de la ventana.
- **Todo el equipo**: todas las carpetas personales y temporales, como hace el análisis semanal. No
  pide contraseña y continúa si cierra la ventana. Lo que encuentra se guarda donde solo un
  administrador puede leerlo, así que para verlo sí se pide su contraseña.

Un análisis que inicia usted se ejecuta como usted, así que comprueba lo que su cuenta puede leer. De
forma predeterminada, nada de lo que encuentra se elimina, se pone en cuarentena ni se mueve: el
resultado lista cada archivo, con **Mostrar en Archivos**, y le deja a usted la decisión. Una
coincidencia puede ser un falso positivo, así que compruebe de dónde viene un archivo antes de
eliminarlo, y después vuelva a analizar. Si configura los análisis para poner en cuarentena o
eliminar, la aplicación le pide su contraseña cuando termina el análisis e indica qué pasó con cada
archivo. Los análisis que ha hecho figuran en `~/.local/state/amethystora/security-scans.json`.

## Protección contra ransomware

```bash
ame security ransomware
```

Cada hora, una instantánea de solo lectura de cada carpeta personal de este equipo, guardada en
`/var/home/.snapshots`. Un ransomware que se ejecute como usted puede cifrar todo lo que usted puede
escribir, y una instantánea no es algo que pueda escribir: cambiar o eliminar una requiere un
administrador. Se conservan todas las instantáneas del último día, y una al día durante dos semanas.
El informe le avisa cuando la más reciente tiene más de unas horas.

Para recuperar archivos, ejecute `ame security ransomware restore` y elija el momento al que volver.
**Open it in Files** muestra su carpeta personal tal como estaba entonces, para copiar de ella;
**Put a folder back** vuelve a copiar una carpeta entera en su sitio, sustituyendo los archivos que
tiene y dejando los que no, como los que renombró el ransomware. En una instantánea no se ven los
archivos de nadie más, y los suyos no son visibles para los demás.

Necesita las carpetas personales en un subvolumen btrfs propio, que es como el instalador prepara un
disco salvo que se particione a mano. El coste es espacio en disco: una instantánea retiene lo que se
ha cambiado o eliminado desde entonces, así que un archivo que elimine para hacer sitio solo libera su
espacio cuando desaparece la última instantánea que lo contiene, hasta dos semanas después. Las
instantáneas nunca se eliminan para hacer sitio, porque entonces un ransomware que reescribiera todos
los archivos eliminaría las de antes de él. Al desactivar la protección, se ofrece eliminarlas.

Las instantáneas están en el mismo disco, y un ransomware que consiga derechos de administrador puede
eliminarlas. Guarde también una [copia de seguridad](backups.md) en otro lugar.
