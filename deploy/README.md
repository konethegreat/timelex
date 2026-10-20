# deploy/

_Author: Kone Tshivhinda · Written by Kone Tshivhinda & Claude_

Today: `docker-compose.yml` starts **Postgres 17** and **MinIO** (S3-compatible object storage) bound to localhost for development. Phase 1 (task P1-07) adds the services and the py-worker so `docker compose up` runs the whole stack; Helm and hosted infrastructure follow in Phases 5 and 8.

```bash
docker compose -f deploy/docker-compose.yml up -d
docker compose -f deploy/docker-compose.yml down        # keeps data volumes
docker compose -f deploy/docker-compose.yml down -v     # deletes data
```

The credentials in the file are throwaway local values. Pin the `latest` MinIO tag to a release before relying on it (Dependabot will propose updates once pinned).
