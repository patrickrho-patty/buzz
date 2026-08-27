-- Fresh-fork posture: mesh retention matches crew-* spellings only.
--
-- 0033/0034 introduced dual-read (crew-* or buzz-*) for pre-fork persisted
-- events. A fresh Patty database has no buzz-era rows, so recreate the
-- trigger function with the crew-* match alone.

CREATE OR REPLACE FUNCTION purge_soft_deleted_crew_mesh_status() RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NULL
       AND NEW.deleted_at IS NOT NULL
       AND NEW.kind = 30003
       AND NEW.d_tag LIKE 'crew-mesh-member-status:%'
       AND NEW.tags @> '[["k", "crew-mesh-status"]]'::jsonb THEN
        DELETE FROM events
        WHERE community_id = NEW.community_id
          AND created_at = NEW.created_at
          AND id = NEW.id;

        DELETE FROM event_mentions
        WHERE community_id = NEW.community_id AND event_id = NEW.id;
    END IF;

    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
