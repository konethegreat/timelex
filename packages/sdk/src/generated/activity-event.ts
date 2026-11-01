// Written by Kone Tshivhinda & Claude
// Generated from contracts/events/activity-event.schema.json by tools/sdk-gen. Do not edit by hand.
// Run `npm run generate:sdk` after changing the contract; `npm run check:sdk` fails on drift.

/**
 * Immutable fact about one observed work signal. Everything derived (time blocks, attributions, narratives, ledger entries) cites these by id. Written by Kone Tshivhinda & Claude.
 */
export interface ActivityEvent {
  /**
   * Major version 1. Additive changes bump the minor; breaking changes need a new major.
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
    }[];
  };
  /**
   * Object-storage key of redacted content, present only when privacy.contentCaptured is true.
   */
  contentRef?: string;
  privacy: {
    contentCaptured: boolean;
    redactionApplied: boolean;
  };
}
