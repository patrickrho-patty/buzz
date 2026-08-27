/**
 * One-time migration: copy every `crew-*` localStorage entry to its `crew-*`
 * twin when the new key is still absent.
 *
 * Large rename batches can't just rename the constant literals — users who
 * upgrade in place would silently lose drafts, read positions, mutes, etc.
 * This migrator runs once at app boot, before any feature code reads its
 * keys, mirroring the Sprout→Buzz `migrateLegacyCommunityStorage` pattern
 * but generically for the whole `buzz-*` namespace.
 *
 * Guard key prevents repeat scans. Old `crew-*` entries are kept (cheap) so
 * a rollback build still sees its data. A future major can clean them.
 */

const MIGRATION_GUARD_KEY = "crew-storage-migration.v1.done";

export function migrateBuzzStorageKeys(storage: Storage = localStorage): number {
  try {
    if (storage.getItem(MIGRATION_GUARD_KEY) !== null) return 0;

    // Snapshot keys first — storage.length shifts on mutation.
    const keys: string[] = [];
    for (let i = 0; i < storage.length; i++) {
      const k = storage.key(i);
      if (k !== null) keys.push(k);
    }

    let migrated = 0;
    for (const oldKey of keys) {
      if (!oldKey.startsWith("buzz-")) continue;
      const newKey = `crew-${oldKey.slice(5)}`;
      if (storage.getItem(newKey) !== null) continue;
      const value = storage.getItem(oldKey);
      if (value === null) continue;
      storage.setItem(newKey, value);
      migrated++;
    }

    storage.setItem(MIGRATION_GUARD_KEY, "1");
    if (migrated > 0) {
      console.info(
        `[buzzKeyMigration] migrated ${migrated} buzz-* keys to crew-*`,
      );
    }
    return migrated;
  } catch (error) {
    console.warn("[buzzKeyMigration] migration failed (storage denied?):", error);
    return 0;
  }
}
