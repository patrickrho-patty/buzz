//! Legacy app-data directory identifiers for Griddle→Crew and Sprout→Crew migrations.
//!
//! Split out of `migration.rs` to keep the file under the size gate.

use std::path::{Path, PathBuf};

pub(crate) const LEGACY_GRIDDLE_DEV_IDENTIFIER: &str = "xyz.patty.griddle.app.dev";
pub(crate) const LEGACY_GRIDDLE_RELEASE_IDENTIFIER: &str = "xyz.patty.griddle.app";
pub(crate) const LEGACY_SPROUT_DEV_IDENTIFIER: &str = "xyz.block.sprout.app.dev";
pub(crate) const LEGACY_SPROUT_RELEASE_IDENTIFIER: &str = "xyz.block.sprout.app";

pub(crate) fn legacy_sprout_app_data_dir(current: &Path) -> Option<PathBuf> {
    let name = current.file_name()?.to_str()?;
    let legacy_name = if name.starts_with(super::CANONICAL_DEV_IDENTIFIER) {
        name.replacen(
            super::CANONICAL_DEV_IDENTIFIER,
            LEGACY_SPROUT_DEV_IDENTIFIER,
            1,
        )
    } else if name.starts_with(super::CANONICAL_RELEASE_IDENTIFIER) {
        name.replacen(
            super::CANONICAL_RELEASE_IDENTIFIER,
            LEGACY_SPROUT_RELEASE_IDENTIFIER,
            1,
        )
    } else if name.starts_with(LEGACY_GRIDDLE_DEV_IDENTIFIER) {
        name.replacen(
            LEGACY_GRIDDLE_DEV_IDENTIFIER,
            LEGACY_SPROUT_DEV_IDENTIFIER,
            1,
        )
    } else if name.starts_with(LEGACY_GRIDDLE_RELEASE_IDENTIFIER) {
        name.replacen(
            LEGACY_GRIDDLE_RELEASE_IDENTIFIER,
            LEGACY_SPROUT_RELEASE_IDENTIFIER,
            1,
        )
    } else {
        return None;
    };
    current.parent().map(|parent| parent.join(legacy_name))
}


pub(crate) fn legacy_app_data_dir(current: &Path) -> Option<PathBuf> {
    let name = current.file_name()?.to_str()?;
    let legacy_name = if name.starts_with(super::CANONICAL_DEV_IDENTIFIER) {
        name.replacen(
            super::CANONICAL_DEV_IDENTIFIER,
            LEGACY_GRIDDLE_DEV_IDENTIFIER,
            1,
        )
    } else if name.starts_with(super::CANONICAL_RELEASE_IDENTIFIER) {
        name.replacen(
            super::CANONICAL_RELEASE_IDENTIFIER,
            LEGACY_GRIDDLE_RELEASE_IDENTIFIER,
            1,
        )
    } else {
        return None;
    };
    current.parent().map(|parent| parent.join(legacy_name))
}
