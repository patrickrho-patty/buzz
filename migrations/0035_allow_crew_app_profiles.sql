-- Allow crew-ios-* app_profile values alongside legacy buzz-ios-*.
-- The relay now advertises and writes crew-ios-* (model.rs), but must read
-- both spellings forever for existing push_leases/installations. The CHECK
-- constraint previously rejected crew- inserts.
--
-- This migration expands the constraint to the union. Old rows stay valid;
-- new rows may use either spelling. A future cleanup migration can narrow
-- back to crew- only after buzz- rows have naturally expired / been re-registered.

ALTER TABLE push_gateway_installations
    DROP CONSTRAINT IF EXISTS push_gateway_installations_app_profile_check;

ALTER TABLE push_gateway_installations
    ADD CONSTRAINT push_gateway_installations_app_profile_check
    CHECK (app_profile IN (
        'buzz-ios-production','buzz-ios-sandbox',
        'crew-ios-production','crew-ios-sandbox'
    ));
