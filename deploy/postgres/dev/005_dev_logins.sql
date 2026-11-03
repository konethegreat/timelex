-- Written by Kone Tshivhinda & Claude
-- LOCAL COMPOSE ONLY. The bootstrap creates service roles NOLOGIN and sets no passwords; the dev
-- stack connects over the container network with trust authentication, so the roles only need LOGIN.
-- Never run this file in a shared or hosted database: it belongs to deploy/docker-compose.yml.
ALTER ROLE timelex_migrator LOGIN;
ALTER ROLE svc_identity LOGIN;
ALTER ROLE svc_vault LOGIN;
ALTER ROLE svc_capture LOGIN;
ALTER ROLE svc_matters LOGIN;
