use super::*;
use crate::modules::staff::PublishFuture;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CommittedPublisher {
    pool: PgPool,
    calls: AtomicUsize,
}
impl CredentialPublisher for CommittedPublisher {
    fn publish(&self, command: UserAction) -> PublishFuture<'_> {
        Box::pin(async move {
            let pin = match command {
                UserAction::Add(pin) | UserAction::Replace { new: pin, .. } => pin,
                _ => panic!("unexpected command"),
            };
            let exists: bool =
                sqlx::query_scalar("select exists(select 1 from staff where pin = $1)")
                    .bind(pin)
                    .fetch_one(&self.pool)
                    .await?;
            anyhow::ensure!(exists, "publish happened before commit");
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}
async fn fixture(pool: &PgPool) -> (StaffService, Arc<CommittedPublisher>, NewStaff) {
    let customer_id: i64 = sqlx::query_scalar(
        "insert into customer (name, email) values ('Example', 'example@example.com') returning id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let publisher = Arc::new(CommittedPublisher {
        pool: pool.clone(),
        calls: AtomicUsize::new(0),
    });
    let service = StaffService::new(pool.clone(), publisher.clone());
    let input = NewStaff {
        customer_id,
        name: "Alice".into(),
        phone: String::new(),
        fob: None,
    };
    (service, publisher, input)
}
#[sqlx::test]
async fn retries_pin_collisions_and_publishes_only_committed_records(pool: PgPool) {
    let (service, publisher, mut input) = fixture(&pool).await;
    service.create_with_pin(&input, || 123456).await.unwrap();
    input.name = "Bob".into();
    let mut candidates = [123456, 234567].into_iter();
    let bob = service
        .create_with_pin(&input, || candidates.next().unwrap())
        .await
        .unwrap();
    assert_eq!(bob.pin, 234567);
    let mut candidates = [123456, 345678].into_iter();
    let bob = service
        .update_pin_with(bob.id, || candidates.next().unwrap())
        .await
        .unwrap();
    assert_eq!(bob.pin, 345678);
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 3);
}
#[sqlx::test]
async fn collision_exhaustion_and_other_constraints_do_not_publish(pool: PgPool) {
    let (service, publisher, mut input) = fixture(&pool).await;
    let alice = service.create_with_pin(&input, || 123456).await.unwrap();
    input.name = "Bob".into();
    let mut attempts = 0;
    let result = service
        .create_with_pin(&input, || {
            attempts += 1;
            123456
        })
        .await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(attempts, 5);
    let bob = service.create_with_pin(&input, || 234567).await.unwrap();
    assert!(matches!(
        service.update_pin_with(bob.id, || alice.pin).await,
        Err(AppError::Conflict(_))
    ));
    assert_eq!(service.get(bob.id).await.unwrap().pin, 234567);
    input.customer_id = -1;
    let mut attempts = 0;
    let result = service
        .create_with_pin(&input, || {
            attempts += 1;
            345678
        })
        .await;
    assert!(matches!(result, Err(AppError::InvalidInput(_))));
    assert_eq!(attempts, 1);
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 2);
}
