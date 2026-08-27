-- Fresh-fork posture: drop the legacy buzz-ios-* app_profile spellings.
--
-- The Patty hard fork starts with a clean database and a new Apple
-- com.patty.crew bundle. No buzz-era installations, leases, or profile
-- values exist, so the dual-read adds a narrow crew-only CHECK is dead weight. Narrow the
-- CHECK back to crew-ios-* only; any stray buzz- rows would fail
-- validation at the constraint level.

ALTER TABLE push_gateway_installations
    DROP CONSTRAINT IF EXISTS push_gateway_installations_app_profile_check;

ALTER TABLE push_gateway_installations
    ADD CONSTRAINT push_gateway_installations_app_profile_check
    CHECK (app_profile IN ('crew-ios-production', 'crew-ios-sandbox'));