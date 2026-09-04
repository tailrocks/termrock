// SPDX-FileCopyrightText: 2026 Alexey Zhokhov
// SPDX-License-Identifier: Apache-2.0
//
// Adapted from junie-tui src/bin/tablepro/db.rs (MIT),
// https://github.com/donbeave/terminal-components-claude

#![allow(elided_lifetimes_in_paths)]
#![allow(missing_docs)]

//! Deterministic demo database: catalog (connections, databases, schemas,
//! tables, columns, indexes, keys) and generated rows. No drivers: a small
//! in-memory engine evaluates the SQL subset the workbench needs, so every
//! interaction is real while results stay reproducible.

// ------------------------------------------------------------------ catalog

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Postgres,
    MySql,
    Sqlite,
}

impl Engine {
    pub fn label(self) -> &'static str {
        match self {
            Engine::Postgres => "PostgreSQL",
            Engine::MySql => "MySQL",
            Engine::Sqlite => "SQLite",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Engine::Postgres => "pg",
            Engine::MySql => "mysql",
            Engine::Sqlite => "sqlite",
        }
    }
}

/// TablePro's per-connection Safe Mode level (`SafeModeLevel.swift`).
/// Destructive statements always confirm regardless of level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SafeMode {
    /// Default: writes and reads run; only dangerous statements ask.
    Silent,
    /// Writes ask for confirmation.
    Alert,
    /// Every statement asks for confirmation.
    AlertFull,
    /// Writes ask and require authentication (Touch ID on macOS).
    Safe,
    /// Every statement asks and requires authentication.
    SafeFull,
    /// Writes are refused.
    ReadOnly,
}

impl SafeMode {
    pub const ALL: [SafeMode; 6] = [
        SafeMode::Silent,
        SafeMode::Alert,
        SafeMode::AlertFull,
        SafeMode::Safe,
        SafeMode::SafeFull,
        SafeMode::ReadOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SafeMode::Silent => "Silent",
            SafeMode::Alert => "Alert",
            SafeMode::AlertFull => "Alert (Full)",
            SafeMode::Safe => "Safe Mode",
            SafeMode::SafeFull => "Safe Mode (Full)",
            SafeMode::ReadOnly => "Read-Only",
        }
    }

    /// Short token for the identity strip.
    pub fn token(self) -> &'static str {
        match self {
            SafeMode::Silent => "silent",
            SafeMode::Alert => "alert",
            SafeMode::AlertFull => "alert+",
            SafeMode::Safe => "safe",
            SafeMode::SafeFull => "safe+",
            SafeMode::ReadOnly => "read-only",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            SafeMode::Silent => "Writes run without asking. Destructive statements still confirm.",
            SafeMode::Alert => "Every write asks for confirmation before it runs.",
            SafeMode::AlertFull => "Every statement, reads included, asks for confirmation.",
            SafeMode::Safe => "Writes ask for confirmation and a deliberate acknowledgement.",
            SafeMode::SafeFull => {
                "Every statement asks for confirmation and a deliberate acknowledgement."
            }
            SafeMode::ReadOnly => "Writes are refused. Reads and exports still work.",
        }
    }

    pub fn requires_confirmation(self) -> bool {
        !matches!(self, SafeMode::Silent | SafeMode::ReadOnly)
    }
    pub fn requires_authentication(self) -> bool {
        matches!(self, SafeMode::Safe | SafeMode::SafeFull)
    }
    pub fn applies_to_all_queries(self) -> bool {
        matches!(self, SafeMode::AlertFull | SafeMode::SafeFull)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Local,
    Development,
    Staging,
    Production,
}

impl Environment {
    pub fn label(self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Development => "development",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    pub name: String,
    pub engine: Engine,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub environment: Environment,
    pub safe_mode: SafeMode,
    pub ssl: bool,
    pub ssh: Option<String>,
    pub group: String,
    pub last_used: String,
    /// Simulated behaviour when connecting.
    pub outcome: ConnectOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectOutcome {
    Ok,
    AuthFailed,
    Unreachable,
}

pub fn connections() -> Vec<Connection> {
    vec![
        Connection {
            name: "Local PostgreSQL".into(),
            engine: Engine::Postgres,
            host: "localhost".into(),
            port: 5432,
            database: "acme_dev".into(),
            user: "postgres".into(),
            environment: Environment::Local,
            safe_mode: SafeMode::Silent,
            ssl: false,
            ssh: None,
            group: "Personal".into(),
            last_used: "2 minutes ago".into(),
            outcome: ConnectOutcome::Ok,
        },
        Connection {
            name: "Development".into(),
            engine: Engine::Postgres,
            host: "dev-db.internal.acme.io".into(),
            port: 5432,
            database: "acme_dev".into(),
            user: "acme_app".into(),
            environment: Environment::Development,
            safe_mode: SafeMode::Silent,
            ssl: true,
            ssh: None,
            group: "Acme".into(),
            last_used: "yesterday".into(),
            outcome: ConnectOutcome::Ok,
        },
        Connection {
            name: "Staging".into(),
            engine: Engine::Postgres,
            host: "staging-db.acme.io".into(),
            port: 5432,
            database: "acme_staging".into(),
            user: "acme_app".into(),
            environment: Environment::Staging,
            safe_mode: SafeMode::Alert,
            ssl: true,
            ssh: Some("bastion.acme.io".into()),
            group: "Acme".into(),
            last_used: "3 days ago".into(),
            outcome: ConnectOutcome::AuthFailed,
        },
        Connection {
            name: "Analytics".into(),
            engine: Engine::MySql,
            host: "analytics.acme.io".into(),
            port: 3306,
            database: "warehouse".into(),
            user: "analyst".into(),
            environment: Environment::Production,
            safe_mode: SafeMode::ReadOnly,
            ssl: true,
            ssh: None,
            group: "Acme".into(),
            last_used: "last week".into(),
            outcome: ConnectOutcome::Unreachable,
        },
        Connection {
            name: "Production".into(),
            engine: Engine::Postgres,
            host: "prod-db-1.acme.io".into(),
            port: 5432,
            database: "acme_prod".into(),
            user: "acme_ops".into(),
            environment: Environment::Production,
            safe_mode: SafeMode::Safe,
            ssl: true,
            ssh: Some("bastion.acme.io".into()),
            group: "Acme".into(),
            last_used: "1 hour ago".into(),
            outcome: ConnectOutcome::Ok,
        },
        Connection {
            name: "Scratch".into(),
            engine: Engine::Sqlite,
            host: "~/scratch.db".into(),
            port: 0,
            database: "scratch".into(),
            user: String::new(),
            environment: Environment::Local,
            safe_mode: SafeMode::Silent,
            ssl: false,
            ssh: None,
            group: "Personal".into(),
            last_used: "never".into(),
            outcome: ConnectOutcome::Ok,
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColType {
    Uuid,
    Text,
    Int,
    Numeric,
    Bool,
    Timestamp,
    Date,
    Json,
    Enum,
}

impl ColType {
    pub fn sql(self) -> &'static str {
        match self {
            ColType::Uuid => "uuid",
            ColType::Text => "text",
            ColType::Int => "integer",
            ColType::Numeric => "numeric(12,2)",
            ColType::Bool => "boolean",
            ColType::Timestamp => "timestamptz",
            ColType::Date => "date",
            ColType::Json => "jsonb",
            ColType::Enum => "text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub ty: ColType,
    pub nullable: bool,
    pub default: Option<String>,
    pub primary: bool,
    pub references: Option<(String, String)>,
    pub enum_values: Vec<&'static str>,
    pub generated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub columns: Vec<String>,
    pub unique: bool,
    pub method: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    pub kind: &'static str,
    pub definition: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub schema: String,
    pub name: String,
    pub kind: ObjectKind,
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
    pub constraints: Vec<Constraint>,
    pub triggers: Vec<String>,
    pub row_count: usize,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectKind {
    Table,
    View,
    Function,
    Sequence,
}

impl Table {
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
    pub fn column(&self, name: &str) -> Option<&Column> {
        self.columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
    pub fn primary_key(&self) -> Vec<&Column> {
        self.columns.iter().filter(|c| c.primary).collect()
    }
}

fn col(name: &str, ty: ColType) -> Column {
    Column {
        name: name.into(),
        ty,
        nullable: false,
        default: None,
        primary: false,
        references: None,
        enum_values: vec![],
        generated: false,
    }
}
fn pk(name: &str) -> Column {
    Column {
        primary: true,
        default: Some("gen_random_uuid()".into()),
        ..col(name, ColType::Uuid)
    }
}
fn fk(name: &str, table: &str) -> Column {
    Column {
        references: Some((table.into(), "id".into())),
        ..col(name, ColType::Uuid)
    }
}
fn nullable(c: Column) -> Column {
    Column {
        nullable: true,
        ..c
    }
}
fn dflt(c: Column, d: &str) -> Column {
    Column {
        default: Some(d.into()),
        ..c
    }
}
fn enum_col(name: &str, values: Vec<&'static str>) -> Column {
    Column {
        enum_values: values,
        ..col(name, ColType::Enum)
    }
}
fn idx(name: &str, cols: &[&str], unique: bool) -> Index {
    Index {
        name: name.into(),
        columns: cols.iter().map(|c| (*c).into()).collect(),
        unique,
        method: "btree",
    }
}

#[derive(Debug, Clone)]
pub struct Catalog {
    pub database: String,
    pub schemas: Vec<String>,
    pub tables: Vec<Table>,
}

impl Catalog {
    pub fn acme_prod() -> Self {
        let created = || dflt(col("created_at", ColType::Timestamp), "now()");
        let updated = || nullable(col("updated_at", ColType::Timestamp));
        let tables = vec![
            Table {
                schema: "public".into(),
                name: "organizations".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    col("name", ColType::Text),
                    col("slug", ColType::Text),
                    enum_col("plan", vec!["free", "starter", "team", "enterprise"]),
                    nullable(col("billing_email", ColType::Text)),
                    dflt(col("seats", ColType::Int), "5"),
                    nullable(col("settings", ColType::Json)),
                    created(),
                    updated(),
                ],
                indexes: vec![
                    idx("organizations_pkey", &["id"], true),
                    idx("organizations_slug_key", &["slug"], true),
                ],
                constraints: vec![
                    Constraint { name: "organizations_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "organizations_slug_key".into(), kind: "UNIQUE", definition: "(slug)".into() },
                    Constraint { name: "organizations_seats_check".into(), kind: "CHECK", definition: "(seats > 0)".into() },
                ],
                triggers: vec!["set_updated_at BEFORE UPDATE".into()],
                row_count: 1_240,
                comment: Some("Tenant accounts".into()),
            },
            Table {
                schema: "public".into(),
                name: "customers".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    fk("organization_id", "organizations"),
                    col("email", ColType::Text),
                    col("full_name", ColType::Text),
                    nullable(col("phone", ColType::Text)),
                    dflt(col("is_active", ColType::Bool), "true"),
                    dflt(col("marketing_opt_in", ColType::Bool), "false"),
                    nullable(col("last_login_at", ColType::Timestamp)),
                    nullable(col("metadata", ColType::Json)),
                    created(),
                    updated(),
                ],
                indexes: vec![
                    idx("customers_pkey", &["id"], true),
                    idx("customers_email_key", &["email"], true),
                    idx("customers_org_idx", &["organization_id"], false),
                ],
                constraints: vec![
                    Constraint { name: "customers_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "customers_email_key".into(), kind: "UNIQUE", definition: "(email)".into() },
                    Constraint { name: "customers_organization_id_fkey".into(), kind: "FOREIGN KEY", definition: "(organization_id) REFERENCES organizations(id) ON DELETE CASCADE".into() },
                ],
                triggers: vec!["set_updated_at BEFORE UPDATE".into(), "audit_customers AFTER INSERT OR UPDATE OR DELETE".into()],
                row_count: 48_912,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "products".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    col("sku", ColType::Text),
                    col("name", ColType::Text),
                    nullable(col("description", ColType::Text)),
                    col("unit_price", ColType::Numeric),
                    dflt(col("currency", ColType::Text), "'USD'"),
                    dflt(col("is_active", ColType::Bool), "true"),
                    nullable(col("attributes", ColType::Json)),
                    created(),
                ],
                indexes: vec![idx("products_pkey", &["id"], true), idx("products_sku_key", &["sku"], true)],
                constraints: vec![
                    Constraint { name: "products_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "products_sku_key".into(), kind: "UNIQUE", definition: "(sku)".into() },
                    Constraint { name: "products_unit_price_check".into(), kind: "CHECK", definition: "(unit_price >= 0)".into() },
                ],
                triggers: vec![],
                row_count: 312,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "orders".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    Column { generated: true, default: Some("nextval('orders_number_seq')".into()), ..col("order_number", ColType::Int) },
                    fk("customer_id", "customers"),
                    fk("organization_id", "organizations"),
                    dflt(enum_col("status", vec!["pending", "paid", "shipped", "delivered", "cancelled", "refunded"]), "'pending'"),
                    col("total_amount", ColType::Numeric),
                    dflt(col("currency", ColType::Text), "'USD'"),
                    nullable(col("shipping_address", ColType::Json)),
                    nullable(col("notes", ColType::Text)),
                    nullable(col("shipped_at", ColType::Timestamp)),
                    nullable(col("delivered_at", ColType::Date)),
                    dflt(col("is_gift", ColType::Bool), "false"),
                    created(),
                    updated(),
                ],
                indexes: vec![
                    idx("orders_pkey", &["id"], true),
                    idx("orders_order_number_key", &["order_number"], true),
                    idx("orders_customer_idx", &["customer_id"], false),
                    idx("orders_status_created_idx", &["status", "created_at"], false),
                ],
                constraints: vec![
                    Constraint { name: "orders_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "orders_customer_id_fkey".into(), kind: "FOREIGN KEY", definition: "(customer_id) REFERENCES customers(id)".into() },
                    Constraint { name: "orders_organization_id_fkey".into(), kind: "FOREIGN KEY", definition: "(organization_id) REFERENCES organizations(id)".into() },
                    Constraint { name: "orders_status_check".into(), kind: "CHECK", definition: "(status IN ('pending','paid','shipped','delivered','cancelled','refunded'))".into() },
                    Constraint { name: "orders_total_amount_check".into(), kind: "CHECK", definition: "(total_amount >= 0)".into() },
                ],
                triggers: vec!["set_updated_at BEFORE UPDATE".into(), "orders_audit AFTER UPDATE".into()],
                row_count: 1_203_338,
                comment: Some("One row per checkout".into()),
            },
            Table {
                schema: "public".into(),
                name: "order_items".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    fk("order_id", "orders"),
                    fk("product_id", "products"),
                    col("quantity", ColType::Int),
                    col("unit_price", ColType::Numeric),
                    nullable(col("discount", ColType::Numeric)),
                    created(),
                ],
                indexes: vec![idx("order_items_pkey", &["id"], true), idx("order_items_order_idx", &["order_id"], false)],
                constraints: vec![
                    Constraint { name: "order_items_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "order_items_order_id_fkey".into(), kind: "FOREIGN KEY", definition: "(order_id) REFERENCES orders(id) ON DELETE CASCADE".into() },
                    Constraint { name: "order_items_product_id_fkey".into(), kind: "FOREIGN KEY", definition: "(product_id) REFERENCES products(id)".into() },
                    Constraint { name: "order_items_quantity_check".into(), kind: "CHECK", definition: "(quantity > 0)".into() },
                ],
                triggers: vec![],
                row_count: 3_811_020,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "payments".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    fk("order_id", "orders"),
                    enum_col("provider", vec!["stripe", "paypal", "wire", "apple_pay"]),
                    nullable(col("provider_ref", ColType::Text)),
                    col("amount", ColType::Numeric),
                    dflt(col("currency", ColType::Text), "'USD'"),
                    enum_col("status", vec!["authorized", "captured", "failed", "refunded"]),
                    nullable(col("failure_reason", ColType::Text)),
                    nullable(col("captured_at", ColType::Timestamp)),
                    created(),
                ],
                indexes: vec![idx("payments_pkey", &["id"], true), idx("payments_order_idx", &["order_id"], false), idx("payments_provider_ref_idx", &["provider", "provider_ref"], false)],
                constraints: vec![
                    Constraint { name: "payments_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "payments_order_id_fkey".into(), kind: "FOREIGN KEY", definition: "(order_id) REFERENCES orders(id)".into() },
                ],
                triggers: vec![],
                row_count: 1_180_442,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "subscriptions".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    fk("organization_id", "organizations"),
                    enum_col("plan", vec!["starter", "team", "enterprise"]),
                    enum_col("status", vec!["trialing", "active", "past_due", "cancelled"]),
                    col("seats", ColType::Int),
                    col("mrr", ColType::Numeric),
                    col("current_period_end", ColType::Date),
                    nullable(col("cancelled_at", ColType::Timestamp)),
                    created(),
                    updated(),
                ],
                indexes: vec![idx("subscriptions_pkey", &["id"], true), idx("subscriptions_org_idx", &["organization_id"], false)],
                constraints: vec![
                    Constraint { name: "subscriptions_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() },
                    Constraint { name: "subscriptions_organization_id_fkey".into(), kind: "FOREIGN KEY", definition: "(organization_id) REFERENCES organizations(id)".into() },
                ],
                triggers: vec!["set_updated_at BEFORE UPDATE".into()],
                row_count: 1_102,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "active_customers".into(),
                kind: ObjectKind::View,
                columns: vec![col("id", ColType::Uuid), col("email", ColType::Text), col("full_name", ColType::Text), col("organization", ColType::Text)],
                indexes: vec![],
                constraints: vec![],
                triggers: vec![],
                row_count: 41_207,
                comment: Some("customers WHERE is_active".into()),
            },
            Table {
                schema: "public".into(),
                name: "order_totals_by_day".into(),
                kind: ObjectKind::View,
                columns: vec![col("day", ColType::Date), col("orders", ColType::Int), col("revenue", ColType::Numeric)],
                indexes: vec![],
                constraints: vec![],
                triggers: vec![],
                row_count: 1_460,
                comment: None,
            },
            Table {
                schema: "analytics".into(),
                name: "events".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    nullable(fk("customer_id", "customers")),
                    enum_col("event_type", vec!["page_view", "add_to_cart", "checkout", "signup", "login", "search"]),
                    nullable(col("session_id", ColType::Uuid)),
                    nullable(col("properties", ColType::Json)),
                    nullable(col("user_agent", ColType::Text)),
                    col("occurred_at", ColType::Timestamp),
                ],
                indexes: vec![idx("events_pkey", &["id"], true), idx("events_occurred_idx", &["occurred_at"], false), idx("events_type_idx", &["event_type"], false)],
                constraints: vec![Constraint { name: "events_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() }],
                triggers: vec![],
                row_count: 98_442_010,
                comment: Some("Product analytics, partitioned by month".into()),
            },
            Table {
                schema: "analytics".into(),
                name: "daily_revenue".into(),
                kind: ObjectKind::Table,
                columns: vec![col("day", ColType::Date), col("orders", ColType::Int), col("revenue", ColType::Numeric), col("refunds", ColType::Numeric), col("net", ColType::Numeric)],
                indexes: vec![idx("daily_revenue_day_key", &["day"], true)],
                constraints: vec![],
                triggers: vec![],
                row_count: 1_460,
                comment: Some("Materialized nightly".into()),
            },
            Table {
                schema: "analytics".into(),
                name: "cohort_retention".into(),
                kind: ObjectKind::View,
                columns: vec![col("cohort_month", ColType::Date), col("month_offset", ColType::Int), col("customers", ColType::Int), col("retained_pct", ColType::Numeric)],
                indexes: vec![],
                constraints: vec![],
                triggers: vec![],
                row_count: 288,
                comment: None,
            },
            Table {
                schema: "audit".into(),
                name: "change_log".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    Column { generated: true, default: Some("identity".into()), ..pk("id") },
                    col("table_name", ColType::Text),
                    col("row_id", ColType::Uuid),
                    enum_col("operation", vec!["INSERT", "UPDATE", "DELETE"]),
                    nullable(col("changed_by", ColType::Text)),
                    nullable(col("old_values", ColType::Json)),
                    nullable(col("new_values", ColType::Json)),
                    col("changed_at", ColType::Timestamp),
                ],
                indexes: vec![idx("change_log_pkey", &["id"], true), idx("change_log_table_row_idx", &["table_name", "row_id"], false)],
                constraints: vec![Constraint { name: "change_log_pkey".into(), kind: "PRIMARY KEY", definition: "(id)".into() }],
                triggers: vec![],
                row_count: 12_004_118,
                comment: None,
            },
            Table {
                schema: "audit".into(),
                name: "login_attempts".into(),
                kind: ObjectKind::Table,
                columns: vec![
                    pk("id"),
                    col("email", ColType::Text),
                    col("succeeded", ColType::Bool),
                    nullable(col("ip", ColType::Text)),
                    col("attempted_at", ColType::Timestamp),
                ],
                indexes: vec![idx("login_attempts_pkey", &["id"], true), idx("login_attempts_email_idx", &["email"], false)],
                constraints: vec![],
                triggers: vec![],
                row_count: 3_301_002,
                comment: None,
            },
            Table {
                schema: "public".into(),
                name: "set_updated_at()".into(),
                kind: ObjectKind::Function,
                columns: vec![],
                indexes: vec![],
                constraints: vec![],
                triggers: vec![],
                row_count: 0,
                comment: Some("trigger function, plpgsql".into()),
            },
            Table {
                schema: "public".into(),
                name: "orders_number_seq".into(),
                kind: ObjectKind::Sequence,
                columns: vec![],
                indexes: vec![],
                constraints: vec![],
                triggers: vec![],
                row_count: 0,
                comment: None,
            },
        ];
        Self {
            database: "acme_prod".into(),
            schemas: vec!["public".into(), "analytics".into(), "audit".into()],
            tables,
        }
    }

    pub fn find(&self, schema: Option<&str>, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| {
            t.name.eq_ignore_ascii_case(name)
                && schema.is_none_or(|s| t.schema.eq_ignore_ascii_case(s))
        })
    }

    pub fn tables_in(&self, schema: &str, kind: ObjectKind) -> impl Iterator<Item = &Table> {
        self.tables
            .iter()
            .filter(move |t| t.schema == schema && t.kind == kind)
    }
}

// ---------------------------------------------------------------- values

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Text(String),
    Int(i64),
    Num(f64),
    Bool(bool),
    Json(String),
}

impl Value {
    pub fn display(&self) -> String {
        match self {
            Value::Null => "NULL".into(),
            Value::Text(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Num(n) => format!("{n:.2}"),
            Value::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            Value::Json(j) => j.clone(),
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }
}

/// Deterministic pseudo-random stream (SplitMix64).
#[derive(Clone)]
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

const FIRST: &[&str] = &[
    "Mira", "Jonas", "Ana", "Kai", "Sofia", "Lucas", "Elena", "Omar", "Priya", "Tomas", "Ingrid",
    "Diego", "Hana", "Felix", "Nadia", "Ravi", "Greta", "Mateo", "Yuki", "Leon",
];
const LAST: &[&str] = &[
    "Okafor",
    "Weber",
    "Costa",
    "Tanaka",
    "Rossi",
    "Nguyen",
    "Petrov",
    "Haddad",
    "Iyer",
    "Novak",
    "Lindqvist",
    "Alvarez",
    "Sato",
    "Brandt",
    "Karim",
    "Mehta",
    "Berg",
    "Silva",
    "Mori",
    "Fischer",
];
const ORGS: &[&str] = &[
    "Northwind Labs",
    "Lumen Retail",
    "Halden & Co",
    "Bluefin Logistics",
    "Orbit Studio",
    "Kestrel Health",
    "Fjord Analytics",
    "Tessellate",
    "Juniper Home",
    "Meridian Foods",
];
const DOMAINS: &[&str] = &[
    "northwind.io",
    "lumen.shop",
    "halden.co",
    "bluefin.dev",
    "orbit.studio",
    "kestrel.health",
    "fjord.ai",
    "tessellate.app",
    "juniper.home",
    "meridian.foods",
];
const PRODUCTS: &[&str] = &[
    "Standing desk 140",
    "Ergo chair",
    "Monitor arm",
    "USB-C dock",
    "Mechanical keyboard",
    "Desk lamp",
    "Cable tray",
    "Laptop stand",
    "Webcam 4K",
    "Headset Pro",
    "Whiteboard 90",
    "Footrest",
];
const STREETS: &[&str] = &[
    "Hauptstrasse",
    "Rue de Rivoli",
    "Market St",
    "Via Roma",
    "Calle Mayor",
    "Kungsgatan",
    "Nevsky Ave",
    "Queen St",
];
const CITIES: &[&str] = &[
    "Berlin",
    "Paris",
    "San Francisco",
    "Milan",
    "Madrid",
    "Stockholm",
    "Lisbon",
    "Toronto",
];
const NOTES: &[&str] = &[
    "Leave at reception; building closes 18:00.",
    "Customer asked for an invoice with VAT number DE812345678, please attach it to the shipment confirmation email.",
    "Gift wrap, no price on the slip.",
    "Second attempt after failed card. Confirmed by phone with the account owner, who asked us to hold the parcel at the depot until Monday.",
    "Replace damaged unit from order #10442.",
];

fn uuid(r: &mut Rng) -> String {
    let a = r.next();
    let b = r.next();
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a & 0xffff_ffff,
        (a >> 32) & 0xffff,
        (a >> 48) & 0xfff,
        0x8000 | ((b >> 48) & 0x3fff),
        b & 0xffff_ffff_ffff
    )
}

fn timestamp(r: &mut Rng, year: i32) -> String {
    let month = 1 + r.below(12);
    let day = 1 + r.below(28);
    format!(
        "{year}-{month:02}-{day:02} {:02}:{:02}:{:02}+00",
        r.below(24),
        r.below(60),
        r.below(60)
    )
}

fn date(r: &mut Rng, year: i32) -> String {
    format!("{year}-{:02}-{:02}", 1 + r.below(12), 1 + r.below(28))
}

/// Generate deterministic rows for a table. `n` rows starting at `offset`.
pub fn rows(table: &Table, offset: usize, n: usize) -> Vec<Vec<Value>> {
    let mut out = Vec::with_capacity(n);
    for i in offset..offset + n {
        let mut r =
            Rng((i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (table.name.len() as u64) << 40);
        let mut row = Vec::with_capacity(table.columns.len());
        let first = r.pick(FIRST);
        let last = r.pick(LAST);
        let org_i = r.below(ORGS.len() as u64) as usize;
        for c in &table.columns {
            let v = match (c.name.as_str(), c.ty) {
                (_, ColType::Uuid) if c.primary => Value::Text(uuid(&mut r)),
                (_, ColType::Uuid) => {
                    if c.nullable && r.chance(20) {
                        Value::Null
                    } else {
                        Value::Text(uuid(&mut r))
                    }
                }
                ("order_number", _) => Value::Int(10_000 + i as i64),
                ("id", ColType::Int) => Value::Int(i as i64 + 1),
                ("email" | "billing_email", _) => Value::Text(format!(
                    "{}.{}@{}",
                    first.to_lowercase(),
                    last.to_lowercase(),
                    DOMAINS[org_i]
                )),
                ("full_name", _) => Value::Text(format!("{first} {last}")),
                ("name", _) if table.name == "organizations" => Value::Text(ORGS[org_i].into()),
                ("name", _) if table.name == "products" => Value::Text(r.pick(PRODUCTS).into()),
                ("slug", _) => Value::Text(
                    ORGS[org_i]
                        .to_lowercase()
                        .replace([' ', '&'], "-")
                        .replace("--", "-"),
                ),
                ("sku", _) => Value::Text(format!("SKU-{:05}", 1000 + i)),
                ("phone", _) => {
                    if r.chance(35) {
                        Value::Null
                    } else {
                        Value::Text(format!(
                            "+{} {} {:03} {:04}",
                            1 + r.below(48),
                            100 + r.below(900),
                            r.below(1000),
                            r.below(10000)
                        ))
                    }
                }
                ("table_name", _) => Value::Text(
                    r.pick(&["orders", "customers", "payments", "subscriptions"])
                        .into(),
                ),
                ("changed_by", _) => Value::Text(format!(
                    "{}@acme.io",
                    r.pick(&["mira", "jonas", "ana", "system", "deploy-bot"])
                )),
                ("ip", _) => Value::Text(format!(
                    "{}.{}.{}.{}",
                    10 + r.below(200),
                    r.below(255),
                    r.below(255),
                    1 + r.below(254)
                )),
                ("user_agent", _) => {
                    if r.chance(15) {
                        Value::Null
                    } else {
                        Value::Text(r.pick(&["Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/605.1.15 Safari/605.1.15", "Mozilla/5.0 (iPhone; CPU iPhone OS 17_4 like Mac OS X) Mobile/15E148", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/124.0"]).into())
                    }
                }
                ("provider_ref", _) => Value::Text(format!("ch_{:016x}", r.next())),
                ("failure_reason", _) => {
                    if r.chance(80) {
                        Value::Null
                    } else {
                        Value::Text(
                            r.pick(&[
                                "insufficient_funds",
                                "card_declined",
                                "expired_card",
                                "3ds_failed",
                            ])
                            .into(),
                        )
                    }
                }
                ("description", _) | ("notes", _) => {
                    if r.chance(45) {
                        Value::Null
                    } else {
                        Value::Text(r.pick(NOTES).into())
                    }
                }
                ("currency", _) => Value::Text(r.pick(&["USD", "USD", "EUR", "GBP"]).into()),
                (_, ColType::Enum) => Value::Text(r.pick(&c.enum_values).into()),
                ("seats", _) => Value::Int(1 + r.below(120) as i64),
                ("quantity", _) => Value::Int(1 + r.below(6) as i64),
                ("orders", _) => Value::Int(200 + r.below(900) as i64),
                ("customers", _) | ("month_offset", _) => Value::Int(r.below(1000) as i64),
                (_, ColType::Int) => Value::Int(r.below(10_000) as i64),
                ("retained_pct", _) => Value::Num(20.0 + r.below(7500) as f64 / 100.0),
                (_, ColType::Numeric) => {
                    if c.nullable && r.chance(60) {
                        Value::Null
                    } else {
                        Value::Num((500 + r.below(250_000)) as f64 / 100.0)
                    }
                }
                ("succeeded", _) => Value::Bool(r.chance(88)),
                ("is_active", _) => Value::Bool(r.chance(85)),
                ("marketing_opt_in", _) | ("is_gift", _) => Value::Bool(r.chance(20)),
                (_, ColType::Bool) => Value::Bool(r.chance(50)),
                ("shipping_address", _) => {
                    if r.chance(10) {
                        Value::Null
                    } else {
                        Value::Json(format!(
                            "{{\"line1\": \"{} {}\", \"city\": \"{}\", \"postal_code\": \"{:05}\", \"country\": \"{}\"}}",
                            1 + r.below(200),
                            r.pick(STREETS),
                            r.pick(CITIES),
                            r.below(99999),
                            r.pick(&["DE", "FR", "US", "IT", "ES", "SE"])
                        ))
                    }
                }
                ("settings", _) => Value::Json(format!(
                    "{{\"sso\": {}, \"retention_days\": {}, \"features\": [\"exports\"{}]}}",
                    r.chance(30),
                    30 * (1 + r.below(12)),
                    if r.chance(50) { ", \"audit\"" } else { "" }
                )),
                ("metadata", _) => {
                    if r.chance(50) {
                        Value::Null
                    } else {
                        Value::Json(format!(
                            "{{\"source\": \"{}\", \"utm_campaign\": \"{}\"}}",
                            r.pick(&["web", "ios", "android", "import"]),
                            r.pick(&["spring-24", "launch", "referral", "none"])
                        ))
                    }
                }
                ("attributes", _) => Value::Json(format!(
                    "{{\"color\": \"{}\", \"weight_kg\": {:.1}}}",
                    r.pick(&["black", "white", "walnut", "graphite"]),
                    r.below(300) as f64 / 10.0
                )),
                ("properties", _) => {
                    if r.chance(30) {
                        Value::Null
                    } else {
                        Value::Json(format!(
                            "{{\"path\": \"/{}\", \"referrer\": \"{}\"}}",
                            r.pick(&["", "pricing", "docs", "cart", "checkout"]),
                            r.pick(&["google", "direct", "newsletter", "twitter"])
                        ))
                    }
                }
                (_, ColType::Json) => Value::Null,
                ("created_at", _)
                | ("occurred_at", _)
                | ("changed_at", _)
                | ("attempted_at", _) => {
                    let year = 2024 + r.below(2) as i32;
                    Value::Text(timestamp(&mut r, year))
                }
                (_, ColType::Timestamp) => {
                    if c.nullable && r.chance(40) {
                        Value::Null
                    } else {
                        Value::Text(timestamp(&mut r, 2025))
                    }
                }
                (_, ColType::Date) => {
                    if c.nullable && r.chance(50) {
                        Value::Null
                    } else {
                        Value::Text(date(&mut r, 2025))
                    }
                }
                (_, ColType::Text) => Value::Text(format!("{first} {last}")),
            };
            row.push(v);
        }
        out.push(row);
    }
    out
}
