//! zombie.iso.weather.ThunderStorm: the thunder clouds of a storm, as far as a save stores them.
use crate::java_nio::ByteBuffer;

/// zombie.iso.weather.ThunderStorm.ThunderCloud: the saved fields (the current position, event frequency and
/// suspend timer are runtime state, not ported).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThunderCloud {
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
    pub thunder_ratio: f32,
    pub percentage_offset: f32,
    pub is_running: bool,
}

/// zombie.iso.weather.ThunderStorm: three clouds, plus the dummy cloud a save with more clouds reads into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThunderStorm {
    pub clouds: [ThunderCloud; 3],
    pub dummy_cloud: Option<ThunderCloud>,
}

impl ThunderStorm {
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
