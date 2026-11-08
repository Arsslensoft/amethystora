# Permisos de las aplicaciones

Cada aplicación Flatpak se ejecuta en un entorno aislado, y sus permisos indican hasta dónde puede
llegar fuera de él sin preguntarle. La página **Aplicaciones** de la aplicación Seguridad, y
`ame security apps` en una terminal, listan para cada aplicación a qué puede acceder, e indican qué
parte de ello se concedió en este equipo en lugar de pedirlo la propia aplicación:

| Qué puede hacer | Qué significa |
| --- | --- |
| Comunicarse con todos los servicios de su sesión o del sistema, usar el servicio de Flatpak o el systemd de su sesión, o poner un programa donde se inicie en su próximo inicio de sesión | Puede salir de su entorno aislado: lo que se ejecute en ella puede hacer todo lo que usted puede hacer |
| Usar X11, o leer los dispositivos de entrada | Puede ver lo que escribe en otras aplicaciones. La imagen quita ambas cosas a todas las aplicaciones, así que una aplicación solo las tiene si se le volvieron a conceder |
| Usar todos los dispositivos | Cámaras, llaves de seguridad y mandos de juego, sin preguntar |
| Leer y cambiar sus archivos, o solo leerlos | Todo lo que hay en su carpeta personal, o todos los archivos que usted puede abrir |
| Usar sus claves SSH o GPG, o leer su depósito de claves | Iniciar sesión, firmar o descifrar como usted, o leer todas las contraseñas que guardó |

Lo que una aplicación pide mediante un portal, como un archivo que usted elige para ella, se pregunta
cada vez y no aparece en la lista. El informe de seguridad indica cuándo una aplicación puede salir de
su entorno aislado, y cuándo a una se le devolvió X11 o los dispositivos de entrada. Si no era su
intención, cámbielo en **Flatseal**, o retire todo lo que concedió a una aplicación con
**Retirar lo que concedió** en la página Aplicaciones, o:

```bash
ame security apps reset
```

## Compartir archivos sin lo que dicen de usted

Las fotos, los documentos y las grabaciones llevan más de lo que muestran: dónde se tomó una foto y con
qué teléfono, quién escribió un documento y cuándo. **Limpiador de metadatos** (Metadata Cleaner)
muestra lo que un archivo dice de usted y guarda una copia sin ello. Ábralo desde la cuadrícula de
aplicaciones y añada los archivos antes de enviarlos o publicarlos.
