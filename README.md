# Doorsys API

Axum REST API and MQTT device gateway for Doorsys, using PostgreSQL and SQLx.
The API supports the [admin console](https://github.com/fabiojmendes/doorsys-web);
MQTT carries credential commands and audit events for
[Doorsys firmware](https://github.com/fabiojmendes/doorsys-firmware).

## Structure

```text
src/
  main.rs                 Startup, task supervision, bounded shutdown
  lib.rs                  Library entry point for tests and embedding
  config.rs               Environment configuration
  db.rs                   Pool, migrations, database health check
  state.rs                Service construction and HTTP state extraction
  error.rs                Transport-independent application errors
  application/            Cross-domain workflows (customer status cascade)
  http/                   Router composition, OpenAPI, HTTP error mapping
  mqtt/                   Protocol encoding/decoding, publishing, supervised workers
  modules/
    customers/
    staff/
    devices/
    entry_logs/
```

Each feature owns its models, private SQLx repository, service, and routes.
HTTP routes extract inputs and call services. Services own business decisions
and transactions; repositories own SQL. Cross-domain workflows call explicit
service operations with a shared transaction connection, never another feature's
repository. Entry-log reporting owns its joined read projections.

PINs and fobs belong to staff. The staff service depends on a small
`CredentialPublisher` interface that accepts `doorsys_protocol::UserAction`
directly. The MQTT adapter owns Postcard serialization, topics, QoS, and the
rumqttc client. HTTP state contains services, not repositories
or transport clients. `http::router` builds a router without starting a server.

## Configuration

```sh
DATABASE_URL='postgres://user:pass@db-host/database'
MQTT_URL='mqtt://user:pass@mqtt-host?client_id=doorsys-api&clean_session=false&keep_alive_secs=5'
# Optional; defaults to 0.0.0.0:3000
HTTP_ADDR='0.0.0.0:3000'
RUST_LOG='doorsys_api=info,tower_http=info'
# Optional syslog-style priority prefix
SYSLOG_FORMAT=1
```

Migrations run on startup. `GET /` checks database connectivity; `GET /openapi`
returns the generated API description. Existing routes and successful JSON
payloads retain their shape, including camelCase fields and bare boolean status
requests. The existing staff update payload still includes `customerId`; updating
staff details does not transfer staff to a different customer.

Application errors return JSON `{ "code": 404, "success": false, "msg": "..." }`.
Missing records return 404, conflicts 409, invalid domain input/references 400,
unexpected failures 500, and publication failures 503. Framework-level malformed
JSON/path/query rejections retain Axum's default responses. Internal database and
broker details are logged, not returned to clients.

## Transactions and device synchronization

Customer activation/deactivation atomically sets the customer and every
non-deleted staff member to the requested status, preserving the existing cascade
policy. Reactivation also reactivates individually disabled staff. Soft-deleted
staff remain available for historical reads but cannot be mutated or reactivated.
Editing credentials for inactive staff does not grant device access.

Staff read-modify-write operations lock rows in a transaction. Six-digit PIN
allocation retries PIN uniqueness collisions for up to five attempts. Commands
are published after commit. A shared mutex orders staff mutations, customer
cascades, and bulk synchronization within one application instance; database row
locks protect concurrent database updates. Command ordering across multiple API
instances is not guaranteed by this direct-publishing design.

MQTT continues to use `doorsys/user` and `doorsys/audit/+`, Postcard, and QoS 1.
Successful publication means acceptance into the client's in-memory queue, not
confirmation from a device. There is no transactional outbox: a crash, cancellation,
or transport failure after commit can leave devices out of sync. A detected
post-commit failure returns 503 stating that changes were saved. Reconcile using
`POST /admin/bulk` after connectivity is restored; do not blindly retry a create.
A disconnected broker can also cause backpressure while the client reconnects.

Audit persistence runs separately from MQTT polling through a queue of 100
messages. When full, the queue applies backpressure, which can delay polling.
Only violations of the existing `event_code_date` constraint count as duplicates.
MQTT acknowledgements retain their existing automatic behavior, so acknowledgements
can precede persistence; failed inserts are logged and are not durably retried.
This queue does not provide durable audit ingestion.

Shutdown stops HTTP intake, drains requests, closes audit intake and drains accepted
audits, then queues MQTT disconnect behind pending commands and closes the database
pool. The whole drain has a 30-second deadline; unfinished work is logged and
cancelled if it expires. Broker reconnection backs off five seconds, and unexpected
worker exits stop the application rather than leaving a partially running service.

## Development and validation

SQLx query macros use the checked-in `.sqlx` metadata for offline compilation:

```sh
SQLX_OFFLINE=true cargo check --locked --offline
cargo fmt --check
SQLX_OFFLINE=true cargo clippy --locked --offline --all-targets -- -D warnings
```

Database tests use `#[sqlx::test]`, create isolated databases, and apply migrations.
Use a dedicated PostgreSQL 15+ instance (the schema uses `UNIQUE NULLS NOT DISTINCT`)
with a role allowed to create databases. For example:

```sh
docker run --detach --rm --name doorsys-test-db \
  -e POSTGRES_PASSWORD=doorsys-test -e POSTGRES_DB=doorsys_test \
  -p 127.0.0.1:55432:5432 postgres:18
export DATABASE_URL='postgres://postgres:doorsys-test@127.0.0.1:55432/doorsys_test'
cargo sqlx migrate run
SQLX_OFFLINE=true cargo test --locked --offline --all-targets
# After changing SQL, regenerate and check in query metadata:
cargo sqlx prepare -- --all-targets
cargo sqlx prepare --check -- --all-targets
docker stop doorsys-test-db
```

Tests cover transaction rollback, commit-before-publication, PIN collision retries,
concurrent credential updates, inactive/deleted staff, cascading activation, audit
deduplication, HTTP/OpenAPI contracts, MQTT wire compatibility and shutdown, and
queue backpressure. Tests use recording publishers and a local MQTT protocol fixture;
no external broker is required.
