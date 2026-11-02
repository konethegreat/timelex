# Written by Kone Tshivhinda & Claude
"""Generate a labelled, synthetic law firm from one seed.

Everything here is fiction with reserved domains (`.example` and `.test`). The generator makes the
same bytes for the same seed; `check_corpus` in `emit.py` proves the committed corpus still matches.
"""

from __future__ import annotations

import hashlib
import random
from datetime import UTC, date, datetime, timedelta
from typing import Any

from .ids import activity_event_id
from .model import Client, Corpus, Exclusion, Firm, Matter, User

DEFAULT_SEED = 20261102

_FIRM_NAME = "Bergman & Dlamini Attorneys"
_FIRM_DOMAIN = "bergman-dlamini.example"
_NEWSLETTER_DOMAIN = "briefing.example"
_START_DAY = date(2026, 9, 7)
_BUSINESS_DAY_COUNT = 20

_USERS = (
    ("Adri Bergman", "a.bergman", "partner"),
    ("Thabo Dlamini", "t.dlamini", "partner"),
    ("Naledi Khumalo", "n.khumalo", "associate"),
    ("Ravi Pillay", "r.pillay", "associate"),
    ("Sipho Mokoena", "s.mokoena", "candidate"),
    ("Lize van Wyk", "l.vanwyk", "paralegal"),
)

_CLIENTS = (
    ("Meridian Logistics", "meridian.example"),
    ("Kalahari Mining", "kalahari.example"),
    ("Cape Point Retail", "capepoint.example"),
    ("Highveld Energy", "highveld.example"),
    ("Durban Marine", "durbanmarine.example"),
    ("Karoo Agri", "karoo-agri.example"),
    ("Sandton Fintech", "sandtonfintech.example"),
    ("Pretoria Health", "pretoriahealth.example"),
)

_MATTER_TITLES = (
    "Shareholder agreement review",
    "Commercial lease renewal",
    "Mining right transfer",
    "Employment contract review",
    "Data protection audit",
    "Trademark opposition",
    "Property transfer",
    "Debt recovery",
    "B-BBEE verification",
    "Supplier agreement",
    "Construction dispute",
    "Loan facility review",
)

_PRACTICE_AREAS = ("Corporate", "Litigation", "Property", "Employment", "Commercial", "Regulatory")
_RATE_CENTS = (280000, 195000, 125000)
_EXCLUDED_DOMAINS = (
    ("adverse.example.test", "Opposing party in active litigation"),
    ("personal.example.test", "Personal mail provider excluded by firm policy"),
)
_CONTACT_LOCALS = ("legal", "ops", "finance", "admin", "instructions")
_GENERIC_SUBJECTS = (
    "Documents for review",
    "Following up",
    "Draft for comment",
    "Meeting notes",
    "Next steps",
)
_INTERNAL_SUBJECTS = (
    "Weekly practice meeting",
    "Fees and billing review",
    "New matter intake",
    "Office move update",
)
_EXCLUDED_SUBJECTS = ("Settlement proposal", "Personal note", "Weekend arrangements")
_MEETING_MINUTES = (30, 45, 60, 90)


def _business_days(start: date, count: int) -> tuple[str, ...]:
    days: list[str] = []
    cursor = start
    while len(days) < count:
        if cursor.weekday() < 5:
            days.append(cursor.isoformat())
        cursor += timedelta(days=1)
    return tuple(days)


def _at(day: str, hour: int, minute: int) -> datetime:
    return datetime.fromisoformat(f"{day}T{hour:02d}:{minute:02d}:00+00:00")


def _iso(moment: datetime) -> str:
    return moment.astimezone(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")


def _participant(role: str, address: str) -> dict[str, str]:
    return {"role": role, "domain": address.rsplit("@", 1)[1], "address": address}


class _Ids:
    """Counter-based ids: deterministic, unique, and independent of the RNG's call order."""

    def __init__(self, seed: int) -> None:
        self._seed = seed
        self._counter = 0

    def next(self, prefix: str) -> str:
        self._counter += 1
        digest = hashlib.sha256(f"{self._seed}:{prefix}:{self._counter}".encode()).hexdigest()
        return prefix + digest[:16]


class _Builder:
    def __init__(self, seed: int) -> None:
        self.seed = seed
        self.rng = random.Random(seed)
        self.ids = _Ids(seed)
        self.days = _business_days(_START_DAY, _BUSINESS_DAY_COUNT)
        self.firm = self._firm()
        self.by_client = {client.client_id: client for client in self.firm.clients}
        self.open_by_client = {
            client.client_id: sorted(
                matter.matter_id
                for matter in self.firm.matters
                if matter.client_id == client.client_id and matter.status == "open"
            )
            for client in self.firm.clients
        }
        self.threads: list[dict[str, Any]] = []
        self.calendar: list[dict[str, Any]] = []
        self._events: list[dict[str, Any]] = []
        self._labels: dict[str, dict[str, Any]] = {}

    def _firm(self) -> Firm:
        tenant_id = "tn_g" + hashlib.sha256(f"tenant:{self.seed}".encode()).hexdigest()[:16]
        users = tuple(
            User(self.ids.next("us_"), name, f"{local}@{_FIRM_DOMAIN}", role)
            for name, local, role in _USERS
        )
        clients = tuple(Client(self.ids.next("cl_"), name, domain) for name, domain in _CLIENTS)
        matters: list[Matter] = []
        for index, client in enumerate(clients):
            for slot in range(3):
                ordinal = index * 3 + slot
                matters.append(
                    Matter(
                        matter_id=self.ids.next("mt_"),
                        client_id=client.client_id,
                        reference=f"M-2026-{101 + ordinal:04d}",
                        title=_MATTER_TITLES[ordinal % len(_MATTER_TITLES)],
                        status="closed" if ordinal in (7, 19) else "open",
                        practice_area=_PRACTICE_AREAS[ordinal % len(_PRACTICE_AREAS)],
                        rate_cents_per_hour=_RATE_CENTS[ordinal % len(_RATE_CENTS)],
                    )
                )
        exclusions = tuple(
            Exclusion("domain", value, reason) for value, reason in _EXCLUDED_DOMAINS
        )
        return Firm(
            tenant_id=tenant_id,
            name=_FIRM_NAME,
            domain=_FIRM_DOMAIN,
            users=users,
            clients=clients,
            matters=tuple(matters),
            exclusions=exclusions,
            business_days=self.days,
        )

    def build(self) -> Corpus:
        self._mail()
        self._meetings()
        self._desktop()
        self._manual()
        events = sorted(self._events, key=lambda event: (event["startedAt"], event["id"]))
        labels = tuple(self._labels[event["id"]] for event in events)
        return Corpus(
            seed=self.seed,
            firm=self.firm,
            threads=tuple(self.threads),
            calendar=tuple(self.calendar),
            events=tuple(events),
            labels=labels,
        )

    def _record(
        self,
        *,
        system: str,
        source_id: str,
        kind: str,
        actor: User,
        start: datetime,
        minutes: int,
        label: dict[str, Any],
        title: str | None = None,
        thread_ref: str | None = None,
        document_ref: str | None = None,
        participants: list[dict[str, str]] | None = None,
    ) -> str:
        end = start + timedelta(minutes=minutes)
        captured = end + timedelta(seconds=self.rng.randint(30, 300))
        subject: dict[str, Any] = {}
        if title is not None:
            subject["title"] = title
        if thread_ref is not None:
            subject["threadRef"] = thread_ref
        if document_ref is not None:
            subject["documentRef"] = document_ref
        if participants is not None:
            subject["participants"] = participants
        event: dict[str, Any] = {
            "schemaVersion": "1.0",
            "tenantId": self.firm.tenant_id,
            "source": {"system": system, "sourceId": source_id},
            "kind": kind,
            "actor": {"userId": actor.user_id},
            "startedAt": _iso(start),
            "endedAt": _iso(end),
            "capturedAt": _iso(captured),
            "privacy": {"contentCaptured": False, "redactionApplied": False},
        }
        if subject:
            event["subject"] = subject
        event_id = activity_event_id(self.firm.tenant_id, system, source_id, start)
        event["id"] = event_id
        self._events.append(event)
        self._labels[event_id] = {"eventId": event_id, **label}
        return event_id

    def _client_label(self, matter: Matter, *, evidence: str) -> dict[str, Any]:
        client = self.by_client[matter.client_id]
        open_ids = self.open_by_client[matter.client_id]
        base: dict[str, Any] = {
            "expectedStorage": "stored",
            "clientId": client.client_id,
            "matterId": matter.matter_id,
            "billable": True,
            "evidence": evidence,
            "ambiguous": False,
        }
        if matter.status != "open":
            return {
                **base,
                "expectedOutcome": "review",
                "candidateMatterIds": [matter.matter_id],
            }
        if evidence == "hard_key":
            return {
                **base,
                "expectedOutcome": "auto_post",
                "candidateMatterIds": [matter.matter_id],
            }
        if len(open_ids) == 1:
            return {
                **base,
                "expectedOutcome": "auto_post",
                "candidateMatterIds": list(open_ids),
            }
        return {
            **base,
            "expectedOutcome": "review",
            "candidateMatterIds": list(open_ids),
            "ambiguous": True,
        }

    def _not_billable_client_label(self, matter: Matter) -> dict[str, Any]:
        client = self.by_client[matter.client_id]
        return {
            "expectedStorage": "stored",
            "expectedOutcome": "not_billable",
            "clientId": client.client_id,
            "matterId": matter.matter_id,
            "billable": False,
            "evidence": "hard_key",
            "candidateMatterIds": [matter.matter_id],
            "ambiguous": False,
        }

    @staticmethod
    def _unassigned_label() -> dict[str, Any]:
        return {
            "expectedStorage": "stored",
            "expectedOutcome": "not_billable",
            "clientId": None,
            "matterId": None,
            "billable": False,
            "evidence": "none",
            "candidateMatterIds": [],
            "ambiguous": False,
        }

    @staticmethod
    def _excluded_label() -> dict[str, Any]:
        return {
            "expectedStorage": "excluded",
            "expectedOutcome": "excluded",
            "clientId": None,
            "matterId": None,
            "billable": False,
            "evidence": "none",
            "candidateMatterIds": [],
            "ambiguous": False,
        }

    def _subject(self, pattern: str, matter: Matter, client: Client) -> str:
        if pattern == "hard_key":
            return f"[{matter.reference}] {matter.title}"
        if pattern == "client_name":
            return f"{client.name}: {matter.title.lower()}"
        return self.rng.choice(_GENERIC_SUBJECTS)

    def _mail(self) -> None:
        for matter in self.firm.matters:
            count = 1 if matter.status == "closed" else (2 if self.rng.random() < 0.65 else 1)
            for _ in range(count):
                if matter.status == "closed" or self.rng.random() < 0.55:
                    pattern = "hard_key"
                elif self.rng.random() < 0.5:
                    pattern = "client_name"
                else:
                    pattern = "domain_only"
                self._client_thread(matter, pattern)
        for _ in range(4):
            self._internal_thread()
        for domain, _reason in _EXCLUDED_DOMAINS:
            for _ in range(2):
                self._excluded_thread(domain)
        for _ in range(3):
            self._newsletter_thread()

    def _client_thread(self, matter: Matter, pattern: str) -> None:
        client = self.by_client[matter.client_id]
        subject = self._subject(pattern, matter, client)
        thread_ref = self.ids.next("th_")
        day = self.rng.choice(self.days)
        count = self.rng.randint(2, 4)
        contact = f"{self.rng.choice(_CONTACT_LOCALS)}@{client.domain}"
        owner = self.rng.choice(self.firm.users)
        moments = sorted(
            _at(day, self.rng.randint(8, 16), self.rng.randint(0, 59)) for _ in range(count)
        )
        label = self._client_label(
            matter, evidence="hard_key" if pattern == "hard_key" else "counterparty"
        )
        messages: list[dict[str, Any]] = []
        for index, moment in enumerate(moments):
            if index % 2 == 0:
                kind = "email_received"
                participants = [
                    _participant("from", contact),
                    _participant("to", owner.email),
                ]
            else:
                kind = "email_sent"
                participants = [
                    _participant("from", owner.email),
                    _participant("to", contact),
                ]
            if self.rng.random() < 0.3:
                copied = self.rng.choice(self.firm.users)
                if copied.user_id != owner.user_id:
                    participants.append(_participant("cc", copied.email))
            message_id = self.ids.next("msg_")
            messages.append(
                {
                    "messageId": message_id,
                    "from": participants[0]["address"],
                    "to": [participants[1]["address"]],
                    "cc": [person["address"] for person in participants[2:]],
                    "sentAt": _iso(moment),
                }
            )
            self._record(
                system="m365_mail",
                source_id=message_id,
                kind=kind,
                actor=owner,
                start=moment,
                minutes=self.rng.randint(3, 15),
                label=label,
                title=subject,
                thread_ref=thread_ref,
                participants=participants,
            )
        self.threads.append({"threadRef": thread_ref, "subject": subject, "messages": messages})

    def _internal_thread(self) -> None:
        subject = self.rng.choice(_INTERNAL_SUBJECTS)
        thread_ref = self.ids.next("th_")
        day = self.rng.choice(self.days)
        sender, receiver = self.rng.sample(self.firm.users, 2)
        moments = sorted(
            _at(day, self.rng.randint(8, 16), self.rng.randint(0, 59))
            for _ in range(self.rng.randint(2, 3))
        )
        label = self._unassigned_label()
        messages: list[dict[str, Any]] = []
        for index, moment in enumerate(moments):
            author, recipient = (sender, receiver) if index % 2 == 0 else (receiver, sender)
            participants = [
                _participant("from", author.email),
                _participant("to", recipient.email),
            ]
            message_id = self.ids.next("msg_")
            messages.append(
                {
                    "messageId": message_id,
                    "from": author.email,
                    "to": [recipient.email],
                    "cc": [],
                    "sentAt": _iso(moment),
                }
            )
            self._record(
                system="m365_mail",
                source_id=message_id,
                kind="email_sent",
                actor=author,
                start=moment,
                minutes=self.rng.randint(2, 10),
                label=label,
                title=subject,
                thread_ref=thread_ref,
                participants=participants,
            )
        self.threads.append({"threadRef": thread_ref, "subject": subject, "messages": messages})

    def _excluded_thread(self, domain: str) -> None:
        subject = self.rng.choice(_EXCLUDED_SUBJECTS)
        thread_ref = self.ids.next("th_")
        day = self.rng.choice(self.days)
        contact = f"{self.rng.choice(_CONTACT_LOCALS)}@{domain}"
        owner = self.rng.choice(self.firm.users)
        moments = sorted(
            _at(day, self.rng.randint(8, 16), self.rng.randint(0, 59))
            for _ in range(self.rng.randint(2, 3))
        )
        label = self._excluded_label()
        messages: list[dict[str, Any]] = []
        for index, moment in enumerate(moments):
            if index % 2 == 0:
                kind = "email_received"
                participants = [
                    _participant("from", contact),
                    _participant("to", owner.email),
                ]
            else:
                kind = "email_sent"
                participants = [
                    _participant("from", owner.email),
                    _participant("to", contact),
                ]
            message_id = self.ids.next("msg_")
            messages.append(
                {
                    "messageId": message_id,
                    "from": participants[0]["address"],
                    "to": [participants[1]["address"]],
                    "cc": [],
                    "sentAt": _iso(moment),
                }
            )
            self._record(
                system="m365_mail",
                source_id=message_id,
                kind=kind,
                actor=owner,
                start=moment,
                minutes=self.rng.randint(2, 8),
                label=label,
                title=subject,
                thread_ref=thread_ref,
                participants=participants,
            )
        self.threads.append({"threadRef": thread_ref, "subject": subject, "messages": messages})

    def _newsletter_thread(self) -> None:
        subject = "Regulatory briefing"
        thread_ref = self.ids.next("th_")
        day = self.rng.choice(self.days)
        contact = f"news@{_NEWSLETTER_DOMAIN}"
        owner = self.rng.choice(self.firm.users)
        moment = _at(day, self.rng.randint(8, 16), self.rng.randint(0, 59))
        message_id = self.ids.next("msg_")
        participants = [_participant("from", contact), _participant("to", owner.email)]
        messages = [
            {
                "messageId": message_id,
                "from": contact,
                "to": [owner.email],
                "cc": [],
                "sentAt": _iso(moment),
            }
        ]
        self._record(
            system="m365_mail",
            source_id=message_id,
            kind="email_received",
            actor=owner,
            start=moment,
            minutes=self.rng.randint(2, 6),
            label=self._unassigned_label(),
            title=subject,
            thread_ref=thread_ref,
            participants=participants,
        )
        self.threads.append({"threadRef": thread_ref, "subject": subject, "messages": messages})

    def _meetings(self) -> None:
        for matter in self.firm.matters:
            count = 1 if matter.status == "closed" else (2 if self.rng.random() < 0.45 else 1)
            for _ in range(count):
                if matter.status == "closed" or self.rng.random() < 0.5:
                    pattern = "hard_key"
                elif self.rng.random() < 0.6:
                    pattern = "client_name"
                else:
                    pattern = "domain_only"
                self._client_meeting(matter, pattern)
        for _ in range(2):
            self._internal_meeting()
        for domain, _reason in _EXCLUDED_DOMAINS:
            self._excluded_meeting(domain)

    def _client_meeting(self, matter: Matter, pattern: str) -> None:
        client = self.by_client[matter.client_id]
        subject = self._subject(pattern, matter, client)
        meeting_id = self.ids.next("mtg_")
        day = self.rng.choice(self.days)
        start = _at(day, self.rng.randint(9, 15), self.rng.choice((0, 15, 30, 45)))
        end = start + timedelta(minutes=self.rng.choice(_MEETING_MINUTES))
        contact = f"{self.rng.choice(_CONTACT_LOCALS)}@{client.domain}"
        owner = self.rng.choice(self.firm.users)
        attendees = [contact, owner.email]
        if self.rng.random() < 0.4:
            second = self.rng.choice(self.firm.users).email
            if second not in attendees:
                attendees.append(second)
        organizer = owner.email if self.rng.random() < 0.7 else contact
        participants = [_participant("organizer", organizer)] + [
            _participant("attendee", address) for address in attendees if address != organizer
        ]
        label = self._client_label(
            matter, evidence="hard_key" if pattern == "hard_key" else "counterparty"
        )
        self.calendar.append(
            {
                "meetingId": meeting_id,
                "subject": subject,
                "organizer": organizer,
                "attendees": attendees,
                "startsAt": _iso(start),
                "endsAt": _iso(end),
            }
        )
        self._record(
            system="m365_calendar",
            source_id=meeting_id,
            kind="meeting",
            actor=owner,
            start=start,
            minutes=int((end - start).total_seconds() // 60),
            label=label,
            title=subject,
            participants=participants,
        )

    def _internal_meeting(self) -> None:
        subject = self.rng.choice(_INTERNAL_SUBJECTS)
        meeting_id = self.ids.next("mtg_")
        day = self.rng.choice(self.days)
        start = _at(day, self.rng.randint(9, 15), self.rng.choice((0, 15, 30, 45)))
        end = start + timedelta(minutes=self.rng.choice(_MEETING_MINUTES))
        attendees = [user.email for user in self.rng.sample(self.firm.users, 3)]
        organizer = attendees[0]
        participants = [_participant("organizer", organizer)] + [
            _participant("attendee", address) for address in attendees[1:]
        ]
        self.calendar.append(
            {
                "meetingId": meeting_id,
                "subject": subject,
                "organizer": organizer,
                "attendees": attendees,
                "startsAt": _iso(start),
                "endsAt": _iso(end),
            }
        )
        self._record(
            system="m365_calendar",
            source_id=meeting_id,
            kind="meeting",
            actor=self.rng.choice(self.firm.users),
            start=start,
            minutes=int((end - start).total_seconds() // 60),
            label=self._unassigned_label(),
            title=subject,
            participants=participants,
        )

    def _excluded_meeting(self, domain: str) -> None:
        subject = self.rng.choice(_EXCLUDED_SUBJECTS)
        meeting_id = self.ids.next("mtg_")
        day = self.rng.choice(self.days)
        start = _at(day, self.rng.randint(9, 15), self.rng.choice((0, 15, 30, 45)))
        end = start + timedelta(minutes=self.rng.choice(_MEETING_MINUTES))
        contact = f"{self.rng.choice(_CONTACT_LOCALS)}@{domain}"
        owner = self.rng.choice(self.firm.users)
        attendees = [contact, owner.email]
        organizer = contact if self.rng.random() < 0.5 else owner.email
        participants = [_participant("organizer", organizer)] + [
            _participant("attendee", address) for address in attendees if address != organizer
        ]
        self.calendar.append(
            {
                "meetingId": meeting_id,
                "subject": subject,
                "organizer": organizer,
                "attendees": attendees,
                "startsAt": _iso(start),
                "endsAt": _iso(end),
            }
        )
        self._record(
            system="m365_calendar",
            source_id=meeting_id,
            kind="meeting",
            actor=owner,
            start=start,
            minutes=int((end - start).total_seconds() // 60),
            label=self._excluded_label(),
            title=subject,
            participants=participants,
        )

    def _desktop(self) -> None:
        for matter in self.firm.matters:
            count = 1 if matter.status == "closed" else (2 if self.rng.random() < 0.5 else 1)
            for _ in range(count):
                client = self.by_client[matter.client_id]
                hard = matter.status == "closed" or self.rng.random() < 0.7
                if hard:
                    title = f"{matter.reference} {matter.title}.docx"
                else:
                    title = f"{client.name} {matter.title.lower()}.pdf"
                kind = "document_edit" if self.rng.random() < 0.75 else "document_view"
                minutes = (
                    self.rng.randint(20, 90) if kind == "document_edit" else self.rng.randint(5, 25)
                )
                day = self.rng.choice(self.days)
                start = _at(day, self.rng.randint(8, 16), self.rng.randint(0, 59))
                owner = self.rng.choice(self.firm.users)
                document_id = self.ids.next("doc_")
                label = self._client_label(matter, evidence="hard_key" if hard else "content")
                self._record(
                    system="desktop_agent",
                    source_id=document_id,
                    kind=kind,
                    actor=owner,
                    start=start,
                    minutes=minutes,
                    label=label,
                    title=title,
                    document_ref=document_id,
                )

    def _manual(self) -> None:
        open_matters = [matter for matter in self.firm.matters if matter.status == "open"]
        for _ in range(6):
            matter = self.rng.choice(open_matters)
            client = self.by_client[matter.client_id]
            hard = self.rng.random() < 0.5
            if hard:
                description = (
                    f"[{matter.reference}] Call with {client.name} about {matter.title.lower()}"
                )
            else:
                description = f"Call with {client.name} about {matter.title.lower()}"
            self._record(
                system="manual",
                source_id=self.ids.next("man_"),
                kind="call",
                actor=self.rng.choice(self.firm.users),
                start=self._moment(),
                minutes=self.rng.randint(10, 45),
                label=self._client_label(matter, evidence="hard_key" if hard else "content"),
                title=description,
            )
        for _ in range(2):
            matter = self.rng.choice(open_matters)
            self._record(
                system="manual",
                source_id=self.ids.next("man_"),
                kind="manual_entry",
                actor=self.rng.choice(self.firm.users),
                start=self._moment(),
                minutes=self.rng.randint(15, 60),
                label=self._client_label(matter, evidence="hard_key"),
                title=f"[{matter.reference}] Attendance: {matter.title.lower()}",
            )
        courtesy = self.rng.choice(open_matters)
        courtesy_client = self.by_client[courtesy.client_id]
        self._record(
            system="manual",
            source_id=self.ids.next("man_"),
            kind="call",
            actor=self.rng.choice(self.firm.users),
            start=self._moment(),
            minutes=self.rng.randint(5, 20),
            label=self._not_billable_client_label(courtesy),
            title=(
                f"Courtesy update to {courtesy_client.name} "
                f"[{courtesy.reference}] on {courtesy.title.lower()}"
            ),
        )
        self._record(
            system="manual",
            source_id=self.ids.next("man_"),
            kind="call",
            actor=self.rng.choice(self.firm.users),
            start=self._moment(),
            minutes=self.rng.randint(5, 20),
            label=self._unassigned_label(),
            title="Call with candidate attorney about admission requirements",
        )

    def _moment(self) -> datetime:
        return _at(
            self.rng.choice(self.days),
            self.rng.randint(8, 16),
            self.rng.randint(0, 59),
        )


def generate(seed: int = DEFAULT_SEED) -> Corpus:
    """Build the whole corpus in memory."""
    return _Builder(seed).build()
