-- Rename mesh retention trigger/function from crew- to crew- naming.
-- The dual-read body (crew-* or crew-*) was introduced in 0033; this
-- migration renames the function and trigger to the crew- spelling while
-- keeping the same dual-read logic. Old trigger/function are dropped after
-- the new ones are created, so no event update is missed.

CREATE OR REPLACE FUNCTION purge_soft_deleted_crew_mesh_status() RETURNS trigger AS $$
BEGIN
    IF OLD.deleted_at IS NULL
       AND NEW.deleted_at IS NOT NULL
       AND NEW.kind = 30003
       AND (NEW.d_tag LIKE 'crew-mesh-member-status:%'
            OR NEW.d_tag LIKE 'buzz-mesh-member-status:%')
       AND (NEW.tags @> '[["k", "crew-mesh-status"]]'::jsonb
            OR NEW.tags @> '[["k", "buzz-mesh-status"]]'::jsonb) THEN
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

DROP TRIGGER IF EXISTS trg_events_purge_soft_deleted_buzz_mesh_status ON events;

CREATE TRIGGER trg_events_purge_soft_deleted_crew_mesh_status
    AFTER UPDATE OF deleted_at ON events
    FOR EACH ROW EXECUTE FUNCTION purge_soft_deleted_crew_mesh_status();

DROP FUNCTION IF EXISTS purge_soft_deleted_buzz_mesh_status();
