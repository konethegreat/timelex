// Written by Kone Tshivhinda & Claude
//! P2-02: the connector runner. A page's events and its cursor move together, and replaying a
//! connector from the start finds the events it already stored.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use time::{Duration, OffsetDateTime};
use timelex_capture_service::{
    CaptureStore, Connector, ConnectorError, Page, Runner, RunnerError, StoreError, MIGRATIONS,
};
use timelex_db::testing::{setup as setup_db, ServiceUnderTest, TestDb};
use timelex_db::TenantPool;
use timelex_identity::{TenantId, UserId};
use timelex_types as types;
use timelex_types::ActivityEvent;
use uuid::Uuid;

struct Env {
    _db: TestDb,
    store: CaptureStore,
    tenants: TenantPool,
}

async fn env() -> Option<Env> {
    let db = setup_db(&ServiceUnderTest {
        service: "capture",
        migrations: &MIGRATIONS,
    })
    .await?;
    let (_, tenants) = db.service_pool().await;
    Some(Env {
        _db: db,
        store: CaptureStore::new(tenants.clone()),
        tenants,
    })
}

fn tenant() -> TenantId {
    TenantId::from_uuid(Uuid::new_v4())
}

fn user() -> UserId {
    UserId::from_uuid(Uuid::new_v4())
}

fn event(tenant: TenantId, user: UserId, source_id: &str, started: OffsetDateTime) -> ActivityEvent {
    let mut event = ActivityEvent {
        schema_version: types::ACTIVITY_EVENT_SCHEMA_VERSION.to_owned(),
        id: String::new(),
        tenant_id: format!("tn_{}", tenant.to_hex32()),
        source: types::Source {
            system: types::SourceSystem::M365Mail,
            source_id: source_id.to_owned(),
        },
        kind: types::ActivityKind::EmailSent,
        actor: types::Actor {
            user_id: format!("us_{}", user.as_uuid().simple()),
        },
        started_at: started,
        ended_at: started + Duration::minutes(8),
        captured_at: started + Duration::minutes(9),
        subject: None,
        content_ref: None,
        privacy: types::Privacy {
            content_captured: false,
            redaction_applied: false,
        },
    };
    event.id = event.expected_id();
    event
}

/// A connector that plays a fixed script, keyed by the cursor it is given, and remembers each call.
struct Scripted {
    name: &'static str,
    script: HashMap<Option<String>, Result<Page, ConnectorError>>,
    calls: Mutex<Vec<Option<String>>>,
}

impl Scripted {
    fn new(
        name: &'static str,
        script: impl IntoIterator<Item = (Option<String>, Result<Page, ConnectorError>)>,
    ) -> Self {
        Self {
            name,
            script: script.into_iter().collect(),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn cursors(&self) -> Vec<Option<String>> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl Connector for Scripted {
    fn name(&self) -> &str {
        self.name
    }

    async fn fetch(&self, cursor: Option<&str>, _limit: usize) -> Result<Page, ConnectorError> {
        self.calls.lock().unwrap().push(cursor.map(str::to_owned));
        self.script
            .get(&cursor.map(str::to_owned))
            .cloned()
            .expect("the runner asked for a cursor the script does not have")
    }
}

fn two_pages(tenant: TenantId, user: UserId, at: OffsetDateTime) -> Scripted {
    Scripted::new(
        "m365_mail",
        [
            (
                None,
                Ok(Page {
                    events: vec![event(tenant, user, "synthetic-message-a", at)],
                    next: Some("c2".into()),
                }),
            ),
            (
                Some("c2".into()),
                Ok(Page {
                    events: vec![event(tenant, user, "synthetic-message-b", at)],
                    next: None,
                }),
            ),
        ],
    )
}

async fn count_events(tenants: &TenantPool, tenant: TenantId) -> i64 {
    let mut tx = tenants.begin(tenant).await.unwrap();
    let count = sqlx::query_scalar("SELECT count(*) FROM capture.activity_events")
        .fetch_one(tx.conn())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    count
}

#[tokio::test]
async fn a_run_drains_every_page_and_advances_the_cursor() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let connector = Arc::new(two_pages(tenant, user(), OffsetDateTime::now_utc()));
    let runner = Runner::new(env.store.clone(), vec![connector.clone()]);

    let reports = runner.run_once(tenant).await;
    assert_eq!(reports.len(), 1);
    let report = reports[0].as_ref().unwrap();
    assert_eq!(report.connector, "m365_mail");
    assert_eq!(report.pages, 2);
    assert_eq!(report.created, 2);
    assert_eq!(report.existing, 0);
    assert_eq!(count_events(&env.tenants, tenant).await, 2);
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("c2".into())
    );
    assert_eq!(
        connector.cursors(),
        [None, Some("c2".into())],
        "the second page was fetched with the first page's next cursor"
    );
}

#[tokio::test]
async fn acceptance_a_replay_from_the_start_creates_nothing_new() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let connector = Arc::new(two_pages(tenant, user(), OffsetDateTime::now_utc()));
    let runner = Runner::new(env.store.clone(), vec![connector.clone()]);
    runner.run_once(tenant).await[0].as_ref().unwrap();

    let replayed = runner.replay(tenant, "m365_mail").await.unwrap();
    assert_eq!(replayed.pages, 2);
    assert_eq!(replayed.created, 0, "every event was already stored");
    assert_eq!(replayed.existing, 2);
    assert_eq!(count_events(&env.tenants, tenant).await, 2);
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("c2".into()),
        "a replay does not move the stored cursor"
    );
}

#[tokio::test]
async fn resuming_at_the_stored_cursor_re_reads_the_last_page_without_duplicating() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let connector = Arc::new(two_pages(tenant, user(), OffsetDateTime::now_utc()));
    let runner = Runner::new(env.store.clone(), vec![connector.clone()]);
    runner.run_once(tenant).await[0].as_ref().unwrap();

    let second = runner.run_once(tenant).await[0].as_ref().unwrap().clone();
    assert_eq!(second.pages, 1);
    assert_eq!(second.created, 0);
    assert_eq!(second.existing, 1);
    assert_eq!(count_events(&env.tenants, tenant).await, 2);
}

#[tokio::test]
async fn a_page_that_does_not_advance_is_refused_instead_of_looping() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let connector = Arc::new(Scripted::new(
        "stuck",
        [
            (
                None,
                Ok(Page {
                    events: vec![],
                    next: Some("same".into()),
                }),
            ),
            (
                Some("same".into()),
                Ok(Page {
                    events: vec![],
                    next: Some("same".into()),
                }),
            ),
        ],
    ));
    let runner = Runner::new(env.store.clone(), vec![connector]);

    let error = runner.run_once(tenant).await.remove(0).unwrap_err();
    assert!(
        matches!(error, RunnerError::Connector(ConnectorError::Invalid(_))),
        "{error}"
    );
}

#[tokio::test]
async fn a_connector_failure_keeps_the_last_good_cursor() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let at = OffsetDateTime::now_utc();
    let connector = Arc::new(Scripted::new(
        "m365_mail",
        [
            (
                None,
                Ok(Page {
                    events: vec![event(tenant, user(), "synthetic-message-a", at)],
                    next: Some("c2".into()),
                }),
            ),
            (
                Some("c2".into()),
                Err(ConnectorError::Unavailable("graph is down".into())),
            ),
        ],
    ));
    let runner = Runner::new(env.store.clone(), vec![connector]);

    let error = runner.run_once(tenant).await.remove(0).unwrap_err();
    assert!(
        matches!(error, RunnerError::Connector(ConnectorError::Unavailable(_))),
        "{error}"
    );
    assert_eq!(count_events(&env.tenants, tenant).await, 1, "page one is kept");
    assert_eq!(
        env.store.cursor(tenant, "m365_mail").await.unwrap(),
        Some("c2".into())
    );
}

#[tokio::test]
async fn a_page_larger_than_the_limit_is_refused_before_storing_anything() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let at = OffsetDateTime::now_utc();
    let user = user();
    let connector = Arc::new(Scripted::new(
        "m365_mail",
        [(
            None,
            Ok(Page {
                events: vec![
                    event(tenant, user, "synthetic-message-a", at),
                    event(tenant, user, "synthetic-message-b", at),
                ],
                next: None,
            }),
        )],
    ));
    let runner = Runner::with_page_size(env.store.clone(), vec![connector], 1);

    let error = runner.run_once(tenant).await.remove(0).unwrap_err();
    assert!(
        matches!(error, RunnerError::Connector(ConnectorError::Invalid(_))),
        "{error}"
    );
    assert_eq!(count_events(&env.tenants, tenant).await, 0);
}

#[tokio::test]
async fn an_unknown_connector_is_reported() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let runner = Runner::new(env.store.clone(), vec![]);
    let error = runner.replay(tenant, "m365_mail").await.unwrap_err();
    assert!(matches!(error, RunnerError::UnknownConnector(_)), "{error}");
}

#[tokio::test]
async fn run_once_reports_each_connector_separately() {
    let Some(env) = env().await else { return };
    let tenant = tenant();
    let at = OffsetDateTime::now_utc();
    let user = user();
    let ok = Arc::new(Scripted::new(
        "google_mail",
        [(
            None,
            Ok(Page {
                events: vec![event(tenant, user, "synthetic-message-g", at)],
                next: None,
            }),
        )],
    ));
    let broken = Arc::new(Scripted::new(
        "m365_mail",
        [(None, Err(ConnectorError::Unavailable("graph is down".into())))],
    ));
    let runner = Runner::new(env.store.clone(), vec![ok, broken]);

    let reports = runner.run_once(tenant).await;
    assert_eq!(reports.len(), 2);
    assert!(reports[0].is_ok());
    assert!(matches!(
        reports[1],
        Err(RunnerError::Connector(ConnectorError::Unavailable(_)))
    ));
    assert_eq!(count_events(&env.tenants, tenant).await, 1);
}

#[tokio::test]
async fn a_connector_cannot_store_an_event_for_another_firm() {
    let Some(env) = env().await else { return };
    let tenant_id = tenant();
    let other = tenant();
    let connector = Arc::new(Scripted::new(
        "m365_mail",
        [(
            None,
            Ok(Page {
                events: vec![event(
                    other,
                    user(),
                    "synthetic-message-a",
                    OffsetDateTime::now_utc(),
                )],
                next: None,
            }),
        )],
    ));
    let runner = Runner::new(env.store.clone(), vec![connector]);

    let error = runner.run_once(tenant_id).await.remove(0).unwrap_err();
    assert!(
        matches!(error, RunnerError::Store(StoreError::Invalid(_))),
        "{error}"
    );
    assert_eq!(count_events(&env.tenants, tenant_id).await, 0);
}
