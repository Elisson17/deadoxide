//! zombie.SystemDisabler: switches for parts of the game that the Lua options and debug scripts set. Only the
//! mutable (non-final) fields are kept; the final ones (`doHighFriction`, `doVehicleLowRider`, `printDetailedInfo`)
//! are false.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemDisabler {
    pub do_character_stats: bool,
    pub do_zombie_creation: bool,
    pub do_survivor_creation: bool,
    pub do_player_creation: bool,
    pub do_override_pov_characters: bool,
    pub do_vehicles_everywhere: bool,
    pub do_world_sync_enable: bool,
    pub do_main_loop_deal_with_net_data: bool,
    pub enable_advanced_sound_options: bool,
    pub uncapped_fps: bool,
}

impl Default for SystemDisabler {
    /// The static initialisers.
    fn default() -> Self {
        SystemDisabler {
            do_character_stats: true,
            do_zombie_creation: true,
            do_survivor_creation: false,
            do_player_creation: true,
            do_override_pov_characters: true,
            do_vehicles_everywhere: false,
            do_world_sync_enable: false,
            do_main_loop_deal_with_net_data: true,
            enable_advanced_sound_options: false,
            uncapped_fps: false,
        }
    }
}

impl SystemDisabler {
    /// zombie.SystemDisabler.Reset: everything but `uncappedFPS` back to its initial value.
    pub fn reset(&mut self) {
        let uncapped_fps = self.uncapped_fps;
        *self = SystemDisabler::default();
        self.uncapped_fps = uncapped_fps;
    }
}
