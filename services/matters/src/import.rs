// Written by Kone Tshivhinda & Claude
//! CSV import: one record type per file, all-or-nothing (ADR-0027).
//!
//! This module turns bytes into rows and refuses anything it does not understand: an unknown column,
//! a missing required column, a short row, too many rows. It does not touch the database and it does
//! not validate the business rules; [`crate::store`] does that, so a single-row request and a CSV row
//! pass exactly the same checks. A refused row names its number and never echoes the value.

use std::collections::BTreeMap;

use crate::store::{NewAlias, NewClient, NewMatter, NewRateCard};

/// Most data rows one import may hold. An import is applied in one transaction, so this is also the
/// bound on how long a firm's other writes can wait behind it.
pub const MAX_IMPORT_ROWS: usize = 5000;

/// Which record a CSV file holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    /// `POST /v1/import/clients`
    Clients,
    /// `POST /v1/import/matters`
    Matters,
    /// `POST /v1/import/aliases`
    Aliases,
    /// `POST /v1/import/rate-cards`
    RateCards,
}

impl ImportKind {
    /// Parse the last path segment.
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "clients" => Ok(Self::Clients),
            "matters" => Ok(Self::Matters),
            "aliases" => Ok(Self::Aliases),
            "rate-cards" => Ok(Self::RateCards),
            _ => Err("kind must be clients, matters, aliases or rate-cards".into()),
        }
    }

    /// The path segment.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clients => "clients",
            Self::Matters => "matters",
            Self::Aliases => "aliases",
            Self::RateCards => "rate-cards",
        }
    }

    fn columns(self) -> (&'static [&'static str], &'static [&'static str]) {
        match self {
            Self::Clients => (&["clientid", "name"], &["status"]),
            Self::Matters => (
                &["matterid", "clientid", "reference", "title"],
                &["status", "practicearea", "openedon", "closedon"],
            ),
            Self::Aliases => (&["kind", "value", "clientid"], &["matterid"]),
            Self::RateCards => (
                &["matterid", "hourlyratecents"],
                &["unitminutes", "rounding", "minimumunits"],
            ),
        }
    }
}

/// Parsed rows, already numbered. The number is one-based and counts the header as row 1.
#[derive(Debug)]
pub enum Rows {
    /// Rows for `clients`.
    Clients(Vec<(usize, NewClient)>),
    /// Rows for `matters`.
    Matters(Vec<(usize, NewMatter)>),
    /// Rows for `aliases`.
    Aliases(Vec<(usize, NewAlias)>),
    /// Rows for `rate-cards`.
    RateCards(Vec<(usize, NewRateCard)>),
}

impl Rows {
    /// The import kind these rows belong to.
    pub fn kind(&self) -> ImportKind {
        match self {
            Self::Clients(_) => ImportKind::Clients,
            Self::Matters(_) => ImportKind::Matters,
            Self::Aliases(_) => ImportKind::Aliases,
            Self::RateCards(_) => ImportKind::RateCards,
        }
    }

    /// How many data rows were read.
    pub fn len(&self) -> usize {
        match self {
            Self::Clients(rows) => rows.len(),
            Self::Matters(rows) => rows.len(),
            Self::Aliases(rows) => rows.len(),
            Self::RateCards(rows) => rows.len(),
        }
    }

    /// True when the file held a header but no data.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A refused import: the row (if it has one) and a fixed explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportError {
    /// One-based row number, or `None` when the file as a whole is unusable.
    pub row: Option<usize>,
    /// Safe to show to the caller.
    pub message: String,
}

impl ImportError {
    fn at(row: usize, message: impl Into<String>) -> Self {
        Self {
            row: Some(row),
            message: message.into(),
        }
    }

    fn whole(message: impl Into<String>) -> Self {
        Self {
            row: None,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.row {
            Some(row) => write!(f, "row {row}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for ImportError {}

struct Columns {
    index: BTreeMap<String, usize>,
    count: usize,
}

impl Columns {
    fn read(kind: ImportKind, headers: &csv::StringRecord) -> Result<Self, ImportError> {
        let (required, optional) = kind.columns();
        let mut index = BTreeMap::new();
        let mut count = 0;
        for header in headers {
            count += 1;
            let name = normalise(header);
            if index.contains_key(&name) {
                return Err(ImportError::at(
                    1,
                    format!("column {} is named twice", quoted(&name)),
                ));
            }
            if !required.contains(&name.as_str()) && !optional.contains(&name.as_str()) {
                let allowed = required
                    .iter()
                    .chain(optional)
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(ImportError::at(
                    1,
                    format!(
                        "unknown column {} in the header row; allowed: {allowed}",
                        quoted(&name)
                    ),
                ));
            }
            index.insert(name, count - 1);
        }
        for name in required {
            if !index.contains_key(*name) {
                return Err(ImportError::at(
                    1,
                    format!("the header row is missing the required column {}", quoted(name)),
                ));
            }
        }
        Ok(Self { index, count })
    }

    fn required(
        &self,
        record: &csv::StringRecord,
        name: &'static str,
        row: usize,
    ) -> Result<String, ImportError> {
        let index = self.index[name];
        let value = record.get(index).unwrap_or("").trim();
        if value.is_empty() {
            return Err(ImportError::at(row, format!("{} is required", label(name))));
        }
        Ok(value.to_owned())
    }

    fn optional(&self, record: &csv::StringRecord, name: &'static str) -> Option<String> {
        let index = *self.index.get(name)?;
        let value = record.get(index).unwrap_or("").trim();
        (!value.is_empty()).then(|| value.to_owned())
    }

    /// A required whole number in cents.
    fn cents(&self, record: &csv::StringRecord, name: &'static str, row: usize) -> Result<i64, ImportError> {
        let value = self.required(record, name, row)?;
        value
            .parse::<i64>()
            .map_err(|_| ImportError::at(row, format!("{} must be a whole number of cents", label(name))))
    }

    /// An optional whole number.
    fn whole_number(
        &self,
        record: &csv::StringRecord,
        name: &'static str,
        row: usize,
    ) -> Result<Option<i64>, ImportError> {
        match self.optional(record, name) {
            None => Ok(None),
            Some(value) => value
                .parse::<i64>()
                .map(Some)
                .map_err(|_| ImportError::at(row, format!("{} must be a whole number", label(name)))),
        }
    }
}

fn normalise(header: &str) -> String {
    header.trim().to_lowercase()
}

/// The spelling a caller recognises, used in row errors. Headers are matched case-insensitively;
/// messages still name the field the way the contract does.
fn label(name: &str) -> &str {
    match name {
        "clientid" => "clientId",
        "matterid" => "matterId",
        "practicearea" => "practiceArea",
        "openedon" => "openedOn",
        "closedon" => "closedOn",
        "hourlyratecents" => "hourlyRateCents",
        "unitminutes" => "unitMinutes",
        "minimumunits" => "minimumUnits",
        other => other,
    }
}

/// A short, printable rendering of a header for the error message. Control characters and runs of
/// unprintable bytes are replaced, so an import cannot put arbitrary text into a response.
fn quoted(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .take(40)
        .map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '?' })
        .collect();
    format!("\"{cleaned}\"")
}

/// Read a CSV body for `kind`.
pub fn parse(kind: ImportKind, text: &str) -> Result<Rows, ImportError> {
    if text.trim().is_empty() {
        return Err(ImportError::whole("the CSV has no header row"));
    }
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .map_err(|_| ImportError::whole("the CSV header row cannot be read"))?
        .clone();
    if headers.is_empty() {
        return Err(ImportError::whole("the CSV has no header row"));
    }
    let columns = Columns::read(kind, &headers)?;

    let mut clients = Vec::new();
    let mut matters = Vec::new();
    let mut aliases = Vec::new();
    let mut cards = Vec::new();
    let mut data_rows = 0;
    for record in reader.records() {
        let record = record.map_err(|_| {
            // Blank lines are not counted, so this is the logical row the file's reader was on.
            ImportError::at(data_rows + 2, "the row cannot be read as CSV")
        })?;
        if record.iter().all(|value| value.trim().is_empty()) {
            continue;
        }
        data_rows += 1;
        let row = data_rows + 1;
        if data_rows > MAX_IMPORT_ROWS {
            return Err(ImportError::at(
                row,
                format!("an import is limited to {MAX_IMPORT_ROWS} rows"),
            ));
        }
        if record.len() != columns.count {
            return Err(ImportError::at(
                row,
                if record.len() < columns.count {
                    "the row has fewer values than the header"
                } else {
                    "the row has more values than the header"
                },
            ));
        }
        match kind {
            ImportKind::Clients => clients.push((
                row,
                NewClient {
                    client_id: columns.required(&record, "clientid", row)?,
                    name: columns.required(&record, "name", row)?,
                    status: columns.optional(&record, "status"),
                },
            )),
            ImportKind::Matters => matters.push((
                row,
                NewMatter {
                    matter_id: columns.required(&record, "matterid", row)?,
                    client_id: columns.required(&record, "clientid", row)?,
                    reference: columns.required(&record, "reference", row)?,
                    title: columns.required(&record, "title", row)?,
                    status: columns.optional(&record, "status"),
                    practice_area: columns.optional(&record, "practicearea"),
                    opened_on: columns.optional(&record, "openedon"),
                    closed_on: columns.optional(&record, "closedon"),
                },
            )),
            ImportKind::Aliases => aliases.push((
                row,
                NewAlias {
                    kind: columns.required(&record, "kind", row)?,
                    value: columns.required(&record, "value", row)?,
                    client_id: columns.required(&record, "clientid", row)?,
                    matter_id: columns.optional(&record, "matterid"),
                },
            )),
            ImportKind::RateCards => cards.push((
                row,
                NewRateCard {
                    matter_id: columns.required(&record, "matterid", row)?,
                    hourly_rate_cents: columns.cents(&record, "hourlyratecents", row)?,
                    unit_minutes: columns.whole_number(&record, "unitminutes", row)?,
                    rounding: columns.optional(&record, "rounding"),
                    minimum_units: columns.whole_number(&record, "minimumunits", row)?,
                },
            )),
        }
    }

    Ok(match kind {
        ImportKind::Clients => Rows::Clients(clients),
        ImportKind::Matters => Rows::Matters(matters),
        ImportKind::Aliases => Rows::Aliases(aliases),
        ImportKind::RateCards => Rows::RateCards(cards),
    })
}
