//! zombie.iso.weather.ClimateColorInfo: the interior and the exterior colour of a climate value.
use crate::java_nio::ByteBuffer;

/// zombie.core.Color as a climate colour holds it: r, g, b, a.
pub type Rgba = [f32; 4];

/// zombie.iso.weather.ClimateColorInfo
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClimateColorInfo {
    pub interior: Rgba,
    pub exterior: Rgba,
}

impl Default for ClimateColorInfo {
    /// zombie.iso.weather.ClimateColorInfo(): both colours are `new Color(0, 0, 0, 1)`, Color's int constructor, so
    /// alpha is 1/255, not 1.
    fn default() -> Self {
        let c = [0.0, 0.0, 0.0, 1.0f32 / 255.0];
        ClimateColorInfo {
            interior: c,
            exterior: c,
        }
    }
}

impl ClimateColorInfo {
    /// zombie.iso.weather.ClimateColorInfo(float, float, float, float): the same colour inside and outside, stored
    /// as given (no clamping).
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        ClimateColorInfo {
            interior: [r, g, b, a],
            exterior: [r, g, b, a],
        }
    }

    /// zombie.iso.weather.ClimateColorInfo(float r, g, b, a, float r2, g2, b2, a2): interior first, then exterior.
    #[allow(clippy::too_many_arguments)]
    pub fn new8(r: f32, g: f32, b: f32, a: f32, r2: f32, g2: f32, b2: f32, a2: f32) -> Self {
        ClimateColorInfo {
            interior: [r, g, b, a],
            exterior: [r2, g2, b2, a2],
        }
    }

    /// zombie.iso.weather.ClimateColorInfo.interp: zombie.core.Color.interp on the interior and the exterior,
    /// `this + (to - this) * t` per channel. The original writes into a result object (which may be `this`);
    /// here the result is returned.
    pub fn interp(&self, to: &ClimateColorInfo, t: f32) -> ClimateColorInfo {
        let c = |from: &Rgba, to: &Rgba| {
            let mut d = [0.0f32; 4];
            for i in 0..4 {
                let v = (to[i] - from[i]) * t;
                d[i] = from[i] + v;
            }
            d
        };
        ClimateColorInfo {
            interior: c(&self.interior, &to.interior),
            exterior: c(&self.exterior, &to.exterior),
        }
    }

    /// zombie.iso.weather.ClimateColorInfo.setInterior(float, float, float, float)
    pub fn set_interior(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.interior = [r, g, b, a];
    }

    /// zombie.iso.weather.ClimateColorInfo.setExterior(float, float, float, float)
    pub fn set_exterior(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.exterior = [r, g, b, a];
    }

    /// zombie.iso.weather.ClimateColorInfo.load(DataInputStream, int): interior r, g, b, a, then exterior r, g, b, a.
    pub fn load(&mut self, input: &mut ByteBuffer, _world_version: i32) -> Result<(), String> {
        for c in [&mut self.interior, &mut self.exterior] {
            for v in c.iter_mut() {
                *v = input.get_float()?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The int Color constructor divides by 255: a new colour's alpha is 1/255 (the float 0x3b808081).
    #[test]
    fn new_color_info_has_alpha_one_255th() {
        assert_eq!(
            ClimateColorInfo::default().exterior[3].to_bits(),
            0x3b80_8081
        );
    }
}
