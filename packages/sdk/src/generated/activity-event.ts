// Written by Kone Tshivhinda & Claude
// Generated from contracts/events/activity-event.schema.json by tools/sdk-gen. Do not edit by hand.
// Run `npm run generate:sdk` after changing the contract; `npm run check:sdk` fails on drift.

/**
 * Immutable fact about one observed work signal. Everything derived (time blocks, attributions, narratives, ledger entries) cites these by id. Written by Kone Tshivhinda & Claude.
 */
export interface ActivityEvent {
  /**
   * Major version 1. Optional properties may be added in a minor and readers ignore fields they do not know; required properties, field meanings, types and enum values are fixed within a major. See contracts/events/README.md.
   */
  schemaVersion: string;
  /**
   * Idempotent id: hex digest of tenant + source.system + source.sourceId + startedAt. Delivering the same signal twice yields the same id.
   */
  id: string;
  tenantId: string;
  source: {
    system:
      | "m365_mail"
      | "m365_calendar"
      | "m365_teams"
      | "google_mail"
      | "google_calendar"
      | "desktop_agent"
      | "manual"
      | "import";
    /**
     * Id of the signal in the originating system.
     */
    sourceId: string;
    [k: string]: unknown;
  };
  kind:
    | "email_sent"
    | "email_received"
    | "meeting"
    | "call"
    | "document_edit"
    | "document_view"
    | "web_research"
    | "desktop_focus"
    | "manual_entry";
  actor: {
    userId: string;
    [k: string]: unknown;
  };
  startedAt: string;
  endedAt: string;
  capturedAt: string;
  /**
   * Metadata about the signal. Titles and addresses may be absent under a metadata-only or redacted privacy tier.
   */
  subject?: {
    title?: string;
    threadRef?: string;
    documentRef?: string;
    /**
     * @maxItems 200
     */
    participants?: {
      role: "from" | "to" | "cc" | "attendee" | "organizer";
      domain: string;
      address?: string;
      [k: string]: unknown;
    }[];
    [k: string]: unknown;
  };
  /**
   * Object-storage key of redacted content, present only when privacy.contentCaptured is true.
   */
  contentRef?: string;
  privacy: {
    contentCaptured: boolean;
    redactionApplied: boolean;
    [k: string]: unknown;
  };
  [k: string]: unknown;
}
