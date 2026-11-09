-- P2.19 -- Migration 0011 down: revoke the API runtime role's telemetry
--          read grant (reverses 0011 up exactly).
REVOKE SELECT ON telemetry FROM flexcore_app;
