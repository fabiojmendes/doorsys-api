<!-- vim: set tw=80: -->

# Doorsys API

This repo contains the api portion of the doorsys platform.

Main roles:

- It provides a rest api for [admin console](https://github.com/fabiojmendes/doorsys-web)
for CRUD operations and viewing the audit logs
- Handles the communication with the mqtt broker for the
[doorsys-firmware](https://github.com/fabiojmendes/doorsys-firmware) in order to
add and remove users and persist audit logs

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

This is still working in progress. The api does very little in terms of user
input validation other than enforcing some DB constraints.

Transactions are still not implemented but there are use case for it in the
staff service.

At this point too much business logic is dealt with at the http handler. Ideally
this should be moved to another layer.
