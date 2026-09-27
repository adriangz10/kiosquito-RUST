-- Migración para cumplimiento de especificación técnica (Fases 1 a 7)
-- RBAC, Kardex, Lotes/Vencimientos, Cobro Multimedio, Cierre Ciego, Sangrías, Histórico de Precios y Outbox

-- 1. Ampliación de Enum de Roles
DO $$
BEGIN
    ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Dueño';
    ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Dueno';
    ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Encargado';
    ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Cajero';
    ALTER TYPE enum_users_role ADD VALUE IF NOT EXISTS 'Repositor';
EXCEPTION
    WHEN others THEN null;
END $$;

-- 2. Seguridad: PIN de supervisor
ALTER TABLE users ADD COLUMN IF NOT EXISTS pin_supervisor VARCHAR(255) NULL;

-- 3. Catálogo: Margen mínimo y marcas
ALTER TABLE products ADD COLUMN IF NOT EXISTS min_margin NUMERIC(5,2) NOT NULL DEFAULT 0.00;
ALTER TABLE products ADD COLUMN IF NOT EXISTS brand VARCHAR(100) NULL;
ALTER TABLE products ALTER COLUMN stock TYPE NUMERIC(12, 3);

-- 4. Ventas: Tipo de comprobante y cantidades decimales para pesables (balanza)
ALTER TABLE sales ADD COLUMN IF NOT EXISTS tipo_comprobante VARCHAR(50) NOT NULL DEFAULT 'TICKET_NO_FISCAL';
ALTER TABLE sale_details ALTER COLUMN quantity TYPE NUMERIC(12, 3);

-- 5. Tabla venta_pagos (Cobro Multimedio / Combinado)
CREATE TABLE IF NOT EXISTS venta_pagos (
    id SERIAL PRIMARY KEY,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    medio_pago VARCHAR(50) NOT NULL,
    monto_imputado NUMERIC(12, 2) NOT NULL,
    monto_recibido NUMERIC(12, 2) NOT NULL,
    vuelto_entregado NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    referencia VARCHAR(100) NULL,
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 6. Caja: Campos adicionales para Cierre Ciego Multimedio
ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS tarjetas_declarado NUMERIC(12, 2) NULL;
ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS transferencias_declarado NUMERIC(12, 2) NULL;
ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS expected_cards NUMERIC(12, 2) NOT NULL DEFAULT 0.00;
ALTER TABLE cash_shifts ADD COLUMN IF NOT EXISTS expected_transfers NUMERIC(12, 2) NOT NULL DEFAULT 0.00;

-- 7. Tabla caja_movimientos (Sangrías y Entradas Extraordinarias de Caja)
CREATE TABLE IF NOT EXISTS caja_movimientos (
    id SERIAL PRIMARY KEY,
    caja_sesion_id INTEGER NOT NULL REFERENCES cash_shifts(id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users(id),
    tipo VARCHAR(20) NOT NULL, -- 'INGRESO', 'EGRESO'
    monto NUMERIC(12, 2) NOT NULL,
    motivo VARCHAR(255) NOT NULL,
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 8. Tabla movimientos_stock_kardex (Trazabilidad inmutable de inventario)
CREATE TABLE IF NOT EXISTS movimientos_stock_kardex (
    id SERIAL PRIMARY KEY,
    producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    tipo_movimiento VARCHAR(50) NOT NULL, -- 'VENTA', 'COMPRA', 'ANULACION_VENTA', 'ANULACION_COMPRA', 'AJUSTE', 'MERMA'
    cantidad NUMERIC(12, 3) NOT NULL,
    stock_anterior NUMERIC(12, 3) NOT NULL,
    stock_resultante NUMERIC(12, 3) NOT NULL,
    costo_unitario NUMERIC(12, 2) NOT NULL DEFAULT 0.00,
    referencia_id INTEGER NULL,
    motivo TEXT NULL,
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 9. Tabla lotes_vencimientos (Control de vencimientos y mermas)
CREATE TABLE IF NOT EXISTS lotes_vencimientos (
    id SERIAL PRIMARY KEY,
    producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    numero_lote VARCHAR(100) NULL,
    fecha_vencimiento DATE NOT NULL,
    cantidad NUMERIC(12, 3) NOT NULL DEFAULT 0,
    estado VARCHAR(50) NOT NULL DEFAULT 'ACTIVO',
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 10. Tabla historico_precios_costo (Auditoría de aumentos y márgenes)
CREATE TABLE IF NOT EXISTS historico_precios_costo (
    id SERIAL PRIMARY KEY,
    producto_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    precio_costo_anterior NUMERIC(12, 2) NOT NULL,
    precio_costo_nuevo NUMERIC(12, 2) NOT NULL,
    precio_venta_anterior NUMERIC(12, 2) NOT NULL,
    precio_venta_nuevo NUMERIC(12, 2) NOT NULL,
    motivo VARCHAR(255) NULL,
    user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 11. Tabla outbox_events (Local-First y sincronización híbrida)
CREATE TABLE IF NOT EXISTS outbox_events (
    id SERIAL PRIMARY KEY,
    aggregate_type VARCHAR(100) NOT NULL,
    aggregate_id VARCHAR(100) NOT NULL,
    event_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'PENDING',
    "createdAt" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    processed_at TIMESTAMP WITH TIME ZONE NULL
);
