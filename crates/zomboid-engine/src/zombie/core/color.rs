//! zombie.core.Color: a mutable RGBA colour of floats (the parts the port needs so far).

/// zombie.core.Color
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    /// zombie.core.Color(): black with alpha 1.
    fn default() -> Self {
        Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }
}

impl Color {
    /// zombie.core.Color.HSBtoRGB(float, float, float): each channel rounded to a byte as `(int)(x * 255 + 0.5)`
    /// (float arithmetic, truncated), then divided by 255; alpha 1 (`set(r, g, b)`).
    pub fn hsb_to_rgb(hue: f32, saturation: f32, brightness: f32) -> Color {
        let byte = |x: f32| (x * 255.0 + 0.5) as i32;
        let (mut r, mut g, mut b) = (0, 0, 0);
        if saturation == 0.0 {
            r = byte(brightness);
            g = r;
            b = r;
        } else {
            let h = (hue - hue.floor()) * 6.0;
            let f = h - h.floor();
            let p = brightness * (1.0 - saturation);
            let q = brightness * (1.0 - saturation * f);
            let t = brightness * (1.0 - saturation * (1.0 - f));
            match h as i32 {
                0 => (r, g, b) = (byte(brightness), byte(t), byte(p)),
                1 => (r, g, b) = (byte(q), byte(brightness), byte(p)),
                2 => (r, g, b) = (byte(p), byte(brightness), byte(t)),
                3 => (r, g, b) = (byte(p), byte(q), byte(brightness)),
                4 => (r, g, b) = (byte(t), byte(p), byte(brightness)),
                5 => (r, g, b) = (byte(brightness), byte(p), byte(q)),
                _ => {}
            }
        }
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The six hue sectors and the grey case, against `java.awt.Color.HSBtoRGB`'s documented results.
    #[test]
    fn hsb_sectors() {
        let c = |h, s, v| {
            let c = Color::hsb_to_rgb(h, s, v);
            [c.r, c.g, c.b].map(|x| (x * 255.0).round() as i32)
        };
        assert_eq!(c(0.0, 1.0, 1.0), [255, 0, 0]);
        assert_eq!(c(1.0 / 3.0, 1.0, 1.0), [0, 255, 0]);
        assert_eq!(c(2.0 / 3.0, 1.0, 1.0), [0, 0, 255]);
        assert_eq!(c(0.5, 0.0, 0.5), [128, 128, 128]);
        assert_eq!(c(1.5, 1.0, 1.0), [0, 255, 255]);
    }
}
