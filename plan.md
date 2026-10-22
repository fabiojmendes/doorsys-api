The most effective way to structure an Axum and SQLx application with
bidirectional MQTT integration is a **feature-based modular architecture driven
by a shared service layer**.

Because your application has two distinct entry points for I/O—HTTP requests
coming in via Axum and asynchronous events flowing in and out via MQTT—your
business logic cannot live inside your Axum route handlers. Instead, both Axum
and your MQTT worker should act as thin transport layers that delegate to a
central domain/service layer.

## 1. Directory & Module Layout

Organize your top-level `src/` directory by **domain feature** (vertical slices)
rather than technical layer, while keeping shared infrastructure (`db`, `mqtt`,
`error`) at the root. Grouping `pins` and `fobs` into a unified `credentials`
module (or keeping them adjacent) often helps, as they share similar lifecycle
and device-syncing logic.

```text
src/
├── main.rs                 # Bootstrap: config, DB pool, MQTT loop, Axum server
├── lib.rs                  # Exports modules for integration testing
├── config.rs               # Environment configuration (figment or dotenvy)
├── error.rs                # Unified AppError implementing Axum's IntoResponse
├── state.rs                # AppState (holds Services, DB Pool, MQTT Publisher)
├── db.rs                   # SQLx pool initialization and migration runner
├── mqtt/                   # MQTT transport layer (e.g., using rumqttc)
│   ├── mod.rs
│   ├── client.rs           # Wrapper around rumqttc::AsyncClient for publishing
│   ├── worker.rs           # EventLoop listener routing incoming topics to services
│   └── messages.rs         # MQTT payload DTOs (SyncPinPayload, AuditEventPayload)
└── modules/                # Feature domains
    ├── mod.rs
    ├── customers/          # Each module contains its own routes, service, repo, models
    │   ├── mod.rs
    │   ├── routes.rs       # Axum Router & HTTP handlers
    │   ├── service.rs      # Business logic
    │   ├── repo.rs         # SQLx queries
    │   └── models.rs       # Domain structs & HTTP Request/Response DTOs
    ├── staff/
    ├── devices/
    └── audits/             # Primarily driven by MQTT worker + GET endpoints for UI
```

---

## 2. Architectural Flow & Responsibilities

To prevent spaghetti code between SQLx and your MQTT broker, enforce a strict
one-way dependency flow: **Transport (Axum / MQTT Worker) $\rightarrow$ Service
$\rightarrow$ Repository (SQLx) & MQTT Publisher**.

| Layer                                          | Responsibility                                                                                                                       | Rules & Constraints                                                                    |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| **Transport (`routes.rs` & `mqtt/worker.rs`)** | Deserializing payloads, extracting auth claims, validating input formats, and mapping outcomes to HTTP status codes or MQTT ACKs.    | Never write `sqlx::query!` or raw MQTT topic formatting here. Call a `Service` method. |
| **Service (`service.rs`)**                     | Orchestrates transactions and side effects. For example, updating a PIN in Postgres _and_ triggering a push to the device over MQTT. | Agnostic of HTTP. Returns domain errors, not `axum::http::StatusCode`.                 |
| **Repository (`repo.rs`)**                     | Pure data access using `sqlx`.                                                                                                       | Accepts `&PgPool` or `&mut PgConnection` (to support multi-step transactions).         |

---

## 3. Bridging Axum, SQLx, and MQTT in Code

### Shared State and the MQTT Publisher

Using a crate like `rumqttc`, splitting the client gives you a cloneable
`AsyncClient` (for pushing updates) and an `EventLoop` (for listening to audit
logs). Wrap the `AsyncClient` in a domain-specific publisher so your services
don't have to deal with raw byte payloads or topic strings.

```rust
// src/mqtt/client.rs
use rumqttc::{AsyncClient, QoS};
use serde::Serialize;
use crate::error::AppError;

#[derive(Clone)]
pub struct MqttPublisher {
    client: AsyncClient,
}

impl MqttPublisher {
    pub fn new(client: AsyncClient) -> Self {
        Self { client }
    }

    pub async fn publish_device_sync<T: Serialize>(
        &self,
        device_id: &str,
        payload: &T,
    ) -> Result<(), AppError> {
        let topic = format!("devices/{device_id}/sync");
        let bytes = serde_json::to_vec(payload)?;
        
        self.client
            .publish(topic, QoS::AtLeastOnce, false, bytes)
            .await
            .map_err(|e| AppError::MqttPublish(e.to_string()))?;
            
        Ok(())
    }
}
```

### Coordinating DB Writes & MQTT Pushes (The Service Layer)

When a staff member updates a PIN or Fob via the REST API, the
`CredentialService` handles both the database persistence and the MQTT
broadcast.

> **Architectural Note on Consistency:** If the database commit succeeds but the
> MQTT broker is temporarily unreachable, a direct publish might fail. For
> high-reliability access control, consider writing an `outbox` or `sync_status`
> flag in the database within the same SQLx transaction, or having devices
> acknowledge the MQTT push to mark the credential as `synced`.

```rust
// src/modules/credentials/service.rs
use sqlx::PgPool;
use crate::{error::AppError, mqtt::client::MqttPublisher};
use super::{models::{CreatePinRequest, PinCredential, DeviceSyncPayload}, repo::CredentialRepo};

#[derive(Clone)]
pub struct CredentialService {
    pool: PgPool,
    mqtt: MqttPublisher,
}

impl CredentialService {
    pub fn new(pool: PgPool, mqtt: MqttPublisher) -> Self {
        Self { pool, mqtt }
    }

    pub async fn assign_pin(&self, req: CreatePinRequest) -> Result<PinCredential, AppError> {
        // 1. Start a SQLx transaction if updating multiple tables
        let mut tx = self.pool.begin().await?;

        // 2. Persist the PIN via the repository (passing &mut *tx)
        let pin = CredentialRepo::insert_pin(&mut *tx, &req).await?;
        let target_devices = CredentialRepo::get_devices_for_customer(&mut *tx, req.customer_id).await?;

        tx.commit().await?;

        // 3. Push updates to affected devices over MQTT
        let sync_payload = DeviceSyncPayload::from(&pin);
        for device in target_devices {
            if let Err(e) = self.mqtt.publish_device_sync(&device.serial_number, &sync_payload).await {
                tracing::error!(device_id = %device.serial_number, error = ?e, "Failed to push PIN update to device");
                // Optional: trigger retry queue or leave device state as pending_sync
            }
        }

        Ok(pin)
    }
}
```

### Handling Flexible SQLx Executors in the Repository

To allow repository functions to be called both standalone (with `&PgPool`) and
inside transactions (with `&mut PgConnection`), write your repository methods
using SQLx's `PgExecutor` trait or pass `&mut PgConnection` for mutations:

```rust
// src/modules/credentials/repo.rs
use sqlx::{PgExecutor, Postgres};
use crate::error::AppError;
use super::models::{CreatePinRequest, PinCredential};

pub struct CredentialRepo;

impl CredentialRepo {
    pub async fn insert_pin<'e, E>(
        executor: E,
        req: &CreatePinRequest,
    ) -> Result<PinCredential, AppError>
    where
        E: PgExecutor<'e>,
    {
        let record = sqlx::query_as!(
            PinCredential,
            r#"
            INSERT INTO pins (customer_id, pin_hash, expires_at)
            VALUES ($1, $2, $3)
            RETURNING id, customer_id, pin_hash, expires_at, created_at
            "#,
            req.customer_id,
            req.pin_hash,
            req.expires_at
        )
        .fetch_one(executor)
        .await?;

        Ok(record)
    }
}
```

### The Inbound MQTT Worker (Audit Logs)

Your MQTT `EventLoop` runs in a background `tokio::spawn` task. It listens for
audit log topics from devices and calls the `AuditService` (or `AuditRepo`),
completely bypassing Axum while sharing the exact same `PgPool` and error
handling.

```rust
// src/mqtt/worker.rs
use rumqttc::{Event, EventLoop, Packet};
use crate::modules::audits::{service::AuditService, models::AuditLogPayload};

pub async fn run_mqtt_listener(mut eventloop: EventLoop, audit_service: AuditService) {
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                if publish.topic.starts_with("devices/") && publish.topic.ends_with("/audit") {
                    let svc = audit_service.clone();
                    
                    // Spawn a task per message so slow DB writes don't block MQTT keep-alives!
                    tokio::spawn(async move {
                        match serde_json::from_slice::<AuditLogPayload>(&publish.payload) {
                            Ok(payload) => {
                                if let Err(e) = svc.record_audit_log(payload).await {
                                    tracing::error!(error = ?e, "Failed to persist MQTT audit log");
                                }
                            }
                            Err(e) => tracing::warn!(error = ?e, "Malformed audit payload received"),
                        }
                    });
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(error = ?e, "MQTT connection error, backing off...");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}
```

---

## 4. Wiring It Together in `main.rs` and `state.rs`

In Axum, construct an `AppState` containing your services (or the `PgPool` and
`MqttPublisher` directly) and merge the routers from each feature module. Using
`FromRef` allows sub-routers to extract only the specific service or state slice
they care about.

```rust
// src/main.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    let pool = db::init_pool(&config.database_url).await?;
    
    // Initialize MQTT
    let (mqtt_client, mqtt_eventloop) = mqtt::init_client(&config.mqtt).await?;
    let publisher = MqttPublisher::new(mqtt_client);

    // Initialize Services
    let audit_service = AuditService::new(pool.clone());
    let credential_service = CredentialService::new(pool.clone(), publisher.clone());

    // 1. Spawn the MQTT background worker
    let mqtt_handle = tokio::spawn(mqtt::worker::run_mqtt_listener(
        mqtt_eventloop,
        audit_service.clone(),
    ));

    // 2. Build the Axum Router by nesting feature modules
    let app_state = AppState {
        pool,
        publisher,
        credential_service,
        audit_service,
    };

    let api_routes = axum::Router::new()
        .nest("/customers", modules::customers::routes::router())
        .nest("/staff", modules::staff::routes::router())
        .nest("/devices", modules::devices::routes::router())
        .nest("/credentials", modules::credentials::routes::router())
        .nest("/audits", modules::audits::routes::router())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(&config.listen_addr).await?;
    
    // Run server with graceful shutdown
    axum::serve(listener, api_routes)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    mqtt_handle.abort();
    Ok(())
}
```

---

## 5. Key Production Details Not to Miss

1. **Centralized `AppError` (`src/error.rs`):** Define a single enum using
   `thiserror` that wraps `sqlx::Error`, `rumqttc::ClientError`, validation
   errors, and domain errors (like `NotFound` or `Conflict`). Implement
   `axum::response::IntoResponse` for `AppError` so handlers can cleanly use the
   `?` operator and automatically return structured JSON error responses.
2. **Never Block the MQTT EventLoop:** In `rumqttc` (and similar async MQTT
   clients), `eventloop.poll().await` drives the underlying socket and
   PINGREQ/PINGRESP heartbeats. Always `tokio::spawn` the actual database
   insertion or passes payloads over a `tokio::sync::mpsc` channel to a worker
   pool so a slow database query doesn't cause the MQTT broker to disconnect
   your server for missing a heartbeat.
3. **High-Throughput Audit Logs:** If devices generate bursts of audit events,
   inserting them one by one via `tokio::spawn` can exhaust your SQLx connection
   pool. Instead, have the MQTT worker push parsed `AuditLogPayload` items into
   a `tokio::sync::mpsc::channel`, and run a batching task that flushes them to
   PostgreSQL using `sqlx::QueryBuilder` (or `UNNEST`) every $N$ items or $M$
   milliseconds.
