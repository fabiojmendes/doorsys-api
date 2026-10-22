use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use doorsys_api::{
    error::AppError,
    http,
    modules::{
        customers::NewCustomer,
        entry_logs::RecordEntry,
        staff::{CredentialPublisher, NewStaff, PublishFuture},
    },
    state::AppState,
};
use doorsys_protocol::UserAction;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tower::ServiceExt;

#[derive(Default)]
struct RecordingPublisher {
    commands: Mutex<Vec<UserAction>>,
    fail: AtomicBool,
}
impl CredentialPublisher for RecordingPublisher {
    fn publish(&self, command: UserAction) -> PublishFuture<'_> {
        Box::pin(async move {
            if self.fail.load(Ordering::SeqCst) {
                anyhow::bail!("simulated broker failure");
            }
            self.commands.lock().unwrap().push(command);
            Ok(())
        })
    }
}
impl RecordingPublisher {
    fn take(&self) -> Vec<UserAction> {
        std::mem::take(&mut *self.commands.lock().unwrap())
    }
}
fn setup(pool: &PgPool) -> (AppState, Arc<RecordingPublisher>) {
    let publisher = Arc::new(RecordingPublisher::default());
    (AppState::new(pool.clone(), publisher.clone()), publisher)
}
async fn customer(state: &AppState) -> i64 {
    state
        .customers
        .create(&NewCustomer {
            name: "Example".into(),
            email: "example@example.com".into(),
            notes: None,
        })
        .await
        .unwrap()
        .id
}
async fn seed_staff(
    pool: &PgPool,
    customer_id: i64,
    name: &str,
    pin: i32,
    fob: Option<i32>,
) -> i64 {
    sqlx::query_scalar("insert into staff (customer_id, name, phone, pin, fob) values ($1, $2, '', $3, $4) returning id")
        .bind(customer_id).bind(name).bind(pin).bind(fob).fetch_one(pool).await.unwrap()
}
fn staff_input(customer_id: i64, fob: Option<i32>) -> NewStaff {
    NewStaff {
        customer_id,
        name: "Alice".into(),
        phone: "123".into(),
        fob,
    }
}

#[sqlx::test]
async fn cascade_rolls_back_both_domains_and_never_publishes_on_failure(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    let staff_id = seed_staff(&pool, customer_id, "Alice", 123456, None).await;
    sqlx::raw_sql("CREATE FUNCTION reject_deactivation() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NOT NEW.active THEN RAISE EXCEPTION 'test rejection'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_deactivation BEFORE UPDATE ON staff FOR EACH ROW EXECUTE FUNCTION reject_deactivation();")
        .execute(&pool).await.unwrap();
    assert!(
        state
            .customer_status
            .update(customer_id, false)
            .await
            .is_err()
    );
    assert!(state.customers.get(customer_id).await.unwrap().active);
    assert!(state.staff.get(staff_id).await.unwrap().active);
    assert!(publisher.take().is_empty());
}

#[sqlx::test]
async fn cascade_preserves_soft_deletion_and_reactivates_remaining_staff(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    let alice = seed_staff(&pool, customer_id, "Alice", 123456, Some(42)).await;
    let bob = seed_staff(&pool, customer_id, "Bob", 234567, None).await;
    state.staff.delete(bob).await.unwrap();
    publisher.take();
    state
        .customer_status
        .update(customer_id, false)
        .await
        .unwrap();
    assert_eq!(
        publisher.take(),
        vec![UserAction::Del(123456), UserAction::Del(42)]
    );
    state
        .customer_status
        .update(customer_id, true)
        .await
        .unwrap();
    assert!(state.staff.get(alice).await.unwrap().active);
    let deleted = state.staff.get(bob).await.unwrap();
    assert!(deleted.deleted.is_some());
    assert!(!deleted.active);
    assert_eq!(state.staff.list(customer_id).await.unwrap().len(), 1);
    assert!(matches!(
        state.staff.update_status(bob, true).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        state.staff.update_pin(bob).await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        state
            .staff
            .update(bob, &staff_input(customer_id, None))
            .await,
        Err(AppError::Conflict(_))
    ));
    assert!(matches!(
        state.staff.delete(bob).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        publisher.take(),
        vec![UserAction::Add(123456), UserAction::Add(42)]
    );
    state.staff.bulk_load_codes().await.unwrap();
    assert_eq!(publisher.take(), vec![UserAction::Bulk(vec![42, 123456])]);
}

#[sqlx::test]
async fn inactive_credentials_change_without_granting_access(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    let id = seed_staff(&pool, customer_id, "Alice", 123456, Some(42)).await;
    state.staff.update_status(id, false).await.unwrap();
    publisher.take();
    state
        .staff
        .update(id, &staff_input(customer_id, Some(43)))
        .await
        .unwrap();
    let staff = state.staff.update_pin(id).await.unwrap();
    assert!((100000..=999999).contains(&staff.pin));
    assert_ne!(staff.pin, 123456);
    assert!(publisher.take().is_empty());
    state.staff.update_status(id, true).await.unwrap();
    assert_eq!(
        publisher.take(),
        vec![UserAction::Add(staff.pin), UserAction::Add(43)]
    );
}

#[sqlx::test]
async fn concurrent_updates_publish_a_consistent_credential_chain(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    let id = seed_staff(&pool, customer_id, "Alice", 123456, Some(42)).await;
    let first = staff_input(customer_id, Some(43));
    let second = staff_input(customer_id, Some(44));
    let (a, b) = tokio::join!(
        state.staff.update(id, &first),
        state.staff.update(id, &second)
    );
    a.unwrap();
    b.unwrap();
    let commands = publisher.take();
    let mut code = 42;
    for command in commands {
        let UserAction::Replace { old, new } = command else {
            panic!("expected replacement");
        };
        assert_eq!(old, code);
        code = new;
    }
    assert_eq!(state.staff.get(id).await.unwrap().fob, Some(code));
}

#[sqlx::test]
async fn publish_failure_is_reported_after_commit_and_bulk_can_reconcile(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    publisher.fail.store(true, Ordering::SeqCst);
    assert!(matches!(
        state.staff.create(&staff_input(customer_id, None)).await,
        Err(AppError::SyncFailed(_))
    ));
    let staff = state.staff.list(customer_id).await.unwrap();
    assert_eq!(staff.len(), 1);
    publisher.fail.store(false, Ordering::SeqCst);
    state.staff.bulk_load_codes().await.unwrap();
    assert_eq!(publisher.take(), vec![UserAction::Bulk(vec![staff[0].pin])]);
}

#[sqlx::test]
async fn audit_deduplication_does_not_hide_other_constraints(pool: PgPool) {
    let (state, _) = setup(&pool);
    let entry = RecordEntry {
        code: 123456,
        code_type: "pin".into(),
        net_id: "unknown".into(),
        success: true,
        event_date: chrono::Utc::now(),
    };
    state.entry_logs.record(&entry).await.unwrap();
    assert!(matches!(
        state.entry_logs.record(&entry).await,
        Err(AppError::DuplicateEntryLog)
    ));
    sqlx::query("alter table entry_log add constraint test_positive_code check (code > 0)")
        .execute(&pool)
        .await
        .unwrap();
    let invalid = RecordEntry {
        code: -1,
        ..entry.clone()
    };
    assert!(matches!(
        state.entry_logs.record(&invalid).await,
        Err(AppError::InvalidInput(_))
    ));
    let rows = state
        .entry_logs
        .list(
            (entry.event_date - chrono::Duration::seconds(1))
                ..(entry.event_date + chrono::Duration::seconds(1)),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].staff_id.is_none());
    assert!(rows[0].device_id.is_none());
}

async fn request(
    app: &axum::Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (u16, Value) {
    let mut request = Request::builder().method(method).uri(path);
    let payload = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            body.to_string()
        }
        None => String::new(),
    };
    let response = app
        .clone()
        .oneshot(request.body(Body::from(payload)).unwrap())
        .await
        .unwrap();
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        },
    )
}

#[sqlx::test]
async fn routes_and_openapi_preserve_contracts_and_document_errors(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let app = http::router(state);
    assert_eq!(
        request(&app, "GET", "/", None).await,
        (200, json!({"ok":true}))
    );
    let (status, customer) = request(
        &app,
        "POST",
        "/customers",
        Some(json!({"name":"Example","email":"test@example.com","notes":null})),
    )
    .await;
    assert_eq!(status, 200);
    let customer_id = customer["id"].as_i64().unwrap();
    let input = json!({"customerId":customer_id,"name":"Alice","phone":"123","fob":42});
    let (status, staff) = request(&app, "POST", "/staff", Some(input.clone())).await;
    assert_eq!(status, 200);
    assert_eq!(staff["customerId"], customer_id);
    assert_eq!(
        publisher.take(),
        vec![
            UserAction::Add(staff["pin"].as_i64().unwrap() as i32),
            UserAction::Add(42)
        ]
    );
    assert_eq!(request(&app, "POST", "/staff", Some(input)).await.0, 409);
    assert_eq!(request(&app, "GET", "/staff/999999", None).await.0, 404);
    assert_eq!(
        request(
            &app,
            "GET",
            &format!("/customers/{customer_id}/staff"),
            None
        )
        .await
        .1
        .as_array()
        .unwrap()
        .len(),
        1
    );
    assert_eq!(
        request(&app, "POST", "/admin/bulk", None).await,
        (200, Value::Null)
    );
    let (_, api) = request(&app, "GET", "/openapi", None).await;
    for path in [
        "/",
        "/openapi",
        "/customers",
        "/customers/{id}",
        "/customers/{id}/status",
        "/customers/{id}/staff",
        "/staff",
        "/staff/{id}",
        "/staff/{id}/pin",
        "/staff/{id}/status",
        "/devices",
        "/entry_logs",
        "/admin/bulk",
    ] {
        assert!(api["paths"].get(path).is_some(), "missing {path}");
    }
    for code in ["400", "404", "409", "500", "503"] {
        assert!(
            api["paths"]["/staff"]["post"]["responses"]
                .get(code)
                .is_some(),
            "missing {code}"
        );
    }
}

#[sqlx::test]
async fn mqtt_runtime_records_audits_and_disconnects_cleanly(pool: PgPool) {
    use doorsys_api::mqtt;
    use doorsys_protocol::{Audit, CodeType};
    use rumqttc::{AsyncClient, MqttOptions};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        sync::watch,
        time::{Duration, timeout},
    };

    async fn packet(stream: &mut TcpStream) -> (u8, Vec<u8>) {
        let header = stream.read_u8().await.unwrap();
        let (mut length, mut multiplier) = (0usize, 1usize);
        loop {
            let byte = stream.read_u8().await.unwrap();
            length += usize::from(byte & 127) * multiplier;
            if byte & 128 == 0 {
                break;
            }
            multiplier *= 128;
        }
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes).await.unwrap();
        (header, bytes)
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let broker = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        assert_eq!(packet(&mut socket).await.0 >> 4, 1);
        socket.write_all(&[0x20, 2, 0, 0]).await.unwrap();
        let mut saw_command = false;
        loop {
            let (header, bytes) = packet(&mut socket).await;
            match header >> 4 {
                8 => {
                    socket
                        .write_all(&[0x90, 3, bytes[0], bytes[1], 1])
                        .await
                        .unwrap();
                    let audit = Audit {
                        timestamp: std::time::SystemTime::now().into(),
                        code: 654321,
                        code_type: CodeType::Pin,
                        success: true,
                    };
                    let topic = b"doorsys/audit/door-1";
                    let mut body = (topic.len() as u16).to_be_bytes().to_vec();
                    body.extend(topic);
                    body.extend([0, 8]);
                    body.extend(postcard::to_allocvec(&audit).unwrap());
                    assert!(body.len() < 128);
                    socket.write_all(&[0x32, body.len() as u8]).await.unwrap();
                    socket.write_all(&body).await.unwrap();
                }
                3 => {
                    let len = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                    assert_eq!(&bytes[2..2 + len], b"doorsys/user");
                    assert_eq!(header & 0x07, 2); // QoS 1, not retained.
                    let id = &bytes[2 + len..4 + len];
                    let command: UserAction = postcard::from_bytes(&bytes[4 + len..]).unwrap();
                    assert_eq!(command, UserAction::Add(123456));
                    socket.write_all(&[0x40, 2, id[0], id[1]]).await.unwrap();
                    saw_command = true;
                }
                4 => {} // Audit PUBACK.
                12 => socket.write_all(&[0xd0, 0]).await.unwrap(),
                14 => {
                    assert!(saw_command);
                    break;
                }
                other => panic!("unexpected MQTT packet {other}"),
            }
        }
    });
    let (client, events) = AsyncClient::new(MqttOptions::new("test", "127.0.0.1", addr.port()), 10);
    let publisher = mqtt::MqttPublisher::new(client.clone());
    let (stop, shutdown) = watch::channel(false);
    let task = tokio::spawn(mqtt::run(
        client,
        events,
        doorsys_api::modules::entry_logs::EntryLogService::new(pool.clone()),
        shutdown,
    ));
    publisher.publish(UserAction::Add(123456)).await.unwrap();
    timeout(Duration::from_secs(5), async {
        loop {
            let count: i64 = sqlx::query_scalar("select count(*) from entry_log")
                .fetch_one(&pool)
                .await
                .unwrap();
            if count == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    stop.send_replace(true);
    timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(5), broker)
        .await
        .unwrap()
        .unwrap();
}

#[sqlx::test]
async fn constraint_errors_return_actionable_messages(pool: PgPool) {
    let (state, publisher) = setup(&pool);
    let customer_id = customer(&state).await;
    seed_staff(&pool, customer_id, "Alice", 123456, Some(42)).await;
    let bob = seed_staff(&pool, customer_id, "Bob", 234567, None).await;
    let app = http::router(state);
    for (method, path, input, status, message) in [
        (
            "POST",
            "/staff".into(),
            json!({"customerId":customer_id,"name":"Carol","phone":"123","fob":42}),
            409,
            "Fob is already in use by another staff member",
        ),
        (
            "PUT",
            format!("/staff/{bob}"),
            json!({"customerId":customer_id,"name":"Bob","phone":"123","fob":42}),
            409,
            "Fob is already in use by another staff member",
        ),
        (
            "POST",
            "/staff".into(),
            json!({"customerId":customer_id,"name":"Alice","phone":"123"}),
            409,
            "A staff member with this name already exists for this customer",
        ),
        (
            "POST",
            "/customers".into(),
            json!({"name":"Example","email":"new@example.com"}),
            409,
            "A customer with this name already exists",
        ),
        (
            "POST",
            "/staff".into(),
            json!({"customerId":-1,"name":"Carol","phone":"123"}),
            400,
            "Customer does not exist",
        ),
    ] {
        let response = request(&app, method, &path, Some(input)).await;
        assert_eq!(
            response,
            (status, json!({"code":status,"success":false,"msg":message}))
        );
    }
    assert!(publisher.take().is_empty());
}

#[sqlx::test]
async fn database_details_are_logged_and_unknown_constraints_have_safe_messages(pool: PgPool) {
    use std::io::Write;
    #[derive(Clone)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);
    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (state, _) = setup(&pool);
    let customer_id = customer(&state).await;
    seed_staff(&pool, customer_id, "Alice", 123456, Some(42)).await;
    let error = sqlx::query(
        "insert into staff (customer_id, name, phone, pin, fob) values ($1, 'Bob', '', 234567, 42)",
    )
    .bind(customer_id)
    .execute(&pool)
    .await
    .unwrap_err();
    let buffer = LogBuffer(Arc::new(Mutex::new(Vec::new())));
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .with_writer(move || writer.clone())
        .finish();
    let mapped = tracing::subscriber::with_default(subscriber, || AppError::from(error));
    assert_eq!(
        mapped.to_string(),
        "Fob is already in use by another staff member"
    );
    let log = String::from_utf8(buffer.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("WARN"));
    assert!(log.contains("staff_fob_key"));
    assert!(log.contains("duplicate key value violates unique constraint"));

    sqlx::raw_sql("create table test_constraint (value int constraint unfamiliar_constraint unique); insert into test_constraint values (1)")
        .execute(&pool).await.unwrap();
    let error = sqlx::query("insert into test_constraint values (1)")
        .execute(&pool)
        .await
        .unwrap_err();
    let mapped = AppError::from(error);
    assert!(matches!(mapped, AppError::Conflict(_)));
    assert_eq!(
        mapped.to_string(),
        "Record conflicts with an existing record"
    );
}
