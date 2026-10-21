# deploy/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Today: `docker-compose.yml` starts **Postgres 17** and **MinIO** (S3-compatible object storage) bound to localhost for development. Phase 1 (task P1-07) adds the services and the py-worker so `docker compose up` runs the whole stack; Helm and hosted infrastructure follow in Phases 5 and 8.

```bash
docker compose -f deploy/docker-compose.yml up -d
docker compose -f deploy/docker-compose.yml down        # keeps data volumes
docker compose -f deploy/docker-compose.yml down -v     # deletes data
```

The credentials in the file are throwaway local values. Pin the `latest` MinIO tag to a release before relying on it (Dependabot will propose updates once pinned).

## Running the gateway and identity by hand (until P1-07 does it for you)
Prerequisites: Postgres with the bootstrap applied ([postgres/README.md](postgres/README.md)), and `timelex_migrator` and `svc_identity` allowed to log in (set your own passwords; keep them in an untracked `.env`).
```bash
cargo run -p timelex-gateway -- keygen           # prints a key pair: put the lines in your untracked environment
# Choose an operator key, store only its hash:
#   TIMELEX_OPERATOR_KEY_SHA256=$(printf %s "$YOUR_OPERATOR_KEY" | sha256sum | cut -d' ' -f1)
# Database URLs (your own passwords): TIMELEX_MIGRATOR_DATABASE_URL, TIMELEX_IDENTITY_DATABASE_URL
cargo run -p timelex-identity-service -- migrate  # once, and after each release
cargo run -p timelex-identity-service             # 127.0.0.1:8081, private: only the gateway should reach it
cargo run -p timelex-gateway                      # 127.0.0.1:8088
curl -X POST http://127.0.0.1:8088/v1/tenants -H "authorization: Bearer $YOUR_OPERATOR_KEY" \
  -H 'content-type: application/json' \
  -d '{"slug":"example-attorneys","displayName":"Example Attorneys Inc","adminEmail":"partner@example-attorneys.example","adminDisplayName":"Pat Partner"}'
```
Environment variables are documented in `services/gateway/src/config.rs` and `services/identity/src/main.rs`. The `initialApiKey.token` in the answer is shown once.
