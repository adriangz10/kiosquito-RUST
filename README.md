# Kiosquito Flow API

API REST para la gestion de un kiosco, construida con Rust, Axum y PostgreSQL. Permite administrar productos, categorias, proveedores y usuarios, ademas de registrar usuarios y autenticar peticiones mediante JWT.

## Tecnologias

- Rust 2021
- Axum 0.8
- Tokio
- SQLx 0.9 con PostgreSQL
- Serde y serde_json
- Argon2 para hash de contrasenas
- JSON Web Tokens mediante `jsonwebtoken`
- Chrono para fechas y expiracion de tokens
- `rust_decimal` para valores monetarios
- `tracing` para logs

## Requisitos

- Rust y Cargo instalados.
- PostgreSQL en ejecucion.
- Una base de datos con las tablas esperadas por las consultas del proyecto.
- Variables de entorno configuradas.

El proyecto no incluye actualmente migraciones SQL, datos iniciales, documentacion OpenAPI ni scripts de despliegue.

## Configuracion

Crea un archivo `.env` en la raiz del proyecto:

```env
DATABASE_URL=postgres://usuario:contrasena@localhost:5432/kiosquito
JWT_SECRET=una-clave-larga-y-segura
PORT=3000
```

Variables disponibles:

| Variable | Obligatoria | Descripcion |
| --- | --- | --- |
| `DATABASE_URL` | Si | URL de conexion a PostgreSQL. |
| `JWT_SECRET` | Para autenticacion | Clave usada para firmar y validar JWT. |
| `PORT` | No | Puerto HTTP. Por defecto es `3000`. |

El servidor escucha en `0.0.0.0` y usa el puerto definido por `PORT`.

## Instalacion y ejecucion

Desde la carpeta del proyecto:

```powershell
cargo build
cargo run
```

El servidor estara disponible en:

```text
http://localhost:3000
```

Comandos utiles:

```powershell
cargo check
cargo test
cargo fmt
cargo fmt --check
```

Si falta `DATABASE_URL` o PostgreSQL no esta disponible, la aplicacion termina durante el arranque.

## Endpoints

### Salud

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `GET` | `/health` | No | Devuelve el estado configurado del servicio. |

Respuesta:

```json
{
  "status": "Base de datos conectada"
}
```

> Nota: el handler actual no ejecuta una consulta contra PostgreSQL; por tanto, esta respuesta no confirma por si sola que la base de datos siga disponible.

### Autenticacion

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `POST` | `/auth/register` | No | Registra un usuario. |
| `POST` | `/auth/login` | No | Valida credenciales y devuelve un JWT. |

Registro:

```json
{
  "username": "empleado1",
  "password": "secreto",
  "role": "Empleado"
}
```

Inicio de sesion:

```json
{
  "username": "empleado1",
  "password": "secreto"
}
```

Respuesta:

```json
{
  "token": "<jwt>"
}
```

Los roles disponibles son `Administrador` y `Empleado`.

### Productos

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `GET` | `/products` | No | Lista todos los productos. |
| `POST` | `/products` | No | Crea un producto. |
| `GET` | `/products/{id}` | No | Obtiene un producto. |
| `PUT` | `/products/{id}` | No | Actualiza un producto. |
| `DELETE` | `/products/{id}` | No | Elimina un producto. |

Campos de producto:

```json
{
  "name": "Gaseosa",
  "price": 1500.00,
  "stock": 10,
  "category_id": 1,
  "provider_id": 2,
  "barcode": "7790000000000",
  "cost_price": 900.00
}
```

### Categorias

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `GET` | `/categories` | No | Lista categorias. |
| `POST` | `/categories` | No | Crea una categoria. |
| `GET` | `/categories/{id}` | No | Obtiene una categoria. |
| `PUT` | `/categories/{id}` | No | Actualiza una categoria. |
| `DELETE` | `/categories/{id}` | No | Elimina una categoria. |

Payload de creacion o actualizacion:

```json
{
  "name": "Bebidas"
}
```

### Proveedores

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `GET` | `/providers` | No | Lista proveedores. |
| `POST` | `/providers` | No | Crea un proveedor. |
| `GET` | `/providers/{id}` | No | Obtiene un proveedor. |
| `PUT` | `/providers/{id}` | No | Actualiza un proveedor. |
| `DELETE` | `/providers/{id}` | No | Elimina un proveedor. |

Payload de creacion:

```json
{
  "name": "Distribuidora Central",
  "phone": "555-0100",
  "direction": "Av. Principal 123"
}
```

### Usuarios

Estos endpoints requieren un JWT valido con rol `Administrador`:

| Metodo | Ruta | Autenticacion | Descripcion |
| --- | --- | --- | --- |
| `GET` | `/users` | Bearer JWT + Administrador | Lista usuarios. |
| `DELETE` | `/users/{id}` | Bearer JWT + Administrador | Elimina un usuario. |

Header requerido:

```http
Authorization: Bearer <jwt>
```

Las sesiones JWT expiran despues de 24 horas.

## Base de datos

La aplicacion usa un pool PostgreSQL de hasta 5 conexiones. Las consultas actuales esperan, como minimo, las siguientes tablas:

- `users`
- `categories`
- `products`
- `providers`

Las consultas tambien esperan:

- Un tipo PostgreSQL llamado `enum_users_role`.
- Columnas camelCase con comillas: `"createdAt"` y `"updatedAt"`.
- Relaciones entre productos, categorias y proveedores mediante `category_id` y `provider_id`.

El esquema exacto, las restricciones, los indices y las migraciones no estan incluidos en este repositorio.

## Estructura del proyecto

```text
src/
├── main.rs                 # Arranque del servidor y composicion de rutas
├── db.rs                   # Conexion y pool PostgreSQL
├── auth.rs                 # Hash de contrasenas y JWT
├── errors.rs               # Errores HTTP de la aplicacion
├── extractors.rs           # Extractores AuthUser y AdminUser
├── middleware/
│   ├── mod.rs
│   └── auth.rs              # Validacion del header Bearer
├── handlers/
│   ├── auth.rs
│   ├── categories.rs
│   ├── health.rs
│   ├── products.rs
│   ├── providers.rs
│   └── users.rs
├── models/
│   ├── category.rs
│   ├── product.rs
│   ├── provider.rs
│   └── user.rs
└── routes/
    └── mod.rs              # Definicion de rutas HTTP
```

## Seguridad y limitaciones actuales

- El registro permite que el cliente envie el rol, incluido `Administrador`. En produccion deberia restringirse la creacion de administradores.
- Productos, categorias y proveedores estan expuestos sin autenticacion, incluyendo operaciones de escritura y eliminacion.
- El middleware JWT protege los endpoints de usuarios.
- El secreto JWT debe ser largo, privado y diferente entre entornos.
- No hay migraciones ni pruebas de integracion incluidas.
- Los errores de base de datos se registran en logs y se devuelven como error interno al cliente.

## Estado del proyecto

Comprobaciones locales previstas:

```powershell
cargo check
cargo test
cargo fmt --check
```

La aplicacion requiere una instancia PostgreSQL configurada para probar los endpoints que acceden a la base de datos.
