// Written by Kone Tshivhinda & Claude
// Generated from contracts/openapi/gateway.v1.yaml by tools/sdk-gen. Do not edit by hand.
// Run `npm run generate:sdk` after changing the contract; `npm run check:sdk` fails on drift.

export type paths = {
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Liveness probe */
        get: operations["getHealth"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/mcp": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * MCP Streamable HTTP endpoint (stateless, 2026-07-28)
         * @description Routes one MCP request without parsing the JSON body. `Mcp-Method` must be `tools/call`; the
         *     tool named in `Mcp-Name` is looked up in `contracts/mcp/routes.json`, and the request is
         *     forwarded unchanged to the service that owns it with an internal token addressed to that
         *     service. The service re-checks `MCP-Protocol-Version`, `Mcp-Method` and `Mcp-Name` against the
         *     body and enforces `Origin` (MCP 2026-07-28).
         *
         *     Other methods (`server/discover`, `tools/list`, `tasks/*`) are not routed yet and answer 501,
         *     pending P1-15. Streaming responses (SSE) are not supported yet: the gateway buffers the
         *     service's answer. Tenant API keys only; operator credentials are refused.
         */
        post: operations["mcpPost"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/api-keys": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Issue an API key
         * @description Routed to identity. Requires `keys:manage`. Same request and response as `identity.v1.yaml` `issueApiKey`.
         */
        post: operations["issueApiKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/api-keys/{keyId}/revoke": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Revoke an API key
         * @description Routed to identity. Requires `keys:manage`. Takes effect immediately.
         */
        post: operations["revokeApiKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/capture/entries": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Record work by hand (manual quick capture)
         * @description Routed to capture. Requires `capture:write`. The firm and user come from the API key, never
         *     from the request, and the server clock supplies `capturedAt`. Same request and response as
         *     `capture.v1.yaml` `createEntry`: 201 stores a new activity event, and 200 means the same
         *     `entryId` was delivered before, so a retry is safe. Kinds a person may capture are `call`,
         *     `meeting` and `manual_entry`.
         */
        post: operations["createEntry"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/meta": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /** Build and contract metadata */
        get: operations["getMeta"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/privacy/data-flow": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Technical data-flow register for the firm's POPIA records
         * @description Open. A versioned technical record of what this build stores, where it goes and which
         *     processors are active. It describes the platform, not a firm, and carries no tenant data, so
         *     the same register is served to every caller and can be attached to a firm's POPIA
         *     documentation. It is explicitly not legal advice (ADR-0007 point 7).
         */
        get: operations["getDataFlowRegister"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/privacy/exclusions": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * List the firm's privacy exclusions
         * @description Routed to capture. Requires `settings:manage`; capture enforces it. Same response as
         *     `capture.v1.yaml` `listExclusions`. Domains are stored lower-case with no trailing dot.
         */
        get: operations["listExclusions"];
        put?: never;
        /**
         * Configure a privacy exclusion (idempotent)
         * @description Routed to capture. Requires `settings:manage`. Same request and response as
         *     `capture.v1.yaml` `addExclusion`: 201 adds it, 200 means the same kind and value was already
         *     excluded and nothing changed. A domain exclusion covers subdomains.
         */
        post: operations["addExclusion"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/privacy/exclusions/{kind}/{value}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        post?: never;
        /**
         * Stop excluding a domain, client or matter
         * @description Routed to capture. Requires `settings:manage`. The gateway forwards only a known kind and a
         *     URL-safe value; anything else is a 400 without reaching capture. Same response as
         *     `capture.v1.yaml` `removeExclusion`.
         */
        delete: operations["removeExclusion"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/secrets": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * List the firm's provider secrets as metadata
         * @description Routed to the vault. Requires `settings:manage`. Same response as `vault.v1.yaml` `listSecrets`; values are never returned.
         */
        get: operations["listSecrets"];
        put?: never;
        post?: never;
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/secrets/{id}/rotate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Replace a secret's value under the same id
         * @description Routed to the vault. Requires `settings:manage`. Keeps the id, label and allowed hosts; replaces the value.
         */
        post: operations["rotateSecret"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/secrets/{id}/test": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Check that a stored secret still works
         * @description Routed to the vault. Requires `settings:manage`. The vault asks the secret's provider whether
         *     the credential is accepted; the value is decrypted in memory for the call and never leaves the
         *     vault. Testing sets `lastTestedAt`, not `lastUsedAt`.
         */
        post: operations["testSecret"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/secrets/{key}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Create or replace the firm's secret of one kind
         * @description Routed to the vault. Requires `settings:manage`. Same request and response as `vault.v1.yaml`
         *     `putSecret`: an upsert that returns 201 for a new secret and 200 when a live one of that kind was
         *     replaced. `allowedHosts` may narrow the kind's default hosts but never widen them.
         */
        put: operations["putSecret"];
        post?: never;
        /**
         * Revoke a secret
         * @description Routed to the vault. Requires `settings:manage`. Revoking an already-revoked secret is a 204 (idempotent).
         */
        delete: operations["revokeSecret"];
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/tenants": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Create a firm and its first administrator (platform operators only)
         * @description Routed to identity. Same request and response as `identity.v1.yaml` `createTenant`.
         */
        post: operations["createTenant"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
};
export type webhooks = Record<string, never>;
export type components = {
    schemas: {
        CreateTenantRequest: {
            /** @example Pat Partner */
            adminDisplayName: string;
            /**
             * @description Lowercase ASCII address; surrounding spaces and capitals are normalised. Use .example or .test addresses in tests.
             * @example partner@example-attorneys.example
             */
            adminEmail: string;
            /** @example Example Attorneys Inc */
            displayName: string;
            slug: components["schemas"]["TenantSlug"];
        };
        CreateTenantResponse: {
            adminUserId: components["schemas"]["Uuid"];
            initialApiKey: components["schemas"]["IssuedApiKey"];
            slug: components["schemas"]["TenantSlug"];
            tenantId: components["schemas"]["Uuid"];
        };
        /**
         * @description A versioned technical record of this build's data flows, storage and processors. It holds no
         *     tenant data. It is a factual engineering record for a firm's POPIA documentation, not legal
         *     advice.
         */
        DataFlowRegister: {
            captureDefaults: {
                contentCaptured: boolean;
                domainExclusionsEnforcedBeforeStorage: boolean;
                manualEntriesStoreTitleOnly: boolean;
                redactionRequiredBeforeContent: boolean;
            };
            dataCategories: {
                containsPersonalData: boolean;
                description: string;
                id: string;
                retention: string;
                storage: string;
            }[];
            disclaimer: string;
            exclusions: {
                /**
                 * @description False when a new exclusion changes nothing already stored.
                 * @constant
                 */
                appliesToExistingEvents: false;
                /** @description Exclusion kinds enforced before an event is stored. */
                enforcedAtCapture: ("domain" | "client" | "matter")[];
                /** @description Exclusion kinds recorded now and enforced when attribution exists. */
                recordedForAttribution: ("domain" | "client" | "matter")[];
            };
            flows: {
                data: string;
                from: string;
                id: string;
                /** @enum {string} */
                status: "active" | "development_only" | "not_active";
                storage: string;
                to: string;
            }[];
            processors: {
                name: string;
                purpose: string;
                region: string;
                /** @enum {string} */
                status: "active" | "development_only" | "not_active";
            }[];
            retentionNote: string;
            /** @description What part of the product this register describes */
            scope: string;
            /** Format: date */
            updatedAt: string;
            /** @description Register version, e.g. 1.0 */
            version: string;
        };
        Exclusion: {
            /** Format: date-time */
            createdAt: string;
            kind: components["schemas"]["ExclusionKind"];
            /** @description An optional note from the person who configured it. */
            reason?: string;
            /** @description The canonical value. Domains are lower-case with no trailing dot. */
            value: string;
        };
        /**
         * @description What is excluded. A domain is enforced at capture; client and matter are enforced when attribution exists (P3).
         * @enum {string}
         */
        ExclusionKind: "domain" | "client" | "matter";
        ExclusionList: {
            exclusions: components["schemas"]["Exclusion"][];
        };
        ExclusionResult: {
            /** @description True when this call added the exclusion; false when it was already there. */
            created: boolean;
            kind: components["schemas"]["ExclusionKind"];
            value: string;
        };
        IngestResult: {
            /** @description True when this delivery stored the event; false when it was already stored or the firm excludes the signal. */
            created: boolean;
            /** @description The event's canonical id, echoed so a sender can record what the service kept. */
            id: string;
        };
        IssueApiKeyRequest: {
            /**
             * Format: date-time
             * @description Optional expiry; must be in the future.
             */
            expiresAt?: string;
            /** @example capture connector */
            name: string;
            scopes: components["schemas"]["Permission"][];
            /** @description The user the key acts as. Defaults to the caller. */
            userId?: components["schemas"]["Uuid"];
        };
        IssuedApiKey: {
            /** Format: date-time */
            createdAt: string;
            /** Format: date-time */
            expiresAt?: string | null;
            id: components["schemas"]["Uuid"];
            name: string;
            /** @description First 8 characters of the secret part, for telling keys apart in lists. */
            prefix: string;
            scopes: components["schemas"]["Permission"][];
            /** @description The full secret, shown once. Format `tlx_<32 hex tenant id>.<43 characters>`. Treat as a password. */
            token: string;
            userId: components["schemas"]["Uuid"];
        };
        /** @description What a person submits for quick capture. The server derives the event. */
        ManualEntry: {
            /** @description Optional short description, stored as `subject.title`. Whitespace is collapsed to one line. */
            description?: string;
            /** Format: date-time */
            endedAt: string;
            /**
             * Format: uuid
             * @description The caller's idempotency key. Any accepted UUID spelling; the same key and start is the same signal.
             */
            entryId: string;
            /**
             * @description What the person did. `manual_entry` covers work that fits no other kind.
             * @enum {string}
             */
            kind: "call" | "meeting" | "manual_entry";
            /** Format: date-time */
            startedAt: string;
        };
        NewExclusion: {
            kind: components["schemas"]["ExclusionKind"];
            /** @description An optional note, plain text only. */
            reason?: string;
            /** @description A domain (case-insensitive, subdomains included) or a client or matter identifier of at most 128 characters. */
            value: string;
        };
        /**
         * @description One thing that can be done. Also the vocabulary of API-key scopes.
         * @enum {string}
         */
        Permission: "users:read" | "users:manage" | "keys:manage" | "settings:manage" | "time:read" | "time:write" | "billing:read" | "billing:write" | "audit:read" | "capture:write";
        /** @description RFC 9457 problem details (standard error body for all services). */
        Problem: {
            detail?: string;
            instance?: string;
            status: number;
            title: string;
            traceId?: string;
            /** Format: uri-reference */
            type: string;
        };
        PutSecretRequest: {
            /** @description May narrow the kind's default hosts, never widen them. Required for generic_api_key. */
            allowedHosts?: string[];
            /** @description Defaults to the kind's human name. */
            label?: string;
            /** @description The credential. Never echoed, never logged. */
            value: string;
        };
        RotateRequest: {
            value: string;
        };
        /**
         * @description `anthropic_api_key` (a workspace or service-account key discussed in ADR-0004; never an Admin
         *     key), `m365_oauth`, `google_oauth` (refresh tokens), or `generic_api_key` for any other
         *     provider, which must name its hosts.
         * @enum {string}
         */
        SecretKind: "anthropic_api_key" | "m365_oauth" | "google_oauth" | "generic_api_key";
        SecretList: {
            secrets: components["schemas"]["SecretMetadata"][];
        };
        /** @description What a caller may know about a secret. Never the value. */
        SecretMetadata: {
            /** @description The hosts this secret may be used against. Always within the kind's default. */
            allowedHosts: string[];
            /** Format: date-time */
            createdAt: string;
            /** Format: uuid */
            id: string;
            kind: components["schemas"]["SecretKind"];
            /** @description A name chosen by the firm, for example "Anthropic (billing workspace)". */
            label: string;
            /** @description The last four characters of the value, or fewer for a very short value. Never more than four. */
            lastFour: string;
            /**
             * Format: date-time
             * @description When `test` last ran, whether it passed or failed. Absent if never tested.
             */
            lastTestedAt?: string;
            /**
             * Format: date-time
             * @description When an internal caller last used the secret. Absent if never used; testing does not count.
             */
            lastUsedAt?: string;
            /**
             * Format: date-time
             * @description Absent while the secret is live.
             */
            revokedAt?: string;
            /** Format: date-time */
            updatedAt: string;
        };
        /**
         * @description A firm's short public name. 3 to 40 characters.
         * @example example-attorneys
         */
        TenantSlug: string;
        TestResult: {
            /** Format: date-time */
            checkedAt: string;
            /** @constant */
            ok: true;
        };
        /** Format: uuid */
        Uuid: string;
    };
    responses: {
        /** @description The service behind this route could not be reached. Nothing was changed. */
        BadGateway: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The credential is valid but lacks the permission this operation needs. */
        Forbidden: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The service did not answer within 5 seconds. The outcome is unknown; check before retrying. */
        GatewayTimeout: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description A field failed validation. `detail` names the rule; it never echoes a secret. */
        Invalid: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description No such object in the caller's firm (objects of other firms look the same). */
        NotFound: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description This MCP method is valid but not routed yet, or the tool's service has no configured URL. */
        NotImplemented: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The caller exceeded its request budget (a firm, or the platform operator). `Retry-After` says how many seconds to wait. */
        RateLimited: {
            headers: {
                /** @description Seconds to wait before retrying (integer, at least 1). */
                "Retry-After"?: number;
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The request body exceeds 64 KiB. */
        TooLarge: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /**
         * @description Missing, malformed or unusable credentials (unknown, revoked, expired, owner disabled, firm
         *     suspended). Always the same body, so the reason is not revealed.
         */
        Unauthorized: {
            headers: {
                /** @description Always `Bearer`. */
                "WWW-Authenticate"?: "Bearer";
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
    };
    parameters: never;
    requestBodies: never;
    headers: never;
    pathItems: never;
};
export type $defs = Record<string, never>;
export interface operations {
    getHealth: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Gateway is up */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        /** @constant */
                        status: "ok";
                    };
                };
            };
        };
    };
    mcpPost: {
        parameters: {
            query?: never;
            header: {
                /** @description Mirrors the JSON-RPC method. Only `tools/call` is routed in this version. */
                "Mcp-Method": string;
                /** @description The tool name for `tools/call` (required there), or the task id for `tasks/*`. */
                "Mcp-Name"?: string;
                /** @description Must be `2026-07-28`; older versions are refused. */
                "MCP-Protocol-Version": "2026-07-28";
                /** @description Forwarded unchanged; the service validates it and answers 403 when it is not allowed. */
                Origin?: string;
            };
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": Record<string, never>;
            };
        };
        responses: {
            /** @description The service's MCP answer, relayed unchanged. JSON in 0.3.0; SSE is not yet supported. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": Record<string, never>;
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            /** @description The tool named in `Mcp-Name` is not in the routing table. */
            404: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            501: components["responses"]["NotImplemented"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    issueApiKey: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["IssueApiKeyRequest"];
            };
        };
        responses: {
            /** @description Key issued. The token is shown only now. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IssuedApiKey"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    revokeApiKey: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                keyId: components["schemas"]["Uuid"];
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Revoked. */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    createEntry: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["ManualEntry"];
            };
        };
        responses: {
            /** @description The same entry was delivered before; nothing changed. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IngestResult"];
                };
            };
            /** @description The entry became a new activity event. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["IngestResult"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    getMeta: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Metadata */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": {
                        contracts: {
                            /** @description Activity-event schema version, e.g. 1.0 */
                            activityEvent: string;
                        };
                        name: string;
                        /** @description pre-alpha | alpha | beta | stable */
                        stage: string;
                        version: string;
                    };
                };
            };
        };
    };
    getDataFlowRegister: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The current register. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["DataFlowRegister"];
                };
            };
        };
    };
    listExclusions: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The firm's exclusions. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExclusionList"];
                };
            };
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    addExclusion: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["NewExclusion"];
            };
        };
        responses: {
            /** @description The exclusion was already there; nothing changed. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExclusionResult"];
                };
            };
            /** @description The exclusion is new. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["ExclusionResult"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    removeExclusion: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                kind: "domain" | "client" | "matter";
                /** @description The value as configured; domains ignore case and a trailing dot. */
                value: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Removed. */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    listSecrets: {
        parameters: {
            query?: {
                /** @description Include secrets that have been revoked. */
                includeRevoked?: boolean;
            };
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The firm's secrets. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SecretList"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    rotateSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                id: string;
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["RotateRequest"];
            };
        };
        responses: {
            /** @description The secret was rotated. Metadata only. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SecretMetadata"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            /** @description The secret has been revoked and cannot be rotated. */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    testSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                id: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description The provider accepted the secret. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["TestResult"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            /** @description The secret has been revoked and cannot be tested. */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            /** @description The provider rejected the secret. `detail` says so; the provider's own message is not relayed. */
            422: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            429: components["responses"]["RateLimited"];
            /** @description The vault could not be reached, or the provider could not be reached or answered in a way that says nothing about the secret. */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            504: components["responses"]["GatewayTimeout"];
        };
    };
    putSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The provider family of the secret. */
                key: components["schemas"]["SecretKind"];
            };
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["PutSecretRequest"];
            };
        };
        responses: {
            /** @description The existing secret of that kind was replaced. Metadata only. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SecretMetadata"];
                };
            };
            /** @description A new secret was stored. Metadata only. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["SecretMetadata"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    revokeSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                key: string;
            };
            cookie?: never;
        };
        requestBody?: never;
        responses: {
            /** @description Revoked, or already revoked. */
            204: {
                headers: {
                    [name: string]: unknown;
                };
                content?: never;
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
    createTenant: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["CreateTenantRequest"];
            };
        };
        responses: {
            /** @description Firm created. The token in `initialApiKey` is shown only now. */
            201: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["CreateTenantResponse"];
                };
            };
            400: components["responses"]["Invalid"];
            401: components["responses"]["Unauthorized"];
            /** @description The slug is already taken. */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            413: components["responses"]["TooLarge"];
            429: components["responses"]["RateLimited"];
            502: components["responses"]["BadGateway"];
            504: components["responses"]["GatewayTimeout"];
        };
    };
}
