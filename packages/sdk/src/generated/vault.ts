// Written by Kone Tshivhinda & Claude
// Generated from contracts/openapi/vault.v1.yaml by tools/sdk-gen. Do not edit by hand.
// Run `npm run generate:sdk` after changing the contract; `npm run check:sdk` fails on drift.

export type paths = {
    "/health": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        /**
         * Liveness and database reachability
         * @description Answers 200 when the service is up and can reach its database, otherwise 503. No credentials; reachable only on the private interface.
         */
        get: operations["getHealth"];
        put?: never;
        post?: never;
        delete?: never;
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
         * List the firm's secrets as metadata
         * @description Returns metadata for every secret of the caller's firm, ordered by kind and then label. Values are never included. Revoked secrets are hidden unless asked for.
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
    "/v1/secrets/{id}": {
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
         * Revoke a secret
         * @description Marks the secret revoked and destroys its wrapped data key, so it can never be decrypted again. Revoking an already-revoked secret is a 204 (idempotent).
         */
        delete: operations["revokeSecret"];
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
         * @description Keeps the id, label and allowed hosts; replaces the value and writes a new encrypted version. The old value is overwritten, not kept.
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
         * @description Asks the secret's provider whether the credential is accepted. `anthropic_api_key` makes a
         *     real read-only call; other kinds are checked for shape only in 0.1.0. The value is decrypted
         *     in memory for the call and never leaves the process. Testing does not count as use: it sets
         *     `lastTestedAt`, not `lastUsedAt`.
         */
        post: operations["testSecret"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/secrets/{kind}": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        /**
         * Create or replace the firm's secret of one kind
         * @description Stores a value for the given kind. If the firm already has a live secret of that kind, its
         *     value is replaced under the same id (a rotation); `label` and `allowedHosts` are updated when
         *     supplied. Otherwise a secret is created. This is an upsert, so replaying the same request is
         *     safe. `allowedHosts` may narrow the kind's default hosts but never widen them; an attempt to
         *     name a host outside the default is a 400.
         */
        put: operations["putSecret"];
        post?: never;
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
        TestResult: {
            /** Format: date-time */
            checkedAt: string;
            /** @constant */
            ok: true;
        };
    };
    responses: {
        /** @description A valid token of the wrong kind, or one without `settings:manage`. */
        Forbidden: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description The vault or its database failed. `detail` is never included. */
        Internal: {
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
        /** @description No such secret in the caller's firm (secrets of other firms look the same). */
        NotFound: {
            headers: {
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
        /** @description Missing, malformed or invalid internal token. Always the same body. */
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
    parameters: {
        /** @description The provider family of the secret. */
        Kind: components["schemas"]["SecretKind"];
        /** @description The secret's id, as returned in its metadata. */
        SecretId: string;
    };
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
            /** @description Up. */
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
            /** @description The database cannot be reached. */
            503: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
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
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            500: components["responses"]["Internal"];
        };
    };
    revokeSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                id: components["parameters"]["SecretId"];
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
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            404: components["responses"]["NotFound"];
            500: components["responses"]["Internal"];
        };
    };
    rotateSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                id: components["parameters"]["SecretId"];
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
            500: components["responses"]["Internal"];
        };
    };
    testSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The secret's id, as returned in its metadata. */
                id: components["parameters"]["SecretId"];
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
            500: components["responses"]["Internal"];
            /** @description The provider could not be reached, or answered in a way that says nothing about the secret. */
            502: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
        };
    };
    putSecret: {
        parameters: {
            query?: never;
            header?: never;
            path: {
                /** @description The provider family of the secret. */
                kind: components["parameters"]["Kind"];
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
            500: components["responses"]["Internal"];
        };
    };
}
