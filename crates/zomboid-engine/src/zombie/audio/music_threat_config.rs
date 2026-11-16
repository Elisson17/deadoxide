//! zombie.audio.MusicThreatConfig: the threat statuses and their music intensity (`media/lua/client/Music/
//! MusicThreatConfig.lua` hands its table to `initStatuses`).
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use super::music_intensity_config::cast_table_impl;
use crate::java;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::object::{TableRef, R};
use crate::zombie::core::math::pz_math;
use crate::zombie::util::string_utils::discard_null_or_whitespace;

/// zombie.audio.MusicThreatConfig.Status
#[derive(Debug)]
pub struct Status {
    pub id: String,
    pub intensity: f32,
    /// NaN when not overridden.
    pub intensity_override: Cell<f32>,
}

/// zombie.audio.MusicThreatConfig (one instance: getInstance()).
#[derive(Debug, Default)]
pub struct MusicThreatConfig {
    pub status_list: Vec<Rc<Status>>,
    /// A HashMap in the original: its order is not observable through the class.
    pub status_by_id: HashMap<String, Rc<Status>>,
}

impl MusicThreatConfig {
    /// zombie.audio.MusicThreatConfig.initStatuses: the list and the map cleared, then every value whose key is
    /// not "VERSION" (ignoring case) is a status table: `id` (blank is none) and `intensity` (rawgetFloat). One
    /// without id or with `intensity <= 0` is dropped (NaN is kept); one whose id is known replaces the earlier
    /// (removed from the list, added at the end).
    pub fn init_statuses(&mut self, statuses_table: &TableRef) -> R<()> {
        self.status_list.clear();
        self.status_by_id.clear();
        for (key, value) in KahluaTableImpl::iterate_snapshot(statuses_table) {
            if java::equals_ignore_case("VERSION", &key.java_to_string()) {
                continue;
            }
            let t = cast_table_impl(&value)?;
            let id = discard_null_or_whitespace(KahluaTableImpl::rawget_str(&t, "id")?);
            let intensity = KahluaTableImpl::rawget_float(&t, "intensity")?;
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            let Some(id) = id.filter(|_| !(intensity <= 0.0)) else {
                continue;
            };
            let status = Rc::new(Status {
                id: id.clone(),
                intensity,
                intensity_override: Cell::new(f32::NAN),
            });
            if let Some(old) = self.status_by_id.get(&id) {
                if let Some(i) = self.status_list.iter().position(|s| Rc::ptr_eq(s, old)) {
                    self.status_list.remove(i);
                }
            }
            self.status_list.push(status.clone());
            self.status_by_id.insert(id, status);
        }
        Ok(())
    }

    /// zombie.audio.MusicThreatConfig.getStatusCount
    pub fn get_status_count(&self) -> i32 {
        self.status_list.len() as i32
    }

    /// zombie.audio.MusicThreatConfig.getStatusIntensity: 0 for an unknown id.
    pub fn get_status_intensity(&self, id: &str) -> f32 {
        self.status_by_id.get(id).map_or(0.0, |s| s.intensity)
    }

    /// zombie.audio.MusicThreatConfig.setStatusIntensityOverride: below 0 clears it (NaN), else clamped to 0..1.
    pub fn set_status_intensity_override(&self, id: &str, intensity: f32) {
        if let Some(s) = self.status_by_id.get(id) {
            s.intensity_override.set(if intensity < 0.0 {
                f32::NAN
            } else {
                pz_math::clamp(intensity, 0.0, 1.0)
            });
        }
    }

    /// zombie.audio.MusicThreatConfig.getStatusIntensityOverride: 0 for an unknown id.
    pub fn get_status_intensity_override(&self, id: &str) -> f32 {
        self.status_by_id
            .get(id)
            .map_or(0.0, |s| s.intensity_override.get())
    }

    /// zombie.audio.MusicThreatConfig.isStatusIntensityOverridden
    pub fn is_status_intensity_overridden(&self, id: &str) -> bool {
        self.status_by_id
            .get(id)
            .is_some_and(|s| !s.intensity_override.get().is_nan())
    }
}
