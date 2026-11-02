# Written by Kone Tshivhinda & Claude
"""The firm, corpus and manifest shapes. No I/O lives here."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from typing import Any

from .jsonio import canonical_bytes

GENERATOR_VERSION = "1.0.0"


@dataclass(frozen=True)
class User:
    user_id: str
    name: str
    email: str
    role: str

    def to_dict(self) -> dict[str, Any]:
        return {
            "userId": self.user_id,
            "name": self.name,
            "email": self.email,
            "role": self.role,
        }


@dataclass(frozen=True)
class Client:
    client_id: str
    name: str
    domain: str

    def to_dict(self) -> dict[str, Any]:
        return {"clientId": self.client_id, "name": self.name, "domain": self.domain}


@dataclass(frozen=True)
class Matter:
    matter_id: str
    client_id: str
    reference: str
    title: str
    status: str
    practice_area: str
    rate_cents_per_hour: int

    def to_dict(self) -> dict[str, Any]:
        return {
            "matterId": self.matter_id,
            "clientId": self.client_id,
            "reference": self.reference,
            "title": self.title,
            "status": self.status,
            "practiceArea": self.practice_area,
            "rateCentsPerHour": self.rate_cents_per_hour,
        }


@dataclass(frozen=True)
class Exclusion:
    kind: str
    value: str
    reason: str

    def to_dict(self) -> dict[str, Any]:
        return {"kind": self.kind, "value": self.value, "reason": self.reason}


@dataclass(frozen=True)
class Firm:
    tenant_id: str
    name: str
    domain: str
    users: tuple[User, ...]
    clients: tuple[Client, ...]
    matters: tuple[Matter, ...]
    exclusions: tuple[Exclusion, ...]
    business_days: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": "timelex.gold-set.firm.v1",
            "tenantId": self.tenant_id,
            "firm": {"name": self.name, "domain": self.domain},
            "users": [user.to_dict() for user in self.users],
            "clients": [client.to_dict() for client in self.clients],
            "matters": [matter.to_dict() for matter in self.matters],
            "policy": {
                "exclusions": [exclusion.to_dict() for exclusion in self.exclusions],
                "businessDays": list(self.business_days),
            },
        }


@dataclass(frozen=True)
class Corpus:
    seed: int
    firm: Firm
    threads: tuple[dict[str, Any], ...]
    calendar: tuple[dict[str, Any], ...]
    events: tuple[dict[str, Any], ...]
    labels: tuple[dict[str, Any], ...]

    def files(self) -> dict[str, dict[str, Any]]:
        return {
            "calendar.json": {
                "schema": "timelex.gold-set.calendar.v1",
                "meetings": list(self.calendar),
            },
            "events.json": {
                "schema": "timelex.gold-set.events.v1",
                "events": list(self.events),
            },
            "firm.json": self.firm.to_dict(),
            "labels.json": {
                "schema": "timelex.gold-set.labels.v1",
                "labels": list(self.labels),
            },
            "threads.json": {
                "schema": "timelex.gold-set.threads.v1",
                "threads": list(self.threads),
            },
        }

    def manifest(self) -> dict[str, Any]:
        rendered = {name: canonical_bytes(body) for name, body in sorted(self.files().items())}
        return {
            "schema": "timelex.gold-set.manifest.v1",
            "generatorVersion": GENERATOR_VERSION,
            "seed": self.seed,
            "tenantId": self.firm.tenant_id,
            "files": [
                {
                    "name": name,
                    "sha256": hashlib.sha256(body).hexdigest(),
                    "bytes": len(body),
                }
                for name, body in rendered.items()
            ],
            "counts": {
                "users": len(self.firm.users),
                "clients": len(self.firm.clients),
                "matters": len(self.firm.matters),
                "threads": len(self.threads),
                "messages": sum(len(thread["messages"]) for thread in self.threads),
                "meetings": len(self.calendar),
                "events": len(self.events),
                "excludedEvents": sum(
                    1 for label in self.labels if label["expectedStorage"] == "excluded"
                ),
                "ambiguousEvents": sum(1 for label in self.labels if label["ambiguous"]),
                "reviewEvents": sum(
                    1 for label in self.labels if label["expectedOutcome"] == "review"
                ),
            },
        }
