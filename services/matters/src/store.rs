// Written by Kone Tshivhinda & Claude
//! The matters store (ADR-0027). Every query runs inside a [`TenantTx`](timelex_db::TenantTx), so
//! Postgres row-level security confines it to one firm. Validation comes from [`crate::domain`].

use time::{Date, OffsetDateTime};
use timelex_auth::Principal;
use timelex_db::{TenantPool, TenantTx};
use timelex_identity::{TenantId, UserId};
use uuid::Uuid;

use crate::domain::{
    self, AliasKind, ClientStatus, MatterStatus, Rounding, MAX_ALIAS_VALUE, MAX_HOURLY_RATE_CENTS,
};
use crate::error::StoreError;

/// Who is acting. Built only from a verified internal token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Actor {
    /// The firm.
    pub tenant: TenantId,
    /// The user acting.
    pub user: UserId,
}

impl From<&Principal> for Actor {
    fn from(principal: &Principal) -> Self {
        Self {
            tenant: principal.tenant,
            user: principal.user,
        }
    }
}

/// A client of the firm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    /// The firm's own identifier.
    pub client_id: String,
    /// Display name.
    pub name: String,
    /// Active or inactive.
    pub status: ClientStatus,
    /// When the row was first stored.
    pub created_at: OffsetDateTime,
    /// When it last changed.
    pub updated_at: OffsetDateTime,
}

/// A matter under a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matter {
    /// The firm's own identifier.
    pub matter_id: String,
    /// The client it belongs to.
    pub client_id: String,
    /// The reference people use.
    pub reference: String,
    /// Short description.
    pub title: String,
    /// Open or closed.
    pub status: MatterStatus,
    /// Practice area, when the firm records one.
    pub practice_area: Option<String>,
    /// When the matter was opened.
    pub opened_on: Option<Date>,
    /// When it was closed.
    pub closed_on: Option<Date>,
    /// When the row was first stored.
    pub created_at: OffsetDateTime,
    /// When it last changed.
    pub updated_at: OffsetDateTime,
}

/// A signal name that resolves to a client and, for a reference, a matter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    /// Surrogate id of the alias row.
    pub id: Uuid,
    /// What kind of name this is.
    pub kind: AliasKind,
    /// The case-folded value to match.
    pub value: String,
    /// The client it belongs to.
    pub client_id: String,
    /// The matter, for a `matter_reference` alias.
    pub matter_id: Option<String>,
}

/// A matter's billing policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateCard {
    /// The matter it prices.
    pub matter_id: String,
    /// Hourly rate in whole cents (ZAR).
    pub hourly_rate_cents: i64,
    /// Minutes in one billable unit.
    pub unit_minutes: i64,
    /// How a partial unit is rounded.
    pub rounding: Rounding,
    /// Minimum units charged for a non-zero duration.
    pub minimum_units: i64,
    /// When it last changed.
    pub updated_at: OffsetDateTime,
}

/// A client as a request or a CSV row presents it: raw, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewClient {
    /// The firm's identifier.
    pub client_id: String,
    /// Display name.
    pub name: String,
    /// `active` or `inactive`; defaults to `active`.
    pub status: Option<String>,
}

/// A matter as a request or a CSV row presents it: raw, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewMatter {
    /// The firm's identifier.
    pub matter_id: String,
    /// The client it belongs to.
    pub client_id: String,
    /// The reference people use.
    pub reference: String,
    /// Short description.
    pub title: String,
    /// `open` or `closed`; defaults to `open`.
    pub status: Option<String>,
    /// Practice area, when recorded.
    pub practice_area: Option<String>,
    /// ISO date, when recorded.
    pub opened_on: Option<String>,
    /// ISO date, when recorded.
    pub closed_on: Option<String>,
}

/// An alias as a request or a CSV row presents it: raw, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAlias {
    /// `matter_reference`, `client_name` or `client_domain`.
    pub kind: String,
    /// The name to match.
    pub value: String,
    /// The client it belongs to.
    pub client_id: String,
    /// The matter, required for `matter_reference`.
    pub matter_id: Option<String>,
}

/// A rate card as a request or a CSV row presents it: raw, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRateCard {
    /// The matter it prices.
    pub matter_id: String,
    /// Hourly rate in whole cents (ZAR).
    pub hourly_rate_cents: i64,
    /// Minutes in one unit; defaults to 6.
    pub unit_minutes: Option<i64>,
    /// `up`, `down` or `nearest`; defaults to `up`.
    pub rounding: Option<String>,
    /// Minimum units; defaults to 1.
    pub minimum_units: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct ClientRow {
    client_id: String,
    name: String,
    status: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<ClientRow> for Client {
    type Error = StoreError;

    fn try_from(row: ClientRow) -> Result<Self, Self::Error> {
        Ok(Self {
            client_id: row.client_id,
            name: row.name,
            status: ClientStatus::parse(&row.status).map_err(StoreError::corrupt)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct MatterRow {
    matter_id: String,
    client_id: String,
    reference: String,
    title: String,
    status: String,
    practice_area: Option<String>,
    opened_on: Option<Date>,
    closed_on: Option<Date>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<MatterRow> for Matter {
    type Error = StoreError;

    fn try_from(row: MatterRow) -> Result<Self, Self::Error> {
        Ok(Self {
            matter_id: row.matter_id,
            client_id: row.client_id,
            reference: row.reference,
            title: row.title,
            status: MatterStatus::parse(&row.status).map_err(StoreError::corrupt)?,
            practice_area: row.practice_area,
            opened_on: row.opened_on,
            closed_on: row.closed_on,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct AliasRow {
    id: Uuid,
    kind: String,
    value: String,
    client_id: String,
    matter_id: Option<String>,
}

impl TryFrom<AliasRow> for Alias {
    type Error = StoreError;

    fn try_from(row: AliasRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            kind: AliasKind::parse(&row.kind).map_err(StoreError::corrupt)?,
            value: row.value,
            client_id: row.client_id,
            matter_id: row.matter_id,
        })
    }
}

#[derive(sqlx::FromRow)]
struct RateCardRow {
    matter_id: String,
    hourly_rate_cents: i64,
    unit_minutes: i16,
    rounding: String,
    minimum_units: i16,
    updated_at: OffsetDateTime,
}

impl TryFrom<RateCardRow> for RateCard {
    type Error = StoreError;

    fn try_from(row: RateCardRow) -> Result<Self, Self::Error> {
        Ok(Self {
            matter_id: row.matter_id,
            hourly_rate_cents: row.hourly_rate_cents,
            unit_minutes: i64::from(row.unit_minutes),
            rounding: Rounding::parse(&row.rounding).map_err(StoreError::corrupt)?,
            minimum_units: i64::from(row.minimum_units),
            updated_at: row.updated_at,
        })
    }
}

struct ValidClient {
    client_id: String,
    name: String,
    status: ClientStatus,
}

struct ValidMatter {
    matter_id: String,
    client_id: String,
    reference: String,
    reference_key: String,
    title: String,
    status: MatterStatus,
    practice_area: Option<String>,
    opened_on: Option<Date>,
    closed_on: Option<Date>,
}

struct ValidAlias {
    kind: AliasKind,
    value: String,
    client_id: String,
    matter_id: Option<String>,
}

struct ValidRateCard {
    matter_id: String,
    hourly_rate_cents: i64,
    unit_minutes: i64,
    rounding: Rounding,
    minimum_units: i64,
}

fn validate_client(new: &NewClient) -> Result<ValidClient, StoreError> {
    Ok(ValidClient {
        client_id: domain::client_id(&new.client_id)?,
        name: domain::display_name(&new.name)?,
        status: match new.status.as_deref() {
            Some(raw) => ClientStatus::parse(raw)?,
            None => ClientStatus::Active,
        },
    })
}

fn validate_matter(new: &NewMatter) -> Result<ValidMatter, StoreError> {
    let opened_on = match new.opened_on.as_deref() {
        Some(raw) if !raw.trim().is_empty() => Some(domain::iso_date(raw)?),
        _ => None,
    };
    let closed_on = match new.closed_on.as_deref() {
        Some(raw) if !raw.trim().is_empty() => Some(domain::iso_date(raw)?),
        _ => None,
    };
    if let (Some(opened), Some(closed)) = (opened_on, closed_on) {
        if closed < opened {
            return Err(StoreError::Invalid("closedOn must not be before openedOn"));
        }
    }
    let practice_area = match new.practice_area.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(raw) => {
            if raw.chars().count() > 100 || raw.chars().any(char::is_control) {
                return Err(StoreError::Invalid(
                    "practiceArea must be 1 to 100 characters on one line",
                ));
            }
            Some(raw.to_owned())
        }
    };
    Ok(ValidMatter {
        matter_id: domain::matter_id(&new.matter_id)?,
        client_id: domain::client_id(&new.client_id)?,
        reference: domain::reference(&new.reference)?,
        reference_key: domain::reference_key(&new.reference),
        title: domain::display_name(&new.title)?,
        status: match new.status.as_deref() {
            Some(raw) => MatterStatus::parse(raw)?,
            None => MatterStatus::Open,
        },
        practice_area,
        opened_on,
        closed_on,
    })
}

fn validate_alias(new: &NewAlias) -> Result<ValidAlias, StoreError> {
    let kind = AliasKind::parse(&new.kind)?;
    let value = domain::alias_value(kind, &new.value)?;
    let matter_id = match new.matter_id.as_deref().map(str::trim) {
        Some("") | None => None,
        Some(raw) => Some(domain::matter_id(raw)?),
    };
    if value.chars().count() > MAX_ALIAS_VALUE {
        return Err(StoreError::Invalid("value must be 1 to 320 characters"));
    }
    domain::alias_target(kind, matter_id.is_some())?;
    Ok(ValidAlias {
        kind,
        value,
        client_id: domain::client_id(&new.client_id)?,
        matter_id,
    })
}

fn validate_rate_card(new: &NewRateCard) -> Result<ValidRateCard, StoreError> {
    if new.hourly_rate_cents.unsigned_abs() > MAX_HOURLY_RATE_CENTS as u64 {
        return Err(StoreError::Invalid("hourlyRateCents must be 0 to 100000000"));
    }
    Ok(ValidRateCard {
        matter_id: domain::matter_id(&new.matter_id)?,
        hourly_rate_cents: domain::hourly_rate_cents(new.hourly_rate_cents)?,
        unit_minutes: domain::unit_minutes(new.unit_minutes.unwrap_or(6))?,
        rounding: match new.rounding.as_deref() {
            Some(raw) => Rounding::parse(raw)?,
            None => Rounding::Up,
        },
        minimum_units: domain::minimum_units(new.minimum_units.unwrap_or(1))?,
    })
}

/// The matters store.
#[derive(Clone)]
pub struct Store {
    pool: TenantPool,
}

impl Store {
    /// A store over the service's connection pool.
    pub fn new(pool: TenantPool) -> Self {
        Self { pool }
    }

    /// Check the database answers. For `/health`; reads no tenant data.
    pub async fn ping(&self) -> Result<(), StoreError> {
        Ok(self.pool.ping().await?)
    }

    // ---------- clients ----------

    /// List the firm's clients by identifier.
    pub async fn list_clients(&self, actor: Actor) -> Result<Vec<Client>, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        let rows = sqlx::query_as::<_, ClientRow>(
            "SELECT client_id, name, status, created_at, updated_at
             FROM matters.clients ORDER BY client_id",
        )
        .fetch_all(tx.conn())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(Client::try_from).collect()
    }

    /// Create a client. An existing `clientId` is a conflict, not an overwrite.
    pub async fn create_client(&self, actor: Actor, new: NewClient) -> Result<Client, StoreError> {
        let valid = validate_client(&new)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        let row = insert_client(&mut tx, &valid).await?;
        tx.commit().await?;
        Client::try_from(row)
    }

    // ---------- matters ----------

    /// List matters, newest reference first, optionally for one client.
    pub async fn list_matters(
        &self,
        actor: Actor,
        client_id: Option<&str>,
    ) -> Result<Vec<Matter>, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        let rows = match client_id {
            Some(client_id) => {
                let client_id = domain::client_id(client_id)?;
                sqlx::query_as::<_, MatterRow>(
                    "SELECT matter_id, client_id, reference, title, status, practice_area,
                            opened_on, closed_on, created_at, updated_at
                     FROM matters.matters WHERE client_id = $1
                     ORDER BY reference_key",
                )
                .bind(client_id)
                .fetch_all(tx.conn())
                .await?
            }
            None => {
                sqlx::query_as::<_, MatterRow>(
                    "SELECT matter_id, client_id, reference, title, status, practice_area,
                            opened_on, closed_on, created_at, updated_at
                     FROM matters.matters ORDER BY reference_key",
                )
                .fetch_all(tx.conn())
                .await?
            }
        };
        tx.commit().await?;
        rows.into_iter().map(Matter::try_from).collect()
    }

    /// Create a matter. A duplicate `matterId` or reference is a conflict.
    pub async fn create_matter(&self, actor: Actor, new: NewMatter) -> Result<Matter, StoreError> {
        let valid = validate_matter(&new)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        ensure_client(tx.conn(), &valid.client_id).await?;
        let row = insert_matter(&mut tx, &valid).await?;
        tx.commit().await?;
        Matter::try_from(row)
    }

    // ---------- aliases ----------

    /// List aliases, optionally narrowed to one kind and exact canonical value.
    pub async fn list_aliases(
        &self,
        actor: Actor,
        kind: Option<AliasKind>,
        value: Option<&str>,
    ) -> Result<Vec<Alias>, StoreError> {
        let value = match (kind, value) {
            (Some(kind), Some(raw)) => Some(domain::alias_value(kind, raw)?),
            (None, Some(_)) => return Err(StoreError::Invalid("kind is required when value is given")),
            (_, None) => None,
        };
        let mut tx = self.pool.begin(actor.tenant).await?;
        let rows = sqlx::query_as::<_, AliasRow>(
            "SELECT id, kind, value, client_id, matter_id FROM matters.aliases
             WHERE ($1::text IS NULL OR kind = $1)
               AND ($2::text IS NULL OR value = $2)
             ORDER BY kind, value",
        )
        .bind(kind.map(AliasKind::as_str))
        .bind(value)
        .fetch_all(tx.conn())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(Alias::try_from).collect()
    }

    /// Add an alias. The same value for the same target is idempotent (`created = false`); the same
    /// value for another target is a conflict.
    pub async fn create_alias(&self, actor: Actor, new: NewAlias) -> Result<(Alias, bool), StoreError> {
        let valid = validate_alias(&new)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        let (row, created) = upsert_alias(&mut tx, &valid).await?;
        tx.commit().await?;
        Ok((Alias::try_from(row)?, created))
    }

    // ---------- rate cards ----------

    /// List every rate card in the firm.
    pub async fn list_rate_cards(&self, actor: Actor) -> Result<Vec<RateCard>, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        let rows = sqlx::query_as::<_, RateCardRow>(
            "SELECT matter_id, hourly_rate_cents, unit_minutes, rounding, minimum_units, updated_at
             FROM matters.rate_cards ORDER BY matter_id",
        )
        .fetch_all(tx.conn())
        .await?;
        tx.commit().await?;
        rows.into_iter().map(RateCard::try_from).collect()
    }

    /// Create or replace a matter's rate card. A missing matter is `NotFound`.
    pub async fn put_rate_card(&self, actor: Actor, new: NewRateCard) -> Result<RateCard, StoreError> {
        let valid = validate_rate_card(&new)?;
        let mut tx = self.pool.begin(actor.tenant).await?;
        let row = upsert_rate_card(&mut tx, &valid).await?;
        tx.commit().await?;
        RateCard::try_from(row)
    }

    // ---------- import ----------

    /// Import clients in one transaction. Every row is an upsert by `clientId`.
    pub async fn import_clients(
        &self,
        actor: Actor,
        rows: &[(usize, NewClient)],
    ) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        for (row, new) in rows {
            let valid = at_row(*row, validate_client(new))?;
            upsert_client(&mut tx, &valid)
                .await
                .map_err(|e| row_error(*row, e))?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }

    /// Import matters in one transaction. Every row is an upsert by `matterId`; a reference may not
    /// move to another matter.
    pub async fn import_matters(
        &self,
        actor: Actor,
        rows: &[(usize, NewMatter)],
    ) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        for (row, new) in rows {
            let valid = at_row(*row, validate_matter(new))?;
            ensure_client(tx.conn(), &valid.client_id)
                .await
                .map_err(|e| row_error(*row, e))?;
            upsert_matter(&mut tx, &valid)
                .await
                .map_err(|e| row_error(*row, e))?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }

    /// Import aliases in one transaction. A value already naming another target fails its row.
    pub async fn import_aliases(
        &self,
        actor: Actor,
        rows: &[(usize, NewAlias)],
    ) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        for (row, new) in rows {
            let valid = at_row(*row, validate_alias(new))?;
            upsert_alias(&mut tx, &valid)
                .await
                .map_err(|e| row_error(*row, e))?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }

    /// Import rate cards in one transaction. Every row replaces the matter's card.
    pub async fn import_rate_cards(
        &self,
        actor: Actor,
        rows: &[(usize, NewRateCard)],
    ) -> Result<usize, StoreError> {
        let mut tx = self.pool.begin(actor.tenant).await?;
        for (row, new) in rows {
            let valid = at_row(*row, validate_rate_card(new))?;
            upsert_rate_card(&mut tx, &valid)
                .await
                .map_err(|e| row_error(*row, e))?;
        }
        tx.commit().await?;
        Ok(rows.len())
    }
}

fn at_row<T>(row: usize, result: Result<T, StoreError>) -> Result<T, StoreError> {
    result.map_err(|error| row_error(row, error))
}

fn row_error(row: usize, error: StoreError) -> StoreError {
    match error {
        StoreError::Invalid(message) | StoreError::Conflict(message) => StoreError::Row { row, message },
        StoreError::NotFound => StoreError::Row {
            row,
            message: "a referenced client or matter does not exist",
        },
        other => other,
    }
}

async fn ensure_client(conn: &mut sqlx::PgConnection, client_id: &str) -> Result<(), StoreError> {
    let found: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM matters.clients WHERE client_id = $1")
        .bind(client_id)
        .fetch_optional(conn)
        .await?;
    found
        .map(|_| ())
        .ok_or(StoreError::Invalid("clientId does not exist"))
}

async fn insert_client(tx: &mut TenantTx, valid: &ValidClient) -> Result<ClientRow, StoreError> {
    let row = sqlx::query_as::<_, ClientRow>(
        "INSERT INTO matters.clients (tenant_id, client_id, name, status)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (tenant_id, client_id) DO NOTHING
         RETURNING client_id, name, status, created_at, updated_at",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&valid.client_id)
    .bind(&valid.name)
    .bind(valid.status.as_str())
    .fetch_optional(tx.conn())
    .await?;
    row.ok_or(StoreError::Conflict("a client with this clientId already exists"))
}

async fn upsert_client(tx: &mut TenantTx, valid: &ValidClient) -> Result<ClientRow, StoreError> {
    sqlx::query_as::<_, ClientRow>(
        "INSERT INTO matters.clients (tenant_id, client_id, name, status)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (tenant_id, client_id) DO UPDATE
           SET name = EXCLUDED.name, status = EXCLUDED.status, updated_at = now()
         RETURNING client_id, name, status, created_at, updated_at",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&valid.client_id)
    .bind(&valid.name)
    .bind(valid.status.as_str())
    .fetch_one(tx.conn())
    .await
    .map_err(StoreError::from)
}

async fn insert_matter(tx: &mut TenantTx, valid: &ValidMatter) -> Result<MatterRow, StoreError> {
    let inserted = sqlx::query_as::<_, MatterRow>(
        "INSERT INTO matters.matters
           (tenant_id, matter_id, client_id, reference, reference_key, title, status,
            practice_area, opened_on, closed_on)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (tenant_id, matter_id) DO NOTHING
         RETURNING matter_id, client_id, reference, title, status, practice_area,
                   opened_on, closed_on, created_at, updated_at",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&valid.matter_id)
    .bind(&valid.client_id)
    .bind(&valid.reference)
    .bind(&valid.reference_key)
    .bind(&valid.title)
    .bind(valid.status.as_str())
    .bind(&valid.practice_area)
    .bind(valid.opened_on)
    .bind(valid.closed_on)
    .fetch_optional(tx.conn())
    .await
    .map_err(reference_conflict)?;
    inserted.ok_or(StoreError::Conflict("a matter with this matterId already exists"))
}

async fn upsert_matter(tx: &mut TenantTx, valid: &ValidMatter) -> Result<MatterRow, StoreError> {
    sqlx::query_as::<_, MatterRow>(
        "INSERT INTO matters.matters
           (tenant_id, matter_id, client_id, reference, reference_key, title, status,
            practice_area, opened_on, closed_on)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (tenant_id, matter_id) DO UPDATE
           SET client_id = EXCLUDED.client_id, reference = EXCLUDED.reference,
               reference_key = EXCLUDED.reference_key, title = EXCLUDED.title,
               status = EXCLUDED.status, practice_area = EXCLUDED.practice_area,
               opened_on = EXCLUDED.opened_on, closed_on = EXCLUDED.closed_on,
               updated_at = now()
         RETURNING matter_id, client_id, reference, title, status, practice_area,
                   opened_on, closed_on, created_at, updated_at",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&valid.matter_id)
    .bind(&valid.client_id)
    .bind(&valid.reference)
    .bind(&valid.reference_key)
    .bind(&valid.title)
    .bind(valid.status.as_str())
    .bind(&valid.practice_area)
    .bind(valid.opened_on)
    .bind(valid.closed_on)
    .fetch_one(tx.conn())
    .await
    .map_err(reference_conflict)
}

async fn upsert_alias(tx: &mut TenantTx, valid: &ValidAlias) -> Result<(AliasRow, bool), StoreError> {
    let existing = sqlx::query_as::<_, AliasRow>(
        "SELECT id, kind, value, client_id, matter_id FROM matters.aliases
         WHERE kind = $1 AND value = $2",
    )
    .bind(valid.kind.as_str())
    .bind(&valid.value)
    .fetch_optional(tx.conn())
    .await?;
    if let Some(row) = existing {
        let same_target = row.client_id == valid.client_id && row.matter_id == valid.matter_id;
        return if same_target {
            Ok((row, false))
        } else {
            Err(StoreError::Conflict(
                "this value already names another client or matter",
            ))
        };
    }

    let client_found: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM matters.clients WHERE client_id = $1")
        .bind(&valid.client_id)
        .fetch_optional(tx.conn())
        .await?;
    if client_found.is_none() {
        return Err(StoreError::Invalid("clientId does not exist"));
    }
    if let Some(matter_id) = &valid.matter_id {
        let matter_found: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM matters.matters WHERE matter_id = $1 AND client_id = $2")
                .bind(matter_id)
                .bind(&valid.client_id)
                .fetch_optional(tx.conn())
                .await?;
        if matter_found.is_none() {
            return Err(StoreError::Invalid("matterId does not exist for this clientId"));
        }
    }

    let inserted = sqlx::query_as::<_, AliasRow>(
        "INSERT INTO matters.aliases (tenant_id, id, kind, value, client_id, matter_id)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (tenant_id, kind, value) DO NOTHING
         RETURNING id, kind, value, client_id, matter_id",
    )
    .bind(tx.tenant().as_uuid())
    .bind(Uuid::new_v4())
    .bind(valid.kind.as_str())
    .bind(&valid.value)
    .bind(&valid.client_id)
    .bind(&valid.matter_id)
    .fetch_optional(tx.conn())
    .await?;
    inserted.map(|row| (row, true)).ok_or(StoreError::Conflict(
        "this value already names another client or matter",
    ))
}

async fn upsert_rate_card(tx: &mut TenantTx, valid: &ValidRateCard) -> Result<RateCardRow, StoreError> {
    let matter_found: Option<(i32,)> = sqlx::query_as("SELECT 1 FROM matters.matters WHERE matter_id = $1")
        .bind(&valid.matter_id)
        .fetch_optional(tx.conn())
        .await?;
    if matter_found.is_none() {
        return Err(StoreError::NotFound);
    }
    let unit_minutes = i16::try_from(valid.unit_minutes)
        .map_err(|_| StoreError::corrupt("unitMinutes does not fit a smallint"))?;
    let minimum_units = i16::try_from(valid.minimum_units)
        .map_err(|_| StoreError::corrupt("minimumUnits does not fit a smallint"))?;
    Ok(sqlx::query_as::<_, RateCardRow>(
        "INSERT INTO matters.rate_cards
           (tenant_id, matter_id, hourly_rate_cents, unit_minutes, rounding, minimum_units)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (tenant_id, matter_id) DO UPDATE
           SET hourly_rate_cents = EXCLUDED.hourly_rate_cents,
               unit_minutes = EXCLUDED.unit_minutes,
               rounding = EXCLUDED.rounding,
               minimum_units = EXCLUDED.minimum_units,
               updated_at = now()
         RETURNING matter_id, hourly_rate_cents, unit_minutes, rounding, minimum_units, updated_at",
    )
    .bind(tx.tenant().as_uuid())
    .bind(&valid.matter_id)
    .bind(valid.hourly_rate_cents)
    .bind(unit_minutes)
    .bind(valid.rounding.as_str())
    .bind(minimum_units)
    .fetch_one(tx.conn())
    .await?)
}

fn reference_conflict(error: sqlx::Error) -> StoreError {
    if is_unique_violation(&error) {
        StoreError::Conflict("a matter with this reference already exists")
    } else {
        StoreError::from(error)
    }
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
}
