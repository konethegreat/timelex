// Written by Kone Tshivhinda & Claude
//! The CSV import reader: headers, row numbering and row-level refusals (ADR-0027). No database.

use timelex_matters_service::import::{self, ImportKind, Rows};

fn clients(csv: &str) -> Rows {
    import::parse(ImportKind::Clients, csv).expect("valid clients CSV")
}

#[test]
fn columns_may_come_in_any_order_and_headers_are_case_insensitive() {
    let Rows::Clients(rows) = clients(
        "Name,CLIENTID,Status\n\
         Meridian Logistics,cl_92c352e5b3f9b6fc,\n\
         Kalahari Mining,cl_4d9599f93023d376,inactive\n",
    ) else {
        panic!("wrong import kind");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, 2, "first data row is row 2");
    assert_eq!(rows[0].1.client_id, "cl_92c352e5b3f9b6fc");
    assert_eq!(rows[0].1.name, "Meridian Logistics");
    assert_eq!(rows[0].1.status, None);
    assert_eq!(rows[1].1.status.as_deref(), Some("inactive"));
}

#[test]
fn an_unknown_column_is_refused_rather_than_ignored() {
    let error = import::parse(ImportKind::Clients, "clientId,name,tenantId\na,A,b\n").unwrap_err();
    assert_eq!(error.row, Some(1));
    assert!(error.message.contains("unknown column"), "{}", error.message);
    assert!(error.message.contains("tenantid"), "{}", error.message);
}

#[test]
fn a_missing_required_column_names_what_is_missing() {
    let error = import::parse(ImportKind::Matters, "matterId,clientId,title\nmt_1,cl_1,x\n").unwrap_err();
    assert_eq!(error.row, Some(1));
    assert!(error.message.contains("reference"), "{}", error.message);
}

#[test]
fn a_repeated_column_is_refused() {
    let error = import::parse(ImportKind::Clients, "clientId,ClientId,name\na,b,c\n").unwrap_err();
    assert_eq!(error.row, Some(1));
    assert!(error.message.contains("twice"), "{}", error.message);
}

#[test]
fn a_row_missing_a_required_value_names_its_row_and_field() {
    let error = import::parse(ImportKind::Clients, "clientId,name\ncl_1,\n").unwrap_err();
    assert_eq!(error.row, Some(2));
    assert_eq!(error.message, "name is required");
}

#[test]
fn a_short_row_is_refused() {
    let error = import::parse(ImportKind::Clients, "clientId,name\ncl_1\n").unwrap_err();
    assert_eq!(error.row, Some(2));
    assert!(error.message.contains("fewer values"), "{}", error.message);
}

#[test]
fn blank_lines_between_rows_are_ignored() {
    let Rows::Clients(rows) = clients("clientId,name\ncl_1,A\n\ncl_2,B\n") else {
        panic!("wrong import kind");
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].0, 3, "blank lines are not counted as rows");
}

#[test]
fn a_rate_card_number_must_be_whole() {
    let error = import::parse(ImportKind::RateCards, "matterId,hourlyRateCents\nmt_1,2800.00\n").unwrap_err();
    assert_eq!(error.row, Some(2));
    assert_eq!(error.message, "hourlyRateCents must be a whole number of cents");
}

#[test]
fn optional_numbers_fall_back_to_the_contract_defaults() {
    let Rows::RateCards(rows) =
        import::parse(ImportKind::RateCards, "matterId,hourlyRateCents\nmt_1,280000\n").unwrap()
    else {
        panic!("wrong import kind");
    };
    let card = &rows[0].1;
    assert_eq!(card.hourly_rate_cents, 280_000);
    assert_eq!(card.unit_minutes, None);
    assert_eq!(card.rounding, None);
    assert_eq!(card.minimum_units, None);
}

#[test]
fn an_import_is_limited_to_five_thousand_rows() {
    let mut csv = String::from("clientId,name\n");
    for i in 1..=5001 {
        csv.push_str(&format!("cl_{i},Client {i}\n"));
    }
    let error = import::parse(ImportKind::Clients, &csv).unwrap_err();
    assert_eq!(error.row, Some(5002));
    assert!(error.message.contains("5000"), "{}", error.message);
}

#[test]
fn an_empty_import_is_refused() {
    let error = import::parse(ImportKind::Aliases, "").unwrap_err();
    assert_eq!(error.message, "the CSV has no header row");
}

#[test]
fn alias_rows_keep_the_optional_matter() {
    let Rows::Aliases(rows) = import::parse(
        ImportKind::Aliases,
        "kind,value,clientId,matterId\n\
         client_domain,Meridian.Example,cl_1,\n\
         matter_reference,M-2026-0101,cl_1,mt_1\n",
    )
    .unwrap() else {
        panic!("wrong import kind");
    };
    assert_eq!(rows[0].1.matter_id, None);
    assert_eq!(rows[0].1.kind, "client_domain");
    assert_eq!(rows[1].1.matter_id.as_deref(), Some("mt_1"));
}

#[test]
fn an_unknown_import_kind_is_named_in_the_refusal() {
    let error = ImportKind::parse("contacts").unwrap_err();
    assert!(error.contains("clients"), "{error}");
}
