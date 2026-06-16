# nissia browser — recetas por tipo de sitio (leer solo cuando haga falta)

Este archivo NO se carga en cada invocación: leelo cuando vayas a operar un tipo de sitio.
Mantiene la SKILL liviana (token-cheap) y acá viven los patrones.

## Principio de economía (por qué esto es barato)
- El binario `nissia` es Rust compilado: corre como instrucciones de máquina en el CPU. **No
  gasta tokens.** El `eval` corre JavaScript dentro del motor V8 de Chrome: **tampoco gasta
  tokens**. Lo único que cuesta tokens es **el texto que imprime y entra a tu contexto**.
- Regla de oro: **hacé el trabajo pesado adentro (binario / V8) y devolvé lo mínimo.**
  - Escalera de extracción, de más barata a más cara: `eval "<js que devuelve solo el dato>"`
    → `read --focus "<contenedor>"` → `snap --focus` → `snap` entero (último recurso).
  - Corré flujos en UN `batch` (una conexión, una entrada al contexto), no 10 comandos sueltos.
  - Filtrá/ordená/contá DENTRO del `eval` (V8) y devolvé solo el top-N, no la lista cruda.

## Técnica genérica: encontrar resultados SIN saber la clase
Las clases CSS de los sitios cambian seguido. No dependas de ellas. Dos caminos:

1. **Barato y robusto (texto):** `read --focus "<contenedor de resultados>"` y leé el markdown.
   Si no sabés el contenedor, probá `main`, `[role=main]`, `#search`, `.results`, o body acotado.
2. **Estructurado (un `eval`, devuelve JSON chico):** detectá ítems por **estructura**, no por
   clase. Patrón "tarjeta de producto" = un `<a>` cuyo ancestro cercano (chico) tiene un precio:
   ```js
   (function(n){var rx=/(US\$|R\$|\$|€)\s?[\d.][\d.,]{1,}/;var bad=/^(enviar|ingres|categor|ayuda|ver m|iniciar|crear)/i;var out=[],seen={};var A=document.querySelectorAll('a[href]');for(var i=0;i<A.length&&out.length<n;i++){var a=A[i];var t=(a.innerText||'').replace(/\s+/g,' ').trim();if(t.length<12||t.length>140||bad.test(t))continue;var node=a,price='',h=0;while(node&&h<5){var tx=node.innerText||'';if(tx.length<320){var m=tx.match(rx);if(m){price=m[0];break;}}node=node.parentElement;h++;}if(price&&!seen[a.href]){seen[a.href]=1;out.push({t:t.slice(0,60),price:price,url:a.href});}}return JSON.stringify(out);})(6)
   ```
   Es un punto de partida; si un sitio devuelve ruido, ajustá el largo del ancestro o el filtro.
3. **Sitio que vas a reusar:** inspeccioná UNA vez (`snap --focus body` o un `eval` que liste
   selectores), anotá el selector real del ítem, y de ahí usalo directo (o grabá con record/replay).

## Buscador robusto + descubrir elementos (validado: Wikipedia, BBC, Booking, Google Flights)
- **El input de búsqueda suele estar OCULTO/colapsado/overlay.** Si `clicksel <input>` da "not visible",
  hay un **ícono/toggle de lupa** que lo abre: clickealo primero
  (`clicksel 'button[aria-label*="search" i], button[aria-label*="buscar" i], .search-toggle, [data-testid*=search]'`),
  y DESPUÉS tipeá en el campo que quedó activo con **`typeactive <texto>`**. Enviá por botón (no Enter).
- **No adivines selectores: DESCUBRÍ.** Para clickear un link/título que no conocés, listá por estructura
  (un humano mira y elige). Ej. titulares = links con texto de titular:
  ```js
  (function(n){var out=[],seen={};var A=document.querySelectorAll('a[href]');for(var i=0;i<A.length&&out.length<n;i++){var a=A[i];var t=(a.innerText||'').replace(/\s+/g,' ').trim();if(t.length<30||t.length>120||!/^https?:/.test(a.href)||seen[a.href])continue;seen[a.href]=1;out.push({t:t.slice(0,60),h:a.href});}return JSON.stringify(out);})(6)
  ```
  Después abrí el elegido con `clicksel 'a[href="<href>"]'`. O usá `snap --focus` para ver los `@eN`.
- **eval defensivo:** nunca `.slice`/`.innerText` sobre algo que puede ser `undefined` (revienta el eval).
  Usá `(el||{}).innerText||''` y validá antes. Un eval que tira excepción te cuesta un round-trip al pedo.
- **El buscador puede NO EXISTIR hasta abrirlo** (MDN): si no encontrás ningún input de búsqueda, clickeá
  el ícono/lupa de búsqueda (el input se monta on-demand), después `typeactive`.
- **Verificá la navegación por la URL/PATH, no por `h1`** (Infobae no tenía h1): compará `location.href`
  antes/después; si no cambió el path, el click no navegó (reintentá o elegí otro elemento).
- **Clickeá links por TEXTO o `@eN` (snap), no por `href` ambiguo:** suele haber varios `<a>` con el mismo
  href (imagen + título); por href podés pegarle al equivocado. Mejor `snap` y `click @eN`, o el link cuyo innerText matchea.

## Gotchas validados en la práctica
- **Enviar búsqueda: por defecto CLICK al botón (lupa), NO `key enter`.** En DuckDuckGo, MercadoLibre y
  Wikipedia `key enter` NO dispara el submit (te quedás en la home). En **Google SÍ** funciona `key Enter`
  (validado 2026-06-16). Regla: probá el botón (siempre anda); Enter es fallback que sirve en algunos (Google).
  Autocomplete con sugerencias: `arrowdown` + `enter`.
- **`[type=submit]` en CSS NO matchea botones sin el atributo (trampa, Wikipedia).** Un `<button>` reporta
  `el.type === "submit"` por DEFAULT en JS aunque NO tenga el atributo, pero el selector CSS `[type=submit]`
  exige el atributo literal → matchea cero. Para descubrir el botón de submit: listá `form button` y filtrá
  por `el.type==='submit'` en JS, o clickealo por su CLASE de componente (ej. Wikipedia: `.cdx-search-input__end-button`).
- **DUPLICADOS responsivos: deduplicá SIEMPRE al extraer listas.** Muchos sitios renderizan cada ítem 2-3 veces
  (variantes mobile/desktop/grid ocultas). Validado: MercadoLibre da cada producto **×3** (180 nodos = 57 reales),
  Google repite resultados. Deduplicá por título o `href` (`var seen={}; if(seen[t])return; seen[t]=1;`). Lo mismo
  con calendarios: hay un duplicado mobile oculto (por eso `clicksel` hace hit-test y elige el VISIBLE).
- **Resultados async:** tras enviar, las tarjetas se renderizan después de navegar. `waitfor` un
  selector de RESULTADO (h1/contenedor/precio), NO `a[href]` (existe siempre y hasta se corta durante la navegación).
- **Extractor: inspeccioná el selector del ítem una vez, después extraé deduplicado.** Selectores que
  funcionan hoy: MercadoLibre card `[class*=poly-card]`, título `.poly-component__title`, precio
  `.andes-money-amount__fraction`, rating `.poly-component__review-compacted` (ej. "4.8"); Booking card
  `[data-testid=property-card]`, título `[data-testid=title]`, score `[data-testid=review-score]`, precio
  `[data-testid=price-and-discounted-price]`. OJO: ML no siempre muestra el rating en la grilla (vive en la
  ficha del producto); si necesitás rating, abrí los 2-3 candidatos o rankeá por relevancia+precio.
- **Genérico de párrafos/contenido: filtrá por LARGO, no por `contenedor > p`.** El `.mw-parser-output > p`
  (hijo directo) falló en Wikipedia porque el párrafo está anidado. Usá `querySelectorAll('p')` y quedate con
  los de `innerText.length > 80-120`. Más robusto a cambios de markup.
- **Sitios que "resisten" el tecleo** (ej. Booking): a veces el texto no entra (input React o anti-bot).
  Si el `value` queda vacío tras tipear: clickeá el campo y usá **`typeactive <texto>`** (tipea en el
  elemento enfocado, sirve para inputs overlay/proxy). **Si ni se enfoca** (`activeElement = BODY` y
  `elementFromPoint` sobre el input devuelve BODY, como Booking): el sitio bloquea el campo a propósito →
  **no insistas, deep-linkeá la URL de resultados** (ver gotcha anti-bot).
- **Click en SPA con lazy-load (noticias/e-commerce): el target se MUEVE.** Las imágenes cargan después del
  scroll y desplazan el elemento; un click a coordenadas viejas cae en el vacío. `clicksel` ya lo maneja
  (scroll-into-view + espera a que la posición se estabilice + re-verifica que el cursor esté sobre el target
  antes de soltar). Aun así, para navegar a un artículo **lo 100% confiable y barato es leer el `href` y
  `goto <href>`** (un click humano es para anti-bot/widgets, no hace falta para seguir un link de contenido).
- **Anti-bot: el TRUCO es saltarse la home e ir directo a la URL de resultados (re-validado 2026-06-16).**
  ML y Booking trampean el FLUJO de la home (la home de ML te redirige a la señuelo `/glossary/X/1`; el campo
  de destino de Booking no se deja enfocar: `elementFromPoint` sobre el input devuelve BODY a propósito).
  PERO la **URL de resultados con query params SÍ carga** (re-probado, ambos funcionan):
  - MercadoLibre: `https://lista.mercadolivre.com.br/<consulta-con-guiones>` (ej. `fone-de-ouvido-bluetooth`) → 50+ productos.
  - Booking: `https://www.booking.com/searchresults.html?ss=<ciudad>&checkin=YYYY-MM-DD&checkout=YYYY-MM-DD&group_adults=2&lang=pt-br` → property-cards.
  Esto invierte la regla vieja: para muros DataDome/Akamai, **NO pelees el campo de búsqueda, deep-linkeá la
  página de resultados.** SIEMPRE verificá tras navegar (URL/título/cant. de resultados); si igual te redirige a
  la señuelo, probá `reload` una vez y, si sigue, avisá al usuario y ofrecé la fuente/API oficial.
  Funcionan: Google, Wikipedia, noticias, DuckDuckGo, y ML/Booking **vía URL de resultados**.

## Sigilo: qué hace nissia y sus límites (investigado EN/ES/PT, 2026)
Cómo detectan hoy (capas combinadas, todas a la vez): (1) **red, antes del JS** — TLS JA3/JA4 + HTTP/2
(orden de frames/SETTINGS); no se puede falsear con JS. (2) **protocolo CDP** — el tell #1 es `Runtime.enable`
(emite `Runtime.consoleAPICalled`, lo cazan DataDome/Cloudflare con unas líneas de JS). (3) **fingerprint** —
canvas/WebGL/AudioContext, fuentes, `navigator.*`. (4) **comportamiento** — DataDome mira 35+ señales:
trayectoria/aceleración del mouse, velocidad de scroll, cadencia de tecleo, timing de clicks; corre miles de
modelos ML por request y SCORE por sesión. (5) **CONSISTENCIA** — que el Chrome-X-en-Windows se vea coherente
entre JS y red (geo/timezone/idioma/UA que coincidan). Inconsistencia = bandera.

Lo que nissia YA hace bien (mejor que Puppeteer/Playwright, confirmado en código 2026-06-16):
- **NO llama `Runtime.enable`** (solo `Page.enable`/`Network.enable`). Evade el tell #1 de CDP. (Patchright/nodriver
  hacen lo mismo; el parcheo JS estilo puppeteer-stealth quedó DEPRECADO y NO vence a DataDome/Akamai — no gastamos tokens ahí.)
- **Chrome real** → TLS/JA4 + canvas/WebGL/AudioContext GENUINOS (un cliente HTTP propio cae en la capa 1; nosotros no).
- `navigator.webdriver=false` sin el flag del cartel; mouse Bézier con velocidad variable, scroll con rueda real, tecleo humano.
- **Consistencia automática**: `Emulation.setTimezoneOverride` + `navigator.language/languages` arreglados (lista limpia
  sin `;q=`). Lo que ponés en `launch` (`--geo --timezone --locale --lang`) se PERSISTE y lo heredan todos los comandos.
  ⇒ usá un bloque coherente: brasil `--lang pt-BR --locale pt-BR --geo=-22.9,-43.1 --timezone America/Sao_Paulo`.

La palanca que SÍ ayuda contra muros fuertes:
- **Perfil real / "calentado"**: `nissia browser launch --profile-path <dir>` (o `NISSIA_USER_DATA_DIR`) con
  cookies/historial/sesión iniciada (navegador CERRADO para ese perfil). Pasa muchos más muros que un perfil vacío.
- **Deep-linkeá la URL de resultados** (la home trampea): ML `lista.mercadolivre.com.br/<consulta>`,
  Booking `searchresults.html?ss=...`. En FRÍO suele andar (1 búsqueda).
- **Límite honesto + ESCALADA:** DataDome/Akamai puntúan por SESIÓN. Tras varios hits automatizados ML **escala a
  `/gz/account-verification` y bloquea TODA la sesión** (validado: ni reload ni otra query la salvan); Magalu da
  "Não é possível acessar". No hay garantía. Si bloqueó: pacing más lento / menos hits / perfil calentado / volvé
  más tarde, y si no, **avisá al usuario** y ofrecé fuente/API oficial o agregador (Google) que sí responde. NO loop.

## Firmas de bloqueo (detectá que te bloquearon, no sigas a ciegas)
Después de CADA navegación, chequeá URL+título+largo del body. Está bloqueado/trampa si:
- **URL** contiene: `glossary`, `account-verification`, `/gz/`, `challenge`, `captcha`, `validateCaptcha`, `/errors/`, `__cf_chl`.
- **Título/body** matchea: `Algo deu errado` (Amazon, suele ser TRANSITORIO → `reload` 1 vez), `Não é possível acessar`
  (Magalu), `Access Denied`, `Verifique`/`Verifying you are human`, `Whoa there`/`network security` (Reddit a veces pasa),
  `unusual traffic`, `robot`, `Just a moment` (Cloudflare).
- **Body muy corto** (< ~500 chars) + sin el contenido esperado = página señuelo.
Regla: 1 `reload` para transitorios; si persiste o es escalada de sesión (account-verification), parar y avisar.

## Cómo lee / navega / busca un humano (investigado NN/g)
- **Leer un blog/artículo = ESCANEAR, no leer todo.** La gente lee ~28% de las palabras, patrón F/Z,
  se guía por **subtítulos, negritas, listas**. → Tu equivalente: **`scroll read` SIEMPRE antes de extraer**
  (recorre la página de verdad), y extraé por subtítulos (h2/h3) y listas, no palabra por palabra.
- **Buscar información = 2 modos:**
  - *Known-item* (sé qué quiero): ir directo, evaluar, seguir adelante. Pocos pasos.
  - *Exploratorio* (no sé bien): **iterativo** — query tentativa → escanear resultados → comparar varios →
    refinar la query → volver a evaluar. NUNCA agarrar el primero a ciegas; comparar 3-4 y elegir.
- **Navegar un sitio:** entrar → cerrar lo molesto (`dismiss`) → escanear arriba/izquierda → scroll leyendo →
  clickear lo relevante (descubierto, no adivinado). Usá `back`/`forward`/`reload` como una persona.
- Nota interna: `dismiss` ahora **OCULTA** (display:none) los bloqueadores en vez de removerlos, para no
  romper sitios React/SPA (algunos tiraban "Failed to execute removeChild" cuando se eliminaban sus nodos).

## Elegir la MEJOR opción (productos / servicios) — para el usuario
Cuando hay que elegir entre varias opciones, no agarres la primera ni la más barata por default.
Investigación: el rating + cantidad de reseñas pesa MÁS que el precio (88% confía en reseñas).
1. **Filtrá lo no-negociable primero** (rango de precio, categoría, marca) si lo sabés o lo pide el usuario.
2. **Rankeá por "mejor valor"**, no solo precio: buen **rating (≥4★) + muchas reseñas + precio razonable**.
   Para servicios: rating + relevancia + disponibilidad/precio. La gente escanea de arriba hacia abajo;
   las primeras suelen ser las más relevantes, pero igual compará 3-4.
3. **Presentá top 3** (título, precio, rating) **+ UNA recomendación con el porqué** (corto, escaneable).
4. **¿Preguntar o decidir?** Si el criterio que define cambia la elección y es ambiguo (lo más barato vs
   lo mejor puntuado vs lo más rápido), hacé **UNA pregunta corta** (`AskUserQuestion`). Si no, elegí
   "mejor valor" y explicá por qué (más barato en tokens). Conocés al usuario: usá eso para sesgar la elección.

## Recetas

### E-commerce (buscar producto, ver precios, abrir uno)
- **Si el sitio tiene anti-bot fuerte (MercadoLibre, Amazon): deep-linkeá la URL de resultados** (la home trampea).
  ML: `goto https://lista.mercadolivre.com.br/<producto-con-guiones>` → `scroll down` 2× (carga lazy) → extraé deduplicado.
- Sitio normal: 1) `clicksel input[type=search], input[name=q], #cb1-edit` → `typeactive <producto>`.
  2) **Enviá por botón** (no `key enter` salvo Google): `clicksel 'button[type=submit], .nav-search-btn, button[aria-label*="buscar" i]'`.
  3) `waitfor` un selector de RESULTADO (contenedor/precio) y `dismiss`.
- Extraé **deduplicado** (cada producto puede venir ×3); abrí uno con `clicksel` (ya scrollea+verifica) o `goto <href>`.
  Para "elegir el mejor": rating × cant. reseñas + precio razonable (ver sección "Elegir la MEJOR opción").

### Hoteles / booking (destino + fechas)
- **Booking bloquea el campo de destino** (no enfoca). **Deep-linkeá la URL de resultados** (validado 2026-06-16):
  `goto 'https://www.booking.com/searchresults.html?ss=<ciudad>&checkin=YYYY-MM-DD&checkout=YYYY-MM-DD&group_adults=2&lang=pt-br'`
  → `wait 3000` → `dismiss` → extraé `[data-testid=property-card]` (título/score/precio). Rankeá por score (≥8) + precio.
- Sitio de hoteles SIN anti-bot fuerte: 1) `clicksel` el campo destino → `typeactive <ciudad>` → `wait 900` → `key arrowdown` → `key enter`.
  2) Fechas: abrí el calendario → `clicksel '[aria-label*="<día> de <mes> de <año>"]'` (entrada y salida; verificá por el aria-label).
  3) `clicksel` Buscar → `waitfor` la lista → `dismiss` → extraé.

### Vuelos (validado)
Igual que hoteles pero con origen+destino+fechas ida/vuelta. Ver el ejemplo en la SKILL
(sección "Operar sitios como humano"). Origen suele autodetectarse por geolocalización.

### Noticia / artículo (leer y resumir)
1. Abrí el resultado (`search --browser --open N`, eligiendo el mejor, no el #1).
2. `scroll read` (recorre y escanea como humano) → `dismiss`.
3. `read --focus "article"` (o `main`, `#mw-content-text`) acotado a pocas líneas. No leas todo.

### Login / paywall (IMPORTANTE: no manejes credenciales)
- Si el sitio pide iniciar sesión o hay paywall: **NO ingreses usuario/contraseña vos.** Frená y
  pedile al usuario que se loguee (o que use `nissia session save` tras loguearse a mano, y luego
  `session load`). Las credenciales nunca van al repo ni al contexto.

### Feeds con scroll infinito / "cargar más"
- `scroll read` carga contenido lazy mientras escanea (acotado ~5s). Para más, repetí `scroll down`
  con `waitfor` del nuevo contenido, o `clicksel` el botón "cargar más"/"ver más".

### Locale / idioma / moneda
- Si la moneda o el idioma salen mal, lanzá con `--locale` / `--geo` / `--lang` (ver `nissia browser --help`),
  o agregá `?hl=es` / parámetros del sitio. Verificá leyendo el valor antes de seguir.

## Reusar un flujo sin gastar tokens (record / replay)
Para un sitio/flujo que repetís: `nissia record start <nombre>` → operalo una vez →
`nissia record stop` → después `nissia replay <nombre>` lo corre **sin modelo en el loop** (0 tokens).
