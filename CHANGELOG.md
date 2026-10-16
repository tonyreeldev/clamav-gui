# Changelog

## Sin publicar

## 1.0.11 — 2026-10-16

### Mejorado (consumo de recursos)
- La ventana solo existe mientras está abierta. Al cerrarla a la bandeja se libera la memoria de la interfaz (~400 MB), y al iniciar sesión ni siquiera se crea. La protección en tiempo real, las actualizaciones y la bandeja siguen funcionando sin ella.
- Se eliminó una recarga de firmas cada 4 horas que duplicaba la memoria de `clamd` (~2 GB) aunque no hubiera firmas nuevas. `clamd` sigue recargando solo cuando de verdad cambian.
- El panel Seguridad de Windows reutiliza su resultado durante 5 minutos, en vez de ejecutar PowerShell cada vez que se abre Resumen.

### Añadido
- **Modo de bajo consumo de memoria** (Configuración → Avanzado): libera las firmas (~1 GB) tras 15 minutos sin archivos nuevos y las vuelve a cargar cuando llega uno. Viene activado en equipos con 8 GB de RAM o menos.

## 1.0.10 — 2026-10-16

### Añadido
- Interruptores en el panel **Seguridad de Windows** para activar o desactivar el firewall, la protección de red y la protección contra ransomware. Windows pide permiso de administrador una vez por cambio.
- Botón **Activar recomendadas**, que enciende el firewall y la protección de red con un solo permiso.
- El tiempo real vigila más tipos de archivo peligrosos:
  - instaladores y ejecutables: `.msi`, `.msix`, `.scr`, `.cmd`, `.hta`, `.jar`, `.lnk`, `.wsf`, `.vbe`, `.jse`
  - documentos de Office con macros: `.docm`, `.xlsm`, `.pptm`
  - comprimidos e imágenes de disco: `.rar`, `.7z`, `.cab`, `.iso`, `.img`
- Toda detección del tiempo real queda en el historial, también en los modos que solo avisan.
- El historial de cuarentena muestra en qué archivo y ruta se encontró la amenaza, no solo su nombre.

### Corregido
- En laptops, la ventana ya no se sale de la pantalla: se ajusta al área disponible y tiene un tamaño mínimo de 800×560.
- En ventanas estrechas, la barra superior muestra solo iconos, para que todas las secciones sigan visibles (antes Estadísticas y Programador quedaban ocultas).
- El tiempo real ya no notifica "Amenaza bloqueada" si no pudo moverla a la cuarentena (por ejemplo, porque el archivo está en uso). Ahora avisa "Amenaza detectada, no se pudo aislar" y lo registra como error.
- Los archivos que todavía se están escribiendo o están bloqueados se reintentan, en vez de quedarse sin escanear.
- Los errores del tiempo real (carpetas que no se pueden vigilar, `clamd` caído, archivos no escaneables) quedan en su registro y en el historial.
- El contador de la cuarentena se actualiza también cuando el tiempo real aísla un archivo.

## 1.0.9 — 2026-10-16

### Añadido
- Panel **Seguridad de Windows** en la pantalla principal. Muestra el estado del firewall, Microsoft Defender, la protección de red, la protección contra ransomware y SmartScreen, y abre la página correspondiente de Seguridad de Windows. Solo consulta: no cambia ningún ajuste.

### Corregido
- La protección en tiempo real no se activaba en el primer arranque tras instalar, aunque está activada por defecto: la lista de carpetas vigiladas aún no existía. Ahora, si está vacía, se vigilan las carpetas de usuario habituales.
- Si una carpeta vigilada ya no existe, se omite en vez de dejar todo el equipo sin protección.
- Si el arranque de la protección falla, se reintenta cada 30 segundos.
- En una instalación nueva, `clamd` arranca en cuanto termina la primera descarga de firmas.

## 1.0.8 — 2026-10-16

### Añadido
- La protección en tiempo real usa `clamd`, que mantiene las firmas cargadas: cada archivo se revisa en milisegundos en vez de cargar la base completa por archivo.

### Corregido
- Un escaneo ya no se pierde al cambiar de sección: el progreso y los resultados se mantienen, y al volver a "Escanear" se muestra el escaneo en curso.
- El escaneo empieza de inmediato; el recuento de archivos para la barra de progreso se hace en paralelo.
- El escaneo principal ya no revisa dos veces las carpetas que están dentro de la carpeta de usuario.
- Las exclusiones se respetan en el recuento de archivos y en los escaneos programados.
- Tiempo real: reiniciarlo o cambiar de modo ya no deja vigilancia duplicada, no se bloquea durante un escaneo, espera a que el archivo termine de escribirse y respeta "reescanear al modificar" según el modo.
- Si la protección en tiempo real no puede activarse, la app lo indica en vez de mostrarla como activa.
- Borrar el historial de las últimas 24 h, 7 o 30 días ya no elimina el historial antiguo.
- El historial ya no pierde entradas cuando varias acciones escriben a la vez.
- Las exclusiones funcionan en Windows, y la carpeta de cuarentena ya no se escanea.
- Las amenazas en rutas con espacios ya se pueden poner en cuarentena o borrar desde los resultados.
- Un escaneo detenido se registra como cancelado, no como "amenazas encontradas".
- Los escaneos programados se crean aunque la app no esté en el PATH, se ejecutan también con batería y muestran su última ejecución en Windows en español.
- La ventana ya no se congela durante las acciones de cuarentena, el programador o las estadísticas.
- El desinstalador quita el inicio automático y las tareas programadas.
- Seguridad: los identificadores de registros y de cuarentena se validan, y se quitó un permiso que dejaba abrir cualquier archivo.

Detalle completo en [CAMBIOS.md](CAMBIOS.md).

## 1.0.7-2 — 2026-10-15

### Añadido
- Instalación automática de ClamAV en Windows (winget) cuando no está presente.
- Configuración de freshclam y base de firmas propias por usuario, sin permisos de administrador.
- Actualización automática de firmas al iniciar y cada 4 horas.
- Inicio con el sistema (oculto en la bandeja) e instancia única.
- Notificaciones con iconos de estado: azul (información), verde (todo bien), rojo (amenaza).
- Idioma español.
- Nueva identidad visual e interfaz minimalista.

### Corregido
- Una actualización fallida ya no se muestra como "actualizado".
- Se evitan dos actualizaciones de firmas simultáneas.
- Los ajustes booleanos ya se pueden desactivar.
- ClamAV se detecta en `C:\Program Files\ClamAV` aunque no esté en el PATH.

### Eliminado
- Actualizador automático de la aplicación: las nuevas versiones se distribuyen como instalador. Las firmas de virus siguen actualizándose solas.
