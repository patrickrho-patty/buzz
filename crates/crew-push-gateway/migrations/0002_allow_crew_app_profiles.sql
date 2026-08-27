-- See migrations/0035_allow_crew_app_profiles.sql in the main relay DB.
ALTER TABLE push_gateway_installations
    DROP CONSTRAINT IF EXISTS push_gateway_installations_app_profile_check;

ALTER TABLE push_gateway_installations
    ADD CONSTRAINT push_gateway_installations_app_profile_check
    CHECK (app_profile IN (
        'buzz-ios-production','buzz-ios-sandbox',
        'crew-ios-production','crew-ios-sandbox'
    ));
