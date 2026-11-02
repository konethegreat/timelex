// Written by Kone Tshivhinda & Claude
//! The connector framework: one trait a provider implements, and one runner that turns its pages
//! into stored activity events.
//!
//! A connector knows how to read one source (Microsoft 365 mail, Google calendar, the desktop
//! agent). It returns a [`Page`] of normalised events and an opaque `next` cursor. It never writes
//! to the database itself and never decides whether an event is new: the runner stores the page
//! through [`CaptureStore::record_page`], which is idempotent.
//!
//! A page's events and the cursor that follows them commit together. A failure therefore leaves the
//! cursor on the last fully stored page, and the next run re-reads the page that failed instead of
//! skipping it. Replay starts from the beginning and relies on the same idempotency: already-stored
//! signals are found, not duplicated (ADR-0021).

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use timelex_identity::TenantId;
use timelex_types::ActivityEvent;

use crate::error::StoreError;
use crate::store::CaptureStore;

/// How many events a connector is asked for at most. Also the run's safety check: a connector that
/// returns more than it was asked for is refused before anything is stored.
pub const DEFAULT_PAGE_SIZE: usize = 100;

/// One page of normalised events and where to continue from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    pub events: Vec<ActivityEvent>,
    /// `None` means the connector is caught up: there is nothing after this page yet.
    pub next: Option<String>,
}

/// Why a connector could not produce a page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorError {
    /// The source could not be read this time (network, throttling, provider outage). Retrying later
    /// may work. The message is for the operator and must never contain a credential or content.
    Unavailable(String),
    /// The connector or its source produced something this service cannot honestly process.
    /// Retrying will not help; a person has to look.
    Invalid(String),
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(message) => write!(f, "the source is unavailable: {message}"),
            Self::Invalid(message) => write!(f, "the connector cannot continue: {message}"),
        }
    }
}

impl std::error::Error for ConnectorError {}

/// One provider's reader. Implementations are stateless about progress: the cursor is passed in and
/// returned, so a restart resumes exactly where the stored cursor says.
#[async_trait]
pub trait Connector: Send + Sync {
    /// The fixed name, also the cursor key and the name in reports. Lowercase, for example
    /// `m365_mail`.
    fn name(&self) -> &str;

    /// Read at most `limit` events after `cursor`, where `None` means from the beginning.
    async fn fetch(&self, cursor: Option<&str>, limit: usize) -> Result<Page, ConnectorError>;
}

/// What one connector did in one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub connector: String,
    pub pages: usize,
    pub created: usize,
    pub existing: usize,
    /// Signals the firm excludes. The cursor still moves past them.
    pub excluded: usize,
}

/// Why a run did not finish.
#[derive(Debug)]
pub enum RunnerError {
    /// No connector with that name is registered.
    UnknownConnector(String),
    /// A page could not be produced.
    Connector(ConnectorError),
    /// A page could not be stored. The cursor has not moved past it.
    Store(StoreError),
}

impl fmt::Display for RunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownConnector(name) => write!(f, "no connector named {name}"),
            Self::Connector(e) => write!(f, "{e}"),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RunnerError {}

impl From<ConnectorError> for RunnerError {
    fn from(e: ConnectorError) -> Self {
        Self::Connector(e)
    }
}

impl From<StoreError> for RunnerError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// Drives every registered connector for one firm, one page at a time.
#[derive(Clone)]
pub struct Runner {
    store: CaptureStore,
    connectors: Vec<Arc<dyn Connector>>,
    page_size: usize,
}

impl Runner {
    /// A runner over `connectors`, asking each for [`DEFAULT_PAGE_SIZE`] events at a time.
    pub fn new(store: CaptureStore, connectors: Vec<Arc<dyn Connector>>) -> Self {
        Self::with_page_size(store, connectors, DEFAULT_PAGE_SIZE)
    }

    /// A runner with a different page size (used by tests; kept small in production too, so one
    /// provider request cannot hold a transaction open for long). A zero is raised to one.
    pub fn with_page_size(
        store: CaptureStore,
        connectors: Vec<Arc<dyn Connector>>,
        page_size: usize,
    ) -> Self {
        Self {
            store,
            connectors,
            page_size: page_size.max(1),
        }
    }

    /// Run every connector once, continuing from each one's stored cursor. One connector's failure
    /// does not stop the others; each result is reported separately.
    pub async fn run_once(&self, tenant: TenantId) -> Vec<Result<RunReport, RunnerError>> {
        let mut reports = Vec::with_capacity(self.connectors.len());
        for connector in &self.connectors {
            let result = match self.store.cursor(tenant, connector.name()).await {
                Ok(cursor) => self.drain(tenant, connector.as_ref(), cursor, true).await,
                Err(e) => Err(RunnerError::Store(e)),
            };
            reports.push(result);
        }
        reports
    }

    /// Run one connector from its stored cursor.
    pub async fn run_connector(&self, tenant: TenantId, name: &str) -> Result<RunReport, RunnerError> {
        let connector = self.find(name)?;
        let cursor = self.store.cursor(tenant, name).await?;
        self.drain(tenant, connector.as_ref(), cursor, true).await
    }

    /// Re-read a connector from the beginning and store whatever is not stored yet. The stored
    /// cursor is not moved, so the connector's normal run continues where it was.
    pub async fn replay(&self, tenant: TenantId, name: &str) -> Result<RunReport, RunnerError> {
        let connector = self.find(name)?;
        self.drain(tenant, connector.as_ref(), None, false).await
    }

    fn find(&self, name: &str) -> Result<Arc<dyn Connector>, RunnerError> {
        self.connectors
            .iter()
            .find(|c| c.name() == name)
            .cloned()
            .ok_or_else(|| RunnerError::UnknownConnector(name.to_owned()))
    }

    async fn drain(
        &self,
        tenant: TenantId,
        connector: &dyn Connector,
        start: Option<String>,
        advance: bool,
    ) -> Result<RunReport, RunnerError> {
        let mut cursor = start;
        let mut report = RunReport {
            connector: connector.name().to_owned(),
            pages: 0,
            created: 0,
            existing: 0,
            excluded: 0,
        };
        loop {
            let used = cursor.clone();
            let page = connector.fetch(used.as_deref(), self.page_size).await?;
            if page.events.len() > self.page_size {
                return Err(ConnectorError::Invalid(format!(
                    "the connector returned {} events for a page limit of {}",
                    page.events.len(),
                    self.page_size
                ))
                .into());
            }
            let next = if advance { page.next.as_deref() } else { None };
            let outcome = self
                .store
                .record_page(tenant, connector.name(), &page.events, next)
                .await?;
            report.pages += 1;
            report.created += outcome.created;
            report.existing += outcome.existing;
            report.excluded += outcome.excluded;

            let Some(next) = page.next else { break };
            if used.as_deref() == Some(next.as_str()) {
                return Err(ConnectorError::Invalid(
                    "the connector's next cursor is the cursor it was given".into(),
                )
                .into());
            }
            cursor = Some(next);
        }
        Ok(report)
    }
}
