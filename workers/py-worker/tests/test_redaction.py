# Written by Kone Tshivhinda & Claude
import pytest

from timelex_worker.redaction import luhn_valid, redact

VALID_SA_ID = "8001015009087"  # synthetic, Luhn-valid, 1 Jan 1980
VISA_TEST = "4111111111111111"  # public test card number


def test_sa_id_is_redacted():
    r = redact(f"ID number {VALID_SA_ID} on file")
    assert r.text == "ID number [SA_ID] on file"
    assert r.counts == {"sa_id": 1}


def test_thirteen_digits_that_are_not_an_id_are_left_alone():
    # Fails Luhn.
    assert redact("ref 8001015009088").text == "ref 8001015009088"
    # Impossible month 13.
    assert redact("ref 8013015009087").counts == {}


def test_card_number_with_spaces_and_dashes():
    r = redact("Card 4111 1111 1111 1111 and 4111-1111-1111-1111.")
    assert r.text == "Card [CARD] and [CARD]."
    assert r.counts == {"card": 2}


def test_card_failing_luhn_is_left_alone():
    assert redact("order 4111111111111112").counts == {}


def test_email_redacted():
    r = redact("Write to fee.earner@example-firm.test today")
    assert r.text == "Write to [EMAIL] today"


@pytest.mark.parametrize(
    "phone",
    [
        "082 123 4567",
        "0821234567",
        "+27 82 123 4567",
        "+27821234567",
        "0027 82 123 4567",
        "011 555 1234",
    ],
)
def test_south_african_phone_numbers(phone):
    r = redact(f"Call {phone} now")
    assert r.text == "Call [PHONE] now"
    assert r.counts == {"phone": 1}


def test_short_numbers_are_not_phones():
    assert redact("Invoice 12345 for R2 400").counts == {}


def test_labelled_bank_account_numbers():
    r = redact("Account number: 1234 5678 901, branch 250655")
    assert r.text == "Account number: [ACCOUNT], branch 250655"
    assert r.counts == {"account": 1}
    r2 = redact("Acc no 62012345678")
    assert r2.text == "Acc no [ACCOUNT]"


def test_unlabelled_long_numbers_are_not_account_numbers():
    assert redact("Matter 20260001234 opened").counts == {}


def test_mixed_text_counts_each_kind():
    text = f"Client {VALID_SA_ID}, mail a@b.co.za, tel 082 123 4567, card {VISA_TEST}"
    r = redact(text)
    assert r.counts == {"sa_id": 1, "email": 1, "phone": 1, "card": 1}
    assert VALID_SA_ID not in r.text
    assert VISA_TEST not in r.text
    assert "082" not in r.text
    assert "a@b.co.za" not in r.text


def test_sa_id_wins_over_card_when_both_match():
    # A Luhn-valid 13-digit SA ID also looks like a 13-digit card; ID takes priority.
    r = redact(VALID_SA_ID)
    assert r.text == "[SA_ID]"
    assert r.counts == {"sa_id": 1}


def test_counts_never_contain_values():
    r = redact(f"{VALID_SA_ID} a@b.co.za")
    assert all(isinstance(v, int) for v in r.counts.values())
    assert VALID_SA_ID not in repr(r.counts)


def test_empty_and_clean_text_unchanged():
    assert redact("").text == ""
    clean = "Telephone conference regarding the draft settlement agreement (0.3 h)."
    r = redact(clean)
    assert r.text == clean
    assert r.counts == {}


def test_luhn_helper():
    assert luhn_valid(VISA_TEST)
    assert not luhn_valid("4111111111111112")
    assert not luhn_valid("abc")
