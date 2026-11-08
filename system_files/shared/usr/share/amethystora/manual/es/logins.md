# Inicios de sesión

## Llaves de seguridad

Una llave de seguridad FIDO2 (YubiKey, Thetis o un modelo con huella dactilar como la YubiKey Bio)
puede iniciar su sesión, desbloquear la pantalla y aprobar `sudo` y las peticiones de administrador
en lugar de su contraseña:

```bash
ame security key
```

Elija **Add a fingerprint key** para un modelo con huella dactilar, de modo que la llave compruebe su
huella y no solo un toque. Antes, la llave necesita un PIN, y un modelo con huella dactilar, un dedo
registrado: configúrelos en Firefox, en `about:webauthn`, o con `ykman fido fingerprints add` en una
YubiKey Bio.
Registre una segunda llave de reserva. Su contraseña sigue funcionando siempre que no haya conectada
ninguna llave registrada.

## Inicio de sesión con huella dactilar

En un equipo con lector de huellas, un dedo puede iniciar su sesión, desbloquear la pantalla y aprobar
`sudo` y las peticiones de administrador, y su contraseña sigue funcionando a la vez:

```bash
ame security fingerprint
```

**Add a finger** registra uno; registre un segundo de reserva. **Configuración › Sistema › Usuarios**
hace lo mismo en **Inicio de sesión con huella**. Dos cosas siguen pidiendo la contraseña: la frase
de contraseña del disco al arrancar, y el depósito de claves, que un inicio de sesión con el dedo deja
bloqueado hasta que una aplicación lo necesita. Los lectores que funcionan están en
[fprint.freedesktop.org](https://fprint.freedesktop.org/supported-devices.html).

## Control parental

Para la cuenta de un niño, conviértala en una cuenta estándar, no de administrador, en
**Configuración › Sistema › Usuarios**, y después abra **Control parental** desde la cuadrícula de
aplicaciones, o desde la página de esa cuenta en Configuración. Decide qué aplicaciones instaladas
puede abrir la cuenta, si puede instalar aplicaciones y hasta qué clasificación por edades y, desde
GNOME 50, cuánto tiempo al día puede usar el equipo y a partir de qué hora de la noche no puede.
Cuando se acaba el tiempo, la pantalla se bloquea.
