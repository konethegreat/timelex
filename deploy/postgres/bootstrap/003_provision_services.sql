-- Written by Kone Tshivhinda & Claude
-- Provision the services that exist in Phase 1. Add a line here only when a service has code
-- ("build only what the next service needs", PHASES.md). host follows with P1-13.
CALL platform.provision_service('identity');
CALL platform.provision_service('vault');
