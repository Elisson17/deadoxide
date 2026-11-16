//! zombie.iso.weather.ThunderStorm: the thunder clouds of a storm, their movement and the thunder events they trigger,
//! and the part of a save that stores the clouds.
//!
//! On a dedicated server (the only side modelled) a thunder event is handed to the network (`networkThunderEvent`
//! and a climate packet); the lightning flash, the thunder sound and the per-player lightning state exist only for
//! a client or a single-player game, whose players, FMOD and UI speed controls are not ported (UNKNOWN there).
use super::climate_manager::{clamp01, lerp, ClimateEnv};
use crate::java::JavaRandom;
use crate::java_fdlibm;
use crate::java_nio::ByteBuffer;
use crate::zombie::core::random::rand_abstract::RandAbstract;
use crate::zombie::game_time::{AnimTimer, GameTime};

/// zombie.iso.weather.ThunderStorm.ThunderEvent as the server's `networkThunderEvent` holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ThunderEvent {
    pub event_x: i32,
    pub event_y: i32,
    pub do_lightning: bool,
    pub do_rumble: bool,
    pub do_strike: bool,
}

/// zombie.iso.weather.ThunderStorm.ThunderCloud
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThunderCloud {
    pub current_x: i32,
    pub current_y: i32,
    pub start_x: i32,
    pub start_y: i32,
    pub end_x: i32,
    pub end_y: i32,
    pub start_time: f64,
    pub end_time: f64,
    pub duration: f64,
    pub strength: f32,
    pub angle: f32,
    pub radius: f32,
    pub event_frequency: f32,
    pub thunder_ratio: f32,
    pub percentage_offset: f32,
    pub is_running: bool,
    pub suspend_timer: AnimTimer,
}

impl ThunderCloud {
    /// zombie.iso.weather.ThunderStorm.ThunderCloud.setCenter: the track through (centerX, centerY) along the
    /// angle, starting and ending 1500..7500 squares past the map's half diagonal (two draws from Rand); the cloud
    /// starts at its start point. `diag` is `ThunderStorm.getMapDiagonal()`.
    pub fn set_center<R: JavaRandom>(
        &mut self,
        center_x: i32,
        center_y: i32,
        angle: f32,
        diag: i32,
        rand: &mut RandAbstract<R>,
    ) {
        let angle_opposing = add_to_angle(angle, 180.0);
        let mut rand_dist = diag.wrapping_add(rand.next_range(1500, 7500));
        let rad = to_radians(f64::from(angle_opposing));
        let sx = (f64::from(center_x) + f64::from(rand_dist) * java_fdlibm::cos(rad)) as i32;
        let sy = (f64::from(center_y) + f64::from(rand_dist) * java_fdlibm::sin(rad)) as i32;
        rand_dist = diag.wrapping_add(rand.next_range(1500, 7500));
        let rad = to_radians(f64::from(angle));
        let ex = (f64::from(center_x) + f64::from(rand_dist) * java_fdlibm::cos(rad)) as i32;
        let ey = (f64::from(center_y) + f64::from(rand_dist) * java_fdlibm::sin(rad)) as i32;
        self.start_x = sx;
        self.start_y = sy;
        self.end_x = ex;
        self.end_y = ey;
        self.current_x = sx;
        self.current_y = sy;
    }
}

/// java.lang.Math.toRadians (JDK 17+): `angdeg * 0.017453292519943295`.
fn to_radians(angdeg: f64) -> f64 {
    angdeg * 0.017453292519943295
}

/// zombie.iso.weather.ThunderStorm.addToAngle
fn add_to_angle(angle: f32, addition: f32) -> f32 {
    let mut angle = angle + addition;
    if angle > 360.0 {
        angle -= 360.0;
    } else if angle < 0.0 {
        angle += 360.0;
    }
    angle
}

/// zombie.iso.weather.ThunderStorm: three clouds, plus the dummy cloud a save with more clouds reads into. The map
/// bounds are statics in the original (set by `ClimateManager.init`); here the storm of each manager holds them.
#[derive(Clone, Debug, PartialEq)]
pub struct ThunderStorm {
    pub map_min_x: i32,
    pub map_min_y: i32,
    pub map_max_x: i32,
    pub map_max_y: i32,
    pub has_active_thunder_clouds: bool,
    pub clouds: [ThunderCloud; 3],
    pub strike_radius: i32,
    pub network_thunder_event: ThunderEvent,
    pub dummy_cloud: Option<ThunderCloud>,
}

impl Default for ThunderStorm {
    /// zombie.iso.weather.ThunderStorm(ClimateManager): idle clouds; the static map bounds start at
    /// (-3000, -3000)..(25000, 20000), the strike radius at 4000.
    fn default() -> Self {
        ThunderStorm {
            map_min_x: -3000,
            map_min_y: -3000,
            map_max_x: 25000,
            map_max_y: 20000,
            has_active_thunder_clouds: false,
            clouds: Default::default(),
            strike_radius: 4000,
            network_thunder_event: ThunderEvent::default(),
            dummy_cloud: None,
        }
    }
}

impl ThunderStorm {
    /// zombie.iso.weather.ThunderStorm.getFreeCloud: the first cloud that is not running.
    fn get_free_cloud(&self) -> Option<usize> {
        self.clouds.iter().position(|c| !c.is_running)
    }

    /// zombie.iso.weather.ThunderStorm.stopAllClouds
    pub fn stop_all_clouds(&mut self) {
        for c in self.clouds.iter_mut() {
            c.is_running = false;
        }
    }

    /// zombie.iso.weather.ThunderStorm.getMapDiagonal: half the diagonal of the map bounds, truncated. The
    /// original's `Math.sqrt(Math.pow(width, 2.0) + Math.pow(height, 2.0))` is exact here (the squares of an int
    /// range are doubles with no rounding, `pow(x, 2.0)` is `x * x`, and the square root is correctly rounded).
    pub fn get_map_diagonal(&self) -> i32 {
        let width = f64::from(self.map_max_x.wrapping_sub(self.map_min_x));
        let height = f64::from(self.map_max_y.wrapping_sub(self.map_min_y));
        let diag = (width * width + height * height).sqrt() as i32;
        diag / 2
    }

    /// zombie.iso.weather.ThunderStorm.startThunderCloud: takes the first free cloud (none: nothing, no draw),
    /// sets it from the arguments (the angle moved by Rand.Next(-10, 10), the radius capped at 20000), the centre
    /// of its track drawn in the middle three fifths of the map, and (when it follows a player) the centre of a
    /// random player. A dedicated server with no players logs and returns null, leaving the cloud stopped but
    /// set; with players it picks one of those standing somewhere (UNKNOWN: the players are not ported, so this
    /// keeps the drawn centre). Returns the cloud's index.
    #[allow(clippy::too_many_arguments)]
    pub fn start_thunder_cloud<R: JavaRandom>(
        &mut self,
        str_: f32,
        angle: f32,
        radius: f32,
        event_freq: f32,
        thunder_ratio: f32,
        duration: f64,
        target_random_player: bool,
        percentage_offset: f32,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) -> Option<usize> {
        if env.client {
            return None;
        }
        let i = self.get_free_cloud()?;
        let mut angle = angle;
        angle = add_to_angle(angle, rand.next_float(-10.0, 10.0));
        let start_time = gt.get_world_age_hours();
        let width = self.map_max_x.wrapping_sub(self.map_min_x);
        let height = self.map_max_y.wrapping_sub(self.map_min_y);
        {
            let c = &mut self.clouds[i];
            c.start_time = start_time;
            c.end_time = c.start_time + duration;
            c.duration = duration;
            c.strength = clamp01(str_);
            c.angle = angle;
            c.radius = radius;
            if c.radius > 20000.0 {
                c.radius = 20000.0;
            }
            c.event_frequency = event_freq;
            c.thunder_ratio = clamp01(thunder_ratio);
            c.percentage_offset = clamp01(percentage_offset);
        }
        let center_x = rand.next_range(
            self.map_min_x.wrapping_add(width / 5),
            self.map_max_x.wrapping_sub(width / 5),
        );
        let center_y = rand.next_range(
            self.map_min_y.wrapping_add(height / 5),
            self.map_max_y.wrapping_sub(height / 5),
        );
        if target_random_player && env.server && env.no_players {
            return None;
        }
        let diag = self.get_map_diagonal();
        let c = &mut self.clouds[i];
        c.set_center(center_x, center_y, angle, diag, rand);
        c.is_running = true;
        c.suspend_timer.init(3);
        Some(i)
    }

    /// zombie.iso.weather.ThunderStorm.update on a server: each running cloud moves along its track by the
    /// elapsed share of its duration (from its percentage offset), counts down its suspend timer and, when that
    /// ends, draws the next wait, the strike radius and a thunder event (a strike with some lightning and rumble,
    /// or a rumble only) near the cloud; past its end it stops. The client/single-player part (lightning
    /// application and sound) is not modelled.
    pub fn update<R: JavaRandom>(
        &mut self,
        current_time: f64,
        gt: &GameTime,
        env: &ClimateEnv,
        rand: &mut RandAbstract<R>,
    ) {
        if !env.client || env.server {
            self.has_active_thunder_clouds = false;
            for i in 0..self.clouds.len() {
                if !self.clouds[i].is_running {
                    continue;
                }
                if current_time < self.clouds[i].end_time {
                    let c = &mut self.clouds[i];
                    let mut t = ((current_time - c.start_time) / c.duration) as f32;
                    if c.percentage_offset > 0.0 {
                        t = c.percentage_offset + (1.0 - c.percentage_offset) * t;
                    }
                    c.current_x = lerp(t, c.start_x as f32, c.end_x as f32) as i32;
                    c.current_y = lerp(t, c.start_y as f32, c.end_y as f32) as i32;
                    c.suspend_timer.update(gt);
                    self.has_active_thunder_clouds = true;
                    if c.suspend_timer.finished() {
                        let suspend_next = rand.next_float(
                            3.5 - 3.0 * c.strength,
                            24.0 - 20.0 * c.strength,
                        );
                        c.suspend_timer.init((suspend_next * 60.0) as i32);
                        let r = rand.next_float(0.0, 1.0);
                        let (radius, strength_thunder_ratio, cx, cy) =
                            (c.radius, c.thunder_ratio, c.current_x, c.current_y);
                        if r < 0.6 {
                            self.strike_radius = (radius / 2.0) as i32 / 3;
                        } else if r < 0.9 {
                            self.strike_radius = (radius / 2.0) as i32 / 4 * 3;
                        } else {
                            self.strike_radius = (radius / 2.0) as i32;
                        }
                        let sr = self.strike_radius;
                        if rand.next_float(0.0, 1.0) < strength_thunder_ratio {
                            let x = rand.next_range(cx.wrapping_sub(sr), cx.wrapping_add(sr));
                            let y = rand.next_range(cy.wrapping_sub(sr), cy.wrapping_add(sr));
                            let rumble = rand.next_float(0.0, 1.0) > 0.4;
                            self.trigger_thunder_event(
                                x,
                                y,
                                true,
                                !env.disable_lightning_during_storms,
                                rumble,
                                env,
                            );
                        } else {
                            let x = rand.next_range(cx.wrapping_sub(sr), cx.wrapping_add(sr));
                            let y = rand.next_range(cy.wrapping_sub(sr), cy.wrapping_add(sr));
                            self.trigger_thunder_event(x, y, false, false, true, env);
                        }
                    }
                } else {
                    self.clouds[i].is_running = false;
                }
            }
        }
    }

    /// zombie.iso.weather.ThunderStorm.triggerThunderEvent on a server: the event is stored as the network event
    /// (and sent to the clients in a climate packet, not modelled). Neither a client nor a server (single player)
    /// enqueues it locally, which needs the players: not modelled.
    pub fn trigger_thunder_event(
        &mut self,
        x: i32,
        y: i32,
        do_strike: bool,
        do_lightning: bool,
        do_rumble: bool,
        env: &ClimateEnv,
    ) {
        if env.server {
            self.network_thunder_event = ThunderEvent {
                event_x: x,
                event_y: y,
                do_lightning,
                do_rumble,
                do_strike,
            };
        }
    }

    /// zombie.iso.weather.ThunderStorm.load(DataInputStream): a signed byte count, then for each cloud its running
    /// flag and, when running, its track, strength and timing. Clouds past the three go into one dummy cloud (made
    /// when the count is over three); a count of zero or less reads nothing more.
    pub fn load(&mut self, input: &mut ByteBuffer) -> Result<(), String> {
        let len = i32::from(input.get()?);
        if len != 0 {
            if len > self.clouds.len() as i32 && self.dummy_cloud.is_none() {
                self.dummy_cloud = Some(ThunderCloud::default());
            }
            for i in 0..len {
                let is_running = input.get()? != 0;
                let cloud = match self.clouds.get_mut(i as usize) {
                    Some(c) => c,
                    None => self.dummy_cloud.get_or_insert_with(ThunderCloud::default),
                };
                cloud.is_running = is_running;
                if is_running {
                    cloud.start_x = input.get_int()?;
                    cloud.start_y = input.get_int()?;
                    cloud.end_x = input.get_int()?;
                    cloud.end_y = input.get_int()?;
                    cloud.radius = input.get_float()?;
                    cloud.angle = input.get_float()?;
                    cloud.strength = input.get_float()?;
                    cloud.thunder_ratio = input.get_float()?;
                    cloud.start_time = input.get_double()?;
                    cloud.end_time = input.get_double()?;
                    cloud.duration = input.get_double()?;
                    cloud.percentage_offset = input.get_float()?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four clouds in the file: three go into the storm's clouds, the fourth into the dummy cloud.
    #[test]
    fn clouds_past_three_go_into_the_dummy() {
        let mut data = vec![4u8, 0, 0, 0, 1];
        for v in [1i32, 2, 3, 4] {
            data.extend(v.to_be_bytes());
        }
        for v in [5.0f32, 6.0, 7.0, 8.0] {
            data.extend(v.to_be_bytes());
        }
        for v in [9.0f64, 10.0, 11.0] {
            data.extend(v.to_be_bytes());
        }
        data.extend(12.0f32.to_be_bytes());
        let mut t = ThunderStorm::default();
        t.load(&mut ByteBuffer::wrap(&data)).unwrap();
        assert!(t.clouds.iter().all(|c| !c.is_running));
        let d = t.dummy_cloud.unwrap();
        assert!(d.is_running);
        assert_eq!(
            (d.start_x, d.end_y, d.radius, d.duration),
            (1, 4, 5.0, 11.0)
        );
    }
}
