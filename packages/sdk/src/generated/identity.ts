// Written by Kone Tshivhinda & Claude
// Generated from contracts/openapi/identity.v1.yaml by tools/sdk-gen. Do not edit by hand.
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
         * @description Requires `keys:manage`. The key acts as `userId` (default: the caller). Its scopes must be a
         *     subset of that user's permissions, so a key can never do more than its owner. The owner must be
         *     an active user. An agent (machine) user can never hold `users:manage`, `keys:manage` or
         *     `settings:manage`. The firm is the caller's firm; a `userId` from another firm is reported as not
         *     found.
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
         * @description Requires `keys:manage`. Takes effect immediately. Revoking is permanent; issue a new key to
         *     rotate. A key id from another firm is reported as not found.
         */
        post: operations["revokeApiKey"];
        delete?: never;
        options?: never;
        head?: never;
        patch?: never;
        trace?: never;
    };
    "/v1/authenticate": {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        get?: never;
        put?: never;
        /**
         * Check a presented API key (internal, gateway only)
         * @description Resolves a token to the principal it acts as. Every failure (malformed, unknown, wrong secret,
         *     revoked, expired, owner disabled, firm suspended) returns the same 401 body, so a caller learns
         *     nothing about why. The reason is recorded in the audit log, not returned.
         *     `permissions` is the key's scopes limited to what the owner's roles grant **now**, so removing a
         *     role takes effect on existing keys immediately.
         */
        post: operations["authenticate"];
        delete?: never;
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
         * Create a firm and its first administrator
         * @description Creates the firm, its first admin (active, role `admin`) and an initial admin API key, in one
         *     transaction. Platform operators only. There are no passwords: the returned key is the first
         *     admin's credential until single sign-on exists.
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
        AuthenticateRequest: {
            /** @description The presented API key. */
            token: string;
        };
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
        /**
         * @description One thing that can be done. Also the vocabulary of API-key scopes.
         * @enum {string}
         */
        Permission: "users:read" | "users:manage" | "keys:manage" | "settings:manage" | "time:read" | "time:write" | "billing:read" | "billing:write" | "audit:read" | "capture:write";
        Principal: {
            keyId: components["schemas"]["Uuid"];
            /** @description The key's scopes limited to what the owner's roles grant now. */
            permissions: components["schemas"]["Permission"][];
            roles: components["schemas"]["Role"][];
            tenantId: components["schemas"]["Uuid"];
            userId: components["schemas"]["Uuid"];
        };
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
        /**
         * @description A job a user can hold. `agent` (a machine) cannot be combined with any other role.
         * @enum {string}
         */
        Role: "admin" | "fee_earner" | "billing" | "auditor" | "agent";
        /**
         * @description A firm's short public name. 3 to 40 characters.
         * @example example-attorneys
         */
        TenantSlug: string;
        /** Format: uuid */
        Uuid: string;
    };
    responses: {
        /** @description The caller lacks the permission this operation needs. */
        Forbidden: {
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
        /** @description No such object in the caller's firm (objects of other firms are indistinguishable from missing ones). */
        NotFound: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description Unexpected failure (for example the database). The body never contains internal details. */
        ServerError: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
        /** @description Missing, malformed or unusable credentials. Always the same body. */
        Unauthorized: {
            headers: {
                [name: string]: unknown;
            };
            content: {
                "application/problem+json": components["schemas"]["Problem"];
            };
        };
    };
    parameters: {
        /** @description Accepted and ignored in 0.1.0 (see the API description). */
        IdempotencyKey: string;
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
    issueApiKey: {
        parameters: {
            query?: never;
            header?: {
                /** @description Accepted and ignored in 0.1.0 (see the API description). */
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
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
            500: components["responses"]["ServerError"];
        };
    };
    revokeApiKey: {
        parameters: {
            query?: never;
            header?: {
                /** @description Accepted and ignored in 0.1.0 (see the API description). */
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
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
            500: components["responses"]["ServerError"];
        };
    };
    authenticate: {
        parameters: {
            query?: never;
            header?: never;
            path?: never;
            cookie?: never;
        };
        requestBody: {
            content: {
                "application/json": components["schemas"]["AuthenticateRequest"];
            };
        };
        responses: {
            /** @description The key is valid. */
            200: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/json": components["schemas"]["Principal"];
                };
            };
            401: components["responses"]["Unauthorized"];
            403: components["responses"]["Forbidden"];
            500: components["responses"]["ServerError"];
        };
    };
    createTenant: {
        parameters: {
            query?: never;
            header?: {
                /** @description Accepted and ignored in 0.1.0 (see the API description). */
                "Idempotency-Key"?: components["parameters"]["IdempotencyKey"];
            };
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
            403: components["responses"]["Forbidden"];
            /** @description The slug is already taken. */
            409: {
                headers: {
                    [name: string]: unknown;
                };
                content: {
                    "application/problem+json": components["schemas"]["Problem"];
                };
            };
            500: components["responses"]["ServerError"];
        };
    };
}
