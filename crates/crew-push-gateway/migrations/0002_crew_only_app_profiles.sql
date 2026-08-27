-- See migrations/0036_crew_only_app_profiles.sql in the main relay DB.
ALTER TABLE push_gateway_installations
    DROP CONSTRAINT IF EXISTS push_gateway_installations_app_profile_check;

ALTER TABLE push_gateway_installations
    ADD CONSTRAINT push_gateway_installations_app_profile_check
    CHECK (app_profile IN ('crew-ios-production', 'crew-ios-sandbox'));
