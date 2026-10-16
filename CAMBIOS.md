# Notas de lanzamiento con historial

- **Por qué:** el usuario pidió conservar el historial de cambios de las versiones anteriores en los lanzamientos de GitHub.
- **Qué se hizo:**
  - `scripts/release-notes.py` añade a las notas de una versión un bloque desplegable por cada versión anterior, sacado de `CHANGELOG.md`.
  - El aviso de SmartScreen y los créditos quedan al final.
  - Si la versión no está en el CHANGELOG, el script falla.
  - Las notas de la v1.0.11 se actualizaron con el historial de las versiones 1.0.10, 1.0.9, 1.0.8 y 1.0.7-2. Su instalador y los lanzamientos anteriores no se tocaron.
- **Corrección del CHANGELOG:** la sección 1.0.8 incluía, por error, las novedades de la 1.0.7-2 (instalación automática de ClamAV, idioma español…), que se publicó el 15 de octubre. Ahora cada versión tiene su propia sección.

---

# v1.0.11: menos consumo de recursos

**Medición de partida** (1.0.10 en reposo, en la bandeja, en el equipo de desarrollo): `clamd` 962 MB, WebView2 437 MB (6 procesos), app 39 MB, CPU 0,2 % de un núcleo. La CPU ya estaba bien; el problema era la RAM (~1,4 GB).

## Ventana solo mientras está abierta (`helpers/window.rs`)

- `tauri.conf.json` tiene `"create": false`: la ventana no se crea al arrancar. `show_main_window` la construye cuando hace falta (al abrir desde la bandeja, con un segundo lanzamiento o al arrancar sin `--minimized`).
- Cerrarla a la bandeja la **destruye**, y con ella WebView2. `run()` evita que la app se cierre al quedarse sin ventanas (`ExitRequested` con `code: None`), salvo que el usuario elija salir. Si hay un escaneo en curso, la ventana solo se oculta, para no perder el progreso.
- Lo que antes dependía de la interfaz ahora lo hace el backend:
  - **Tiempo real:** lo arranca `start_from_settings` al iniciar la app, con reintentos cada 30 s. Sus preferencias (`realTime`, `behavior`) se copian de la interfaz al almacén de ajustes. Al abrir la ventana, la interfaz adopta la sesión en marcha (`get_real_time_status`) en lugar de reiniciarla.
  - **Idioma de la bandeja:** `boot_language` usa el idioma guardado por `set_language`; si no hay, el del sistema, y si no, inglés.
  - **Rutas pedidas sin ventana** (menú de la bandeja, `--scan=`): esperan en `take_pending_route` hasta que la página carga.

## Sin recarga forzada de firmas

- Antes se enviaba `RELOAD` a `clamd` tras cada actualización. Pero freshclam devuelve 0 también cuando no hay nada nuevo, así que la recarga ocurría cada 4 horas y duplicaba la memoria (~2 GB) durante unos segundos.
- `clamd` ya revisa la carpeta de firmas cada 10 minutos (`SelfCheck`) y solo recarga si cambió. Se eliminó `clamd::reload`.

## Panel de Seguridad de Windows

- El resultado se reutiliza durante 5 minutos. Al volver a la ventana solo se consulta de nuevo si el usuario pulsó "Abrir" o si el dato es viejo.

## Modo de bajo consumo de memoria

- `LOW_MEMORY` en `real_time.rs`. Cuando está activo:
  - `clamd` no se carga al arrancar el tiempo real.
  - Se carga con el primer archivo nuevo, en unos 5 s.
  - Se descarga tras 15 minutos sin archivos nuevos.
- La opción está en Configuración → Avanzado. Por defecto se activa en equipos con ≤ 8 GB de RAM (`lowMemoryMode` en el almacén; si no existe, decide la RAM instalada).

## Verificado en el equipo de desarrollo (sin tocar el escritorio: solo se abrió y cerró la propia app)

El usuario probó el menú de la bandeja con la ventana destruida (Abrir, Acerca de, Configuración): la ventana se reconstruye y abre en la sección pedida, con el menú en español.

| Situación | WebView2 | clamd | Tiempo real |
|---|---|---|---|
| Arranque minimizado (inicio de sesión) | **0 procesos, 0 MB** | 959 MB | Activo |
| Ventana abierta | 6 procesos, ~380 MB | 959 MB | Adoptado, sin reinicio (0 entradas nuevas) |
| Ventana cerrada a la bandeja (dos ciclos) | **0 MB** | 959 MB | Activo |
| Modo de bajo consumo, al arrancar | 0 MB | **no cargado** | Activo |
| Modo de bajo consumo, llega un archivo | 0 MB | cargado en 5 s | Activo |
| Modo de bajo consumo, sin archivos nuevos | 0 MB | **descargado** (probado con 60 s en una compilación de desarrollo: `CLAMAVGUI_IDLE_UNLOAD_SECS`, que solo existe en esas compilaciones) | Activo |

**Comportamiento real.** En la prueba de 15 minutos con el equipo en uso, `clamd` no llegó a descargarse: Firefox guarda `prefs.js` con un nombre temporal y luego lo renombra, y eso cuenta como un `.js` nuevo, así que reinicia el contador. Las firmas se liberan cuando el equipo está de verdad en reposo (mientras el usuario no está, de noche, al dejarlo en reposo), no a cada rato mientras se usa. Para que se vea qué las mantiene cargadas, el registro del tiempo real anota qué archivo provocó cada carga en este modo.

---

# v1.0.10: interruptores de seguridad, tiempo real más fiable y ventana adaptada a laptops

## Interruptores en "Seguridad de Windows"

**Por qué.** Quien no es técnico no sabe qué hacer con un botón "Abrir" o con un comando. El usuario pidió interruptores como en el resto de la app.

**Qué se puede cambiar y qué no:**

| Fila | Control | Motivo |
|---|---|---|
| Firewall | Interruptor | `Set-NetFirewallProfile -Profile Domain,Private,Public -Enabled True/False` |
| Protección de red | Interruptor | `Set-MpPreference -EnableNetworkProtection Enabled/Disabled` |
| Protección contra ransomware | Interruptor, con aviso al activar | `Set-MpPreference -EnableControlledFolderAccess ...`. Puede bloquear apps legítimas, así que se confirma antes. |
| Antivirus de Windows | "Abrir" | La protección contra alteraciones de Defender bloquea cambiarlo desde fuera (está activa en este equipo). |
| SmartScreen | "Abrir" | Está repartido entre Windows (HKLM) y Edge (HKCU); cambiarlo por registro no es fiable. |

**Cómo funciona.**
- `set_security_features` (en `system/security.rs`) solo ejecuta comandos fijos: el webview no puede pasar nada más.
- Los cambios se aplican en un PowerShell elevado (`Start-Process -Verb RunAs`), así que Windows muestra una sola ventana de permisos, también para "Activar recomendadas".
- Si se cancela el permiso, la app avisa "Cambio cancelado" (Windows devuelve el código 1223).
- Después de cada cambio se vuelve a leer el estado. Si una directiva u otro producto mantiene el valor anterior, se avisa en vez de dar el cambio por hecho.
- Desactivar algo pide confirmación.
- El envoltorio se probó sin elevación, con comandos inofensivos: un comando correcto devuelve 0 y uno que falla devuelve 1.

## Tiempo real: detecciones fiables y visibles

**Problemas encontrados al revisar qué hace el tiempo real en segundo plano:**
1. La notificación "Amenaza bloqueada" salía *antes* de intentar la cuarentena. Si el archivo estaba en uso, se decía "bloqueada" y el archivo seguía ahí.
2. En los modos que solo avisan, la detección no quedaba en ningún sitio; si no se veía la notificación, se perdía.
3. El modo por defecto no vigilaba formatos muy usados para colar malware (`.msi`, `.scr`, `.lnk`, `.hta`, Office con macros, `.rar`, `.7z`, `.iso`...).
4. Los errores solo iban a `eprintln`, que no se ve en la app compilada.
5. Un archivo que `clamd` no podía leer en ese momento (bloqueado o a medio escribir) se quedaba sin escanear.
6. El contador de la cuarentena no subía con el tiempo real.

**Solución (`helpers/real_time.rs`):**
- `handle_detection` primero intenta la cuarentena y luego notifica lo que realmente pasó. Si no se pudo aislar, el aviso es "Amenaza detectada, no se pudo aislar", con instrucciones, en los 7 idiomas.
- Cada detección que no acaba en cuarentena se guarda con el tipo `real-time-threat` (`quarantine_failed` distingue el caso). Las que sí acaban en cuarentena ya las registraba `quarantine_file`.
- Las listas de extensiones están en dos constantes (`EXECUTABLE_TYPES` y `CONTAINER_TYPES`), con su prueba unitaria.
- Los errores van al registro del tiempo real. Una caída de `clamd` también va al historial, una sola vez por sesión.
- Cada lectura fallida se reintenta hasta 5 veces, más o menos cada 3 s.
- El evento `realtime:quarantined` actualiza el contador de la interfaz.
- La entrada de historial `quarantine-threat` ahora guarda `file_path`. La interfaz muestra "X se encontró en <ruta> y se movió a la cuarentena". Las entradas antiguas, sin ruta, se siguen leyendo (hay una prueba para ello). Esto salió de una observación del usuario: el historial no decía qué archivo ni en qué ruta.

**Prueba de punta a punta en el equipo de desarrollo.** Se creó una firma temporal propia (`sigtool --md5`, con el nombre `ClamAVGUI.Prueba.Inofensiva`) para un archivo inofensivo, porque Defender bloquea EICAR. Se probaron 3 casos en Descargas:

| Caso | Resultado |
|---|---|
| A: `.exe` nuevo | Detectado, en cuarentena, notificado y registrado |
| B: `.exe` abierto en solo lectura y sin permitir moverlo (como uno en ejecución) | Detectado, la cuarentena falla (código 32), aviso "no se pudo aislar", registrado como error y sin metadatos huérfanos |
| C: archivo bloqueado del todo durante 8 s | Los reintentos lo escanean al liberarse y acaba en cuarentena |

Al terminar se borraron la firma, los archivos, sus entradas de cuarentena y las entradas de prueba del historial.

## Ventana y responsive

**Problema.**
- La ventana abría siempre a 1300×900 (unidades lógicas). En una laptop de 1920×1080 al 150 % quedan unos 1280×690 útiles, así que se salía de la pantalla.
- Con ventanas estrechas, la barra superior escondía Estadísticas y Programador en un desplazamiento horizontal sin barra visible.

**Solución.**
- `lib.rs` (`fit_to_screen`): al arrancar, la ventana se reduce al 95 % del área útil del monitor (sin la barra de tareas) y se centra.
- `tauri.conf.json`: tamaño mínimo de 800×560.
- `top-nav.tsx`: los nombres de las secciones solo aparecen desde 1280 px; por debajo, iconos con el nombre como descripción emergente y etiqueta accesible.

**Verificado.** Se renderizaron todas las pantallas en un Edge headless (sin ventana, con un perfil temporal) y el backend simulado, a 800×560, 1093×614 y 1280×690, con los tres juegos de estados del panel. El arnés de prueba no se guardó en el repositorio.

---

# v1.0.9: tiempo real desde el primer arranque y panel de Seguridad de Windows

## Panel "Seguridad de Windows"

**Por qué.** ClamAV solo analiza archivos: no tiene firewall, protección de red, filtro de webs ni protección contra ransomware. Hacer eso de verdad necesitaría un controlador del kernel firmado por Microsoft. Esas protecciones ya existen en Windows, así que el panel muestra si están activas y lleva a activarlas. Por decisión del usuario, solo abre la configuración y nunca cambia ajustes.

**Qué consulta** (todo sin permisos de administrador, comprobado en el equipo de desarrollo):

| Fila | Fuente |
|---|---|
| Firewall | `Get-NetFirewallProfile`, más `SecurityCenter2/FirewallProduct` para firewalls de otras marcas |
| Antivirus de Windows | `Get-MpComputerStatus`, más `SecurityCenter2/AntiVirusProduct` para otros antivirus |
| Protección de red | `Get-MpPreference` → `EnableNetworkProtection` |
| Protección contra ransomware | `Get-MpPreference` → `EnableControlledFolderAccess` |
| SmartScreen | Registro: `SmartScreenEnabled`, la directiva `EnableSmartScreen` y la clave de Edge en HKCU |

**Estados:**
- Activado, desactivado y activado en parte (por ejemplo, firewall apagado solo en redes públicas).
- Solo auditoría.
- Lo gestiona otro producto.
- Desactualizado, cuando las firmas de Defender tienen más de 3 días.
- No aplica: la protección de red y la de ransomware solo funcionan si Defender es el antivirus principal.
- Desconocido, cuando no se pudo leer.

**Implementación.**
- `src-tauri/src/system/security.rs`:
  - Hace una sola consulta de PowerShell que devuelve JSON. Tarda ~4 s y tiene un límite de 20 s.
  - La interpretación está en Rust, con 5 pruebas: una con la salida real de este equipo y las demás para cada estado.
  - `open_security_settings` solo abre una lista fija de URI `windowsdefender://`.
  - La protección de red no tiene interruptor en Seguridad de Windows, así que esa fila muestra el comando `Set-MpPreference -EnableNetworkProtection Enabled` para copiarlo.
- `src/components/antivirus/windows-security.tsx`:
  - Mientras carga se ve un esqueleto, y si algo falla aparece un error con reintento.
  - Se actualiza al abrirse, al volver a la ventana (como mucho cada 15 s) y con "Comprobar de nuevo".
  - En sistemas que no son Windows el panel no aparece.

**Verificado en uso real:** el panel se ve con los estados del equipo de desarrollo (firewall, Defender y SmartScreen activados; protección de red y contra ransomware desactivadas).

## El tiempo real no se activaba en el primer arranque

**Síntoma.** Tras instalar, la protección en tiempo real aparecía apagada aunque está activada por defecto.

**Causa.** En una instalación nueva, la lista de carpetas vigiladas (`monitoringPaths`) la rellena la interfaz de forma asíncrona. Pero la interfaz pide el tiempo real nada más abrirse, así que:
1. El backend recibía la lista vacía y respondía `No paths provided for monitoring` antes de escribir ningún registro.
2. No había reintento.
3. El aviso se perdía porque salía mientras se veía la pantalla de carga.

Además, una sola carpeta inexistente en la lista (por ejemplo "Música") también impedía arrancar. El diagnóstico salió de los datos de la app: `logs/realtime` no existía y el historial no tenía ninguna entrada del tiempo real.

**Solución.**
- `antivirus/mod.rs`:
  - Si la lista está vacía, se vigilan las carpetas habituales (`get_main_paths`).
  - Las carpetas que no existen se omiten y quedan en el registro.
  - Las carpetas anidadas se reducen a la más externa con `outermost_paths`, que tiene su propia prueba.
- `context/antivirus/real-time.tsx`: si el arranque falla, se reintenta cada 30 s mientras la protección siga activada en los ajustes.
- `antivirus/update.rs`: en una instalación nueva, `clamd` no puede cargar porque aún no hay firmas. Ahora se arranca en cuanto termina la primera descarga, sin esperar al siguiente archivo nuevo.

**Verificado en uso real.** Se desinstaló la app, borrando sus datos, y se abrió la 1.0.9:
- El tiempo real arrancó solo, vigilando 2 carpetas (la de usuario y la pública).
- `clamd` arrancó en 5 s con el primer archivo nuevo: respondió `PONG` y usaba ~960 MB.
- El desinstalador de la 1.0.8 había quitado la entrada de inicio automático.

---

# Cambios del 16 de octubre de 2026 (v1.0.8)

Publicado como [v1.0.8](https://github.com/tonyreeldev/clamav-gui/releases/tag/v1.0.8). Este documento resume qué se cambió en ClamAV GUI, por qué y cómo se comprobó. Sirve como registro para quien revise el código en GitHub. El resumen corto para usuarios está en [CHANGELOG.md](CHANGELOG.md).

## Resumen

1. **El tiempo real ahora usa `clamd`.** Antes lanzaba `clamscan` por cada archivo y cada archivo tardaba entre 10 y 30 s; ahora tarda milisegundos.
2. **Un escaneo ya no se pierde al cambiar de sección.**
3. **Revisión completa de la app:** se encontraron y corrigieron unos 35 problemas. Los más graves: el borrado del historial eliminaba lo que no debía, las exclusiones nunca funcionaban en Windows, los escaneos volvían a detectar la cuarentena y las rutas con espacios rompían la tabla de amenazas.

## Proceso

1. **Diagnóstico.** Se leyó el código del escaneo (`helpers/scan.rs`, `antivirus/scan.rs`) y del tiempo real (`helpers/real_time.rs`), y se buscó la causa de cada síntoma que reportó el usuario.
2. **Prueba de `clamd` en el equipo real.** Antes de programar se arrancó `clamd` 1.5.4 a mano con una configuración propia y se comprobó lo siguiente:
   - Carga las firmas en unos 9 s y usa unos 960 MB de RAM.
   - Responde por TCP y acepta rutas con tildes (`carpeta ñandú`).
   - Detecta amenazas. EICAR no sirve en este equipo porque Windows Defender lo bloquea, así que la detección se probó con una firma propia creada con `sigtool --md5`.
   - `RELOAD` y `SHUTDOWN` funcionan.
3. **Implementación** del tiempo real con `clamd` y del estado global del escaneo.
4. **Revisión completa.** Tres revisores, cada uno de solo lectura, cubrieron por separado el backend de antivirus, el backend de sistema y el frontend. Cada hallazgo se verificó en el código antes de corregirlo, y los dudosos se probaron en el equipo:
   - Las exclusiones de `clamscan` se probaron con dos formas de escapar la regex.
   - Los comandos de PowerShell del programador se probaron con una tarea temporal.
5. **Comprobación:**
   - `cargo check` y `cargo test`: 3 pruebas nuevas que pasan.
   - `tsc --noEmit` sin errores.
   - `npm run tauri build` genera el instalador.

## 1. Tiempo real con `clamd`

**Problema.**
- Cada archivo nuevo lanzaba un `clamscan`, que vuelve a cargar toda la base de firmas, y los archivos se escaneaban uno detrás de otro. Descomprimir un ZIP podía tardar minutos con la CPU al máximo.
- Además:
  - Reiniciar el tiempo real dejaba hilos duplicados.
  - Los bloqueos se mantenían durante todo el escaneo, así que la interfaz se quedaba esperando.
  - `rescan_on_modify` no se usaba en ningún sitio.
  - La interfaz mostraba la protección como activa aunque hubiera fallado al arrancar.

**Solución.**

| Archivo | Cambio |
|---|---|
| `src-tauri/src/helpers/clamd.rs` (nuevo) | Arranca `clamd` con su propio `clamd.conf` en `%LOCALAPPDATA%\com.clamavgui.app\clamav\` y le habla por TCP en `127.0.0.1:3311` (`PING`, `SCAN`, `RELOAD`, `SHUTDOWN`). El puerto es fijo para reutilizar un `clamd` que haya quedado vivo tras un cierre inesperado. Si `clamd` no arranca, se vuelve a `clamscan`. |
| `helpers/real_time.rs` | Cada sesión tiene un número de generación, y los hilos de una sesión anterior terminan solos. Un archivo se escanea cuando lleva 2 s sin cambios, así que una descarga a medio escribir no se revisa antes de tiempo. Un renombrado cuenta como archivo nuevo, porque los navegadores descargan con un nombre temporal. Las ediciones solo se reescanean en los modos que lo indican. No se retienen bloqueos mientras se escanea. |
| `antivirus/update.rs` | Después de actualizar las firmas, `clamd` las recarga. |
| `lib.rs` | Al cerrar la app se apaga `clamd`, que si no seguiría ocupando ~1 GB. |
| `context/antivirus/real-time.tsx` | Si el arranque falla, se avisa y la protección se marca como apagada. |

## 2. El escaneo se cortaba al cambiar de sección

**Problema.**
- Todo el estado del escaneo (progreso, registro, amenazas y aviso de fin) vivía dentro de la página `/scan/:type` y se borraba al salir de ella.
- El proceso seguía corriendo, pero al volver, el menú no lo mostraba.
- Si el usuario pulsaba otra vez, el backend rechazaba el escaneo nuevo y dejaba entradas de inicio sin fin en el historial.
- El escaneo Principal tardaba minutos en arrancar porque primero contaba todos los archivos.

**Solución.**

| Archivo | Cambio |
|---|---|
| `src/context/antivirus/scan.tsx` | El proveedor del escaneo ahora es global y está montado encima del router (`main.tsx`). Escucha los eventos y avisa del fin del escaneo estés donde estés. |
| `src/contents/scan.tsx`, `pages/scan.tsx` | Abrir la ruta de un escaneo lo inicia, salvo que ya haya uno en curso o con resultados sin ver. En ese caso, la página lo muestra. |
| `src/contents/scan-menu.tsx` | Con un escaneo activo, "Escanear" lleva a ese escaneo. |
| `helpers/scan.rs`, `antivirus/scan.rs` | `ScanSlot` rechaza un segundo escaneo antes de escribir en el historial. El recuento de archivos se hace en un hilo aparte, así que `clamscan` arranca al instante. |

## 3. Problemas encontrados en la revisión

### Pérdida de datos
- **Borrar "últimas 24 h / 7 / 30 días" eliminaba el historial antiguo**, no el reciente (`helpers/history.rs`, `clear_by_days`).
- **El historial podía perder entradas o vaciarse.**
  - Varios hilos escribían el mismo archivo a la vez, y un archivo ilegible se sobrescribía con una sola entrada.
  - Ahora todas las escrituras pasan por un mismo bloqueo y se guardan con archivo temporal más renombrado. Un archivo dañado se aparta como `.json.corrupt`, y `load_history` lo salta en vez de fallar entero.
- **Los escaneos Principal y Completo volvían a detectar la carpeta de cuarentena** y podían poner en cuarentena los propios archivos de cuarentena. Ahora esa carpeta se excluye siempre: en `apply_exclusions`, en el recuento, en el tiempo real y en los escaneos programados.
- **Cuarentena sin huérfanos.**
  - Los metadatos se escriben antes de mover el archivo y se borran si el movimiento falla.
  - Si se copia entre discos y el original está bloqueado, se deshace la copia.

### Exclusiones
- **Las exclusiones nunca funcionaban en Windows.** `path_to_regex` duplicaba las barras invertidas que `regex::escape` ya había escapado. Se comprobó con `clamscan --exclude-dir` y con una prueba unitaria (`helpers/path.rs`).
- **Los escaneos programados ignoraban las exclusiones.** Ahora las leen directamente de `settings.json`.

### Resultados del escaneo
- **Las rutas con espacios rompían la tabla de amenazas** (por ejemplo `C:\Users\Tony Reel\...`): la ruta se cortaba en el primer espacio y no se podía poner en cuarentena ni borrar. El nuevo parser es `/^(.*): (.+) FOUND$/`.
- **Si había errores de lectura (código 2), las amenazas encontradas se ocultaban.** Ahora se muestra la tabla.
- **Un escaneo detenido se registraba como "amenazas encontradas".** Ahora queda como `cancelled`, traducido a los 7 idiomas.
- Ya solo cuenta como amenaza una línea que termina en ` FOUND`: cualquier carpeta que se llamara `FOUND.000` sumaba amenazas falsas.
- Las líneas de salida que no son UTF-8 válido ya no se descartan.
- "Resolver todo" ya no se bloquea tras resolver la primera amenaza.
- Si el tiempo real pone un archivo en cuarentena y aparece de nuevo en la misma ruta, se vuelve a detectar.

### Seguridad
- **`read_log`, `reveal_log`, `restore_quarantine` y `delete_quarantine` solo aceptan UUID**; antes un `id` como `..\..\x` salía de su carpeta.
- **Se quitó el permiso `opener:allow-open-path` con `**`**, que dejaba al webview abrir o ejecutar cualquier archivo. En su lugar se añadió `opener:allow-reveal-item-in-dir`, que es lo que necesita "Mostrar en carpeta" y que antes faltaba.

### Escaneos programados
- **No se podían crear si la app no estaba arrancada desde su carpeta.** El ejecutable se buscaba con `where clamav-gui` y la carpeta de instalación no está en el PATH. Ahora se usa `std::env::current_exe()`.
- **No se ejecutaban con batería ni si el equipo estaba apagado a esa hora.** Ahora se configuran `AllowStartIfOnBatteries`, `DontStopIfGoingOnBatteries` y `StartWhenAvailable`.
- **"Última ejecución" no funcionaba en Windows en español.** El código buscaba la cabecera en inglés del CSV de `schtasks`, y además la fecha local daba `Invalid Date`. Ahora se usa `Get-ScheduledTaskInfo` y la fecha se devuelve en formato ISO.
- Crear una tarea duplicada ya no sobrescribe la existente antes de dar el error.
- Una tarea borrada fuera de la app ya se puede quitar de la lista.

### Interfaz y rendimiento
- **Los comandos lentos ya no congelan la ventana:**
  - cuarentena y acciones masivas
  - programador
  - estadísticas del sistema
  - `remove_file`

  Todos pasaron a `#[command(async)]`.
- **Las estadísticas de CPU y disco siempre marcaban ~0.** Se creaba un `System` nuevo en cada llamada; ahora se reutiliza uno.
- **"Escaneo personalizado" y "Escanear archivo" desde la bandeja:**
  - Los selectores ya no bloquean la ventana.
  - Las rutas con `&`, `#`, `%` o `?` ya no se cortan, porque ahora se codifican.
- **Interruptor de tiempo real:**
  - Ya no arranca ni detiene el tiempo real dos veces.
  - El interruptor, la barra superior y el resumen muestran el estado real del backend.
  - Al abrir la app ya no se registra un "detenido" falso.
- **Desde la bandeja, un escaneo nuevo funciona aunque ya estés en la página de escaneo.**
- **La pestaña Actualizar ya no se queda cargando para siempre** cuando no hay versión de la base de firmas.
- **Varios cambios rápidos en las opciones de escaneo ya no pisan los anteriores.**
- **El contador de cuarentena ya no pierde incrementos.**
- **Si la bandeja no se puede crear al iniciar sesión, la app no se cierra**: muestra la ventana.

### Desinstalación
- **El desinstalador borra la entrada de inicio automático y las tareas `ClamAV GUI - *`** (`src-tauri/windows/hooks.nsh`). No lo hace en una actualización (`/UPDATE`), para que se conserven al actualizar.

## Pendiente o fuera de alcance

- **Prueba de la interfaz en uso real.** No se hizo con la app abierta antes de publicar la v1.0.8, porque la versión anterior estaba abierta en la bandeja y la app solo permite una instancia. Hay que instalar el nuevo instalador y probar:
  - un escaneo cambiando de sección
  - el tiempo real (debe aparecer `clamd.exe` en el Administrador de tareas)
  - la cuarentena desde los resultados
  - una tarea programada
- **CSP.** Sigue en `null`. Una política restrictiva necesita probarse con la interfaz abierta, porque las banderas de idioma se cargan desde `react-circle-flags.pages.dev`.
- **`remove_file` y `delete_all` aceptan cualquier ruta**, porque así borran el archivo infectado desde los resultados. Se mantienen por diseño.
- **Traducciones.** Al francés, polaco, armenio, ruso y chino les faltan algunas claves, que se muestran en inglés. El español está completo.
- **Puerto 3311.** Si otro programa lo ocupa, el tiempo real usará `clamscan`, que es más lento.

## Publicación

- La versión pasó de 1.0.7-2 a 1.0.8 en `package.json`, `package-lock.json`, `Cargo.toml` y `tauri.conf.json`.
- Se compiló el instalador y se subieron el commit y la etiqueta `v1.0.8`.
- Se publicó el lanzamiento con `ClamAV-GUI_1.0.8_x64-setup.exe`, comprobando que el tamaño coincide con el compilado local.
- Python en Windows convirtió a CRLF los archivos que editó. Se devolvieron a LF antes del commit para que el diff mostrara solo los cambios reales.
- El procedimiento completo está en el README, en "Publicar una versión".

## Cómo compilar y probar

```bash
cd src-tauri && cargo test --lib     # pruebas de Rust
cd .. && npx tsc --noEmit            # tipos del frontend
npm run tauri build                  # instalador en src-tauri/target/release/bundle/nsis/
```
