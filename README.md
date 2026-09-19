# Kiosquito Flow API

Backend REST para un kiosco, construido con Rust, Axum, SQLx y PostgreSQL. Incluye autenticacion JWT, inventario, ventas, clientes, caja, gastos, compras, cuentas corrientes y reportes financieros.

## Requisitos

- Rust y Cargo.
- PostgreSQL.
- Base de datos creada y cargada con `kiosco_flow.sql`.

El archivo SQL de la raiz es el esquema utilizado por la aplicacion. Contiene tablas, enums, claves foraneas, indices y secuencias para usuarios, productos, ventas, compras, caja, cuentas corrientes, gastos y auditoria.

## Configuracion

Crea un archivo `.env` en la raiz:

```env
DATABASE_URL=postgres://usuario:contrasena@localhost:5432/kiosquito
JWT_SECRET=una-clave-de-al-menos-16-caracteres
PORT=3000
FRONTEND_URL=http://localhost:5173
```

Variables:

| Variable | Obligatoria | Uso |
| --- | --- | --- |
| `DATABASE_URL` | Si | Conexion PostgreSQL. |
| `JWT_SECRET` | Si para login | Firma de tokens JWT. |
| `PORT` | No | Puerto HTTP; por defecto `3000`. |
| `FRONTEND_URL` | No | Origen CORS; por defecto `http://localhost:5173`. |
| `INITIAL_ADMIN_USERNAME` | No | Usuario administrador inicial si no existen usuarios. |
| `INITIAL_ADMIN_PASSWORD` | No | Password inicial, minimo 8 caracteres. |

Al iniciar, el backend asegura el cliente `Consumidor Final` con id `1`, sincroniza su secuencia y crea el administrador inicial si se proporcionan las dos variables `INITIAL_ADMIN_*` y la tabla de usuarios esta vacia.

## Instalacion y ejecucion

```powershell
cargo build
cargo run
```

La API queda disponible en `http://localhost:3000`.

Comandos de verificacion:

```powershell
cargo fmt --check
cargo check
cargo test
```

Actualmente no hay tests automatizados de integracion; `cargo test` verifica que el proyecto compile y ejecuta la suite existente, que todavia no contiene casos.

## Autenticacion

El healthcheck es publico:

```http
GET /health
```

Respuesta normal:

```json
{"status":"ok"}
```

El login tambien es publico:

```http
POST /api/auth/login
Content-Type: application/json

{"username":"admin","password":"una-password"}
```

La respuesta contiene `message`, `token` y `user`. El token dura una hora y debe enviarse asi:

```http
Authorization: Bearer <token>
Content-Type: application/json
```

Las rutas financieras, de inventario, categorias, proveedores y usuarios requieren JWT. Las operaciones de administracion requieren el rol `Administrador`.

- `401`: token ausente, invalido o vencido.
- `403`: rol insuficiente.
- `400`: datos invalidos o regla de negocio incumplida.
- `404`: entidad inexistente.

Los errores se devuelven como `{ "message": "..." }`.

Despues de cinco intentos fallidos de login, el usuario queda bloqueado durante cinco minutos.

## Rutas

Todas las rutas siguientes usan el prefijo `/api`.

### Usuarios

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `GET` | `/auth/users` | Administrador |
| `POST` | `/auth/users` | Administrador |
| `PUT` | `/auth/users/:id` | Administrador |
| `DELETE` | `/auth/users/:id` | Administrador |

Tambien existen los aliases `/users` y `/users/:id`.

No se puede eliminar la propia cuenta, degradar o eliminar al ultimo administrador, ni eliminar usuarios con operaciones asociadas.

### Catalogos e inventario

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `GET` | `/categories` | Auth |
| `POST` | `/categories` | Administrador |
| `GET` | `/categories/:id` | Auth |
| `PUT` | `/categories/:id` | Administrador |
| `DELETE` | `/categories/:id` | Administrador |
| `GET` | `/products` | Auth |
| `GET` | `/products/barcode/:barcode` | Auth |
| `POST` | `/products` | Administrador |
| `GET` | `/products/:id` | Auth |
| `PUT` | `/products/:id` | Administrador |
| `DELETE` | `/products/:id` | Administrador |
| `GET` | `/providers` | Auth |
| `POST` | `/providers` | Administrador |
| `GET` | `/providers/:id` | Auth |
| `PUT` | `/providers/:id` | Administrador |
| `DELETE` | `/providers/:id` | Administrador |

Los productos incluyen `category` y `provider` cuando existen. Los precios y el stock no pueden ser negativos, el barcode es unico y cada detalle de venta conserva precio y costo historicos.

### Ventas

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `POST` | `/sales` | Auth |
| `GET` | `/sales/stats` | Auth |
| `GET` | `/sales/history` | Auth |
| `GET` | `/sales/:id` | Auth |
| `PATCH` | `/sales/:id/anular` | Administrador |
| `PUT` | `/sales/:id/anular` | Administrador |

Body de creacion:

```json
{
  "customer_id": 1,
  "products": [{"product_id": 1, "quantity": 2}],
  "payment_method": "Efectivo",
  "discount_amount": 0
}
```

Medios validos: `Efectivo`, `Debito`, `Cuenta corriente`, `Transferencia` y `Tarjeta de credito`. La venta valida stock y credito dentro de una transaccion, descuenta inventario y actualiza caja cuando corresponde.

### Clientes y cuentas corrientes

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `GET` | `/customers` | Auth |
| `GET` | `/customers/search?term=juan` | Auth |
| `POST` | `/customers` | Auth |
| `GET` | `/customers/:id` | Auth |
| `PUT` | `/customers/:id` | Auth |
| `POST` | `/customer-accounts/payment` | Auth |
| `GET` | `/customer-accounts/balance/:id` | Auth |
| `GET` | `/customer-accounts/history/:id` | Auth |

Los cobros en efectivo requieren turno abierto y actualizan `expected_cash`. `creditMax` no puede ser negativo y el DNI es unico cuando se informa.

### Caja

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `POST` | `/cash-shifts/open` | Auth |
| `GET` | `/cash-shifts/current` | Auth |
| `POST` | `/cash-shifts/close` | Auth |
| `GET` | `/cash-shifts/history` | Administrador |

La caja registra fondo inicial, efectivo esperado, efectivo contado, diferencia, ventas, gastos y movimientos asociados.

### Gastos

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `GET` | `/expenses?from=YYYY-MM-DD&to=YYYY-MM-DD` | Auth |
| `POST` | `/expenses` | Auth |
| `GET` | `/expenses/summary?from=YYYY-MM-DD&to=YYYY-MM-DD` | Auth |
| `DELETE` | `/expenses/:id` | Administrador |

Categorias: `Estructura`, `Herramientas`, `Personal`, `Marketing`. Los gastos en efectivo requieren un turno abierto y descuentan el efectivo esperado.

### Compras y cuentas de proveedores

| Metodo | Ruta | Acceso |
| --- | --- | --- |
| `GET` | `/purchases?from=YYYY-MM-DD&to=YYYY-MM-DD` | Auth |
| `POST` | `/purchases` | Administrador |
| `GET` | `/purchases/:id` | Auth |
| `POST` | `/purchases/:id/anular` | Administrador |
| `GET` | `/provider-accounts/balance/:id` | Administrador |
| `POST` | `/provider-accounts/payment` | Administrador |

Las compras actualizan stock y costo promedio ponderado. Las compras a cuenta corriente generan deuda. Los pagos en efectivo requieren turno abierto y saldo suficiente.

### Reportes

Todas las rutas de reportes requieren administrador:

| Metodo | Ruta |
| --- | --- |
| `GET` | `/reports/sales-summary?from=...&to=...` |
| `GET` | `/reports/top-products?from=...&to=...` |
| `GET` | `/reports/sales-by-user?from=...&to=...` |
| `GET` | `/reports/financial-compliance` |
| `GET` | `/reports/financial-dashboard?from=...&to=...` |

Las fechas usan formato `YYYY-MM-DD` y un rango invertido devuelve `400`.

## Auditoria y seguridad

Las escrituras exitosas (`POST`, `PUT`, `PATCH`, `DELETE`) se registran en `audit_logs` con metodo, ruta, usuario, entidad y payload. Los campos `password` y `token` se eliminan antes de persistir el payload.

Las contrasenas se almacenan con Argon2. La identidad de las operaciones financieras sale del JWT; el cliente no debe enviar `user_id` para decidir quien realizo una operacion.

## Estructura

```text
src/
├── main.rs
├── db.rs
├── auth.rs
├── errors.rs
├── extractors.rs
├── handlers/
│   ├── auth.rs
│   ├── categories.rs
│   ├── financial.rs
│   ├── health.rs
│   ├── products.rs
│   ├── providers.rs
│   └── users.rs
├── middleware/
│   ├── auth.rs
│   └── audit.rs
├── models/
└── routes/
```

## Estado de validacion

El proyecto pasa:

```text
cargo fmt --check
cargo check
cargo test
```

La validacion realizada no reemplaza una prueba de integracion contra PostgreSQL. Para probar los endpoints, carga `kiosco_flow.sql`, configura `.env` y ejecuta `cargo run`.
