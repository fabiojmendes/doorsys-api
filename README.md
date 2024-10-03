<!-- vim: set tw=80: -->

# Doorsys API

This repository contains the code for the
[axum](https://github.com/tokio-rs/axum) based rest API for the Doorsys
platform. The communication with the Doorsys devices is carried through MQTT for
reliability purposes. The At-Least-Once (QoS 1) semantics of the protocol is
used to ensure messages are delivered even if components are offline. To deal
with message duplication, the consumers of the messages are idempotent. For
persistence, [sqlx](https://github.com/launchbadge/sqlx) and Postgres are used.

Main roles:

- It provides a rest api for
  [admin console](https://github.com/fabiojmendes/doorsys-web) for CRUD
  operations and viewing the audit logs
- Handles the communication with the MQTT broker for the
  [doorsys-firmware](https://github.com/fabiojmendes/doorsys-firmware) in order
  to add and remove users and persist audit logs

## Configuration

Two environment variables are expected in order to run the service:

```shell
# Database URL for the postgres instance
DATABASE_URL='postgres://user:pass@db-host/database'

# MQTT Broker url
MQTT_URL='mqtt://user:pass@mqtt-host?client_id=doorsys-dev&clean_session=false&keep_alive_secs=5'

# Optional logging
RUST_LOG='doorsys_api=trace,tower_http=trace'
```

## WIP

This is still working in progress. The API does very little in terms of user
input validation other than enforcing some DB constraints.

Transactions are still not implemented, but there are use case for it in the
staff service.

At this point too much business logic is dealt with at the HTTP handler. Ideally
this should be moved to another layer.
