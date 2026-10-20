// Written by Kone Tshivhinda & Claude
// Hand-mirrored from contracts/events/activity-event.schema.json until SDK generation (task P1-09)
// replaces this file. The test in client.test.ts checks the golden example against these keys.

export type SourceSystem =
  | "m365_mail"
  | "m365_calendar"
  | "m365_teams"
  | "google_mail"
  | "google_calendar"
  | "desktop_agent"
  | "manual"
  | "import";

export type ActivityKind =
  | "email_sent"
  | "email_received"
  | "meeting"
  | "call"
  | "document_edit"
  | "document_view"
  | "web_research"
  | "desktop_focus"
  | "manual_entry";

export interface Participant {
  role: "from" | "to" | "cc" | "attendee" | "organizer";
  domain: string;
  address?: string;
}

export interface ActivityEvent {
  schemaVersion: string;
  id: string;
  tenantId: string;
  source: { system: SourceSystem; sourceId: string };
  kind: ActivityKind;
  actor: { userId: string };
  startedAt: string;
  endedAt: string;
  capturedAt: string;
  subject?: {
    title?: string;
    threadRef?: string;
    documentRef?: string;
    participants?: Participant[];
  };
  contentRef?: string;
  privacy: { contentCaptured: boolean; redactionApplied: boolean };
}

export interface Health {
  status: string;
}

export interface Meta {
  name: string;
  version: string;
  contracts: { activityEvent: string };
  stage: string;
}
