//! zombie.erosion.utils.Noise2D: layered value noise over Ken Perlin's permutation.
use crate::zombie::iso::weather::simplex_noise::P;

/// zombie.erosion.utils.Noise2D.Layer
#[derive(Clone, Debug, PartialEq)]
struct Layer {
    freq: f32,
    amp: f32,
    p: Vec<i32>,
}

/// zombie.erosion.utils.Noise2D
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Noise2D {
    layers: Vec<Layer>,
}

/// zombie.erosion.utils.Noise2D.perm (the same 256 values as SimplexNoise.p).
fn perm(i: i32) -> f32 {
    f32::from(P[i as usize])
}

impl Noise2D {
    /// zombie.erosion.utils.Noise2D.lerp
    fn lerp(t: f32, a: f32, b: f32) -> f32 {
        a + t * (b - a)
    }

    /// zombie.erosion.utils.Noise2D.fade
    fn fade(t: f32) -> f32 {
        t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
    }

    /// zombie.erosion.utils.Noise2D.noise(float, float, int[]): the cell is taken modulo 255 in double.
    fn noise(orig_x: f32, orig_y: f32, layer_p: &[i32]) -> f32 {
        let x = (f64::from(orig_x) - f64::from(orig_x / 255.0).floor() * 255.0).floor() as i32;
        let y = (f64::from(orig_y) - f64::from(orig_y / 255.0).floor() * 255.0).floor() as i32;
        let fx = Self::fade(orig_x - f64::from(orig_x).floor() as f32);
        let fy = Self::fade(orig_y - f64::from(orig_y).floor() as f32);
        let aa = layer_p[x as usize] + y;
        let ab = layer_p[x as usize] + y + 1;
        let ba = layer_p[x as usize + 1] + y;
        let bb = layer_p[x as usize + 1] + y + 1;
        Self::lerp(
            fy,
            Self::lerp(fx, perm(layer_p[aa as usize]), perm(layer_p[ba as usize])),
            Self::lerp(fx, perm(layer_p[ab as usize]), perm(layer_p[bb as usize])),
        )
    }

    /// zombie.erosion.utils.Noise2D.layeredNoise: the amplitude-weighted sum of the layers, scaled to 0..1.
    pub fn layered_noise(&self, x: f32, y: f32) -> f32 {
        let mut fn_ = 0.0f32;
        let mut max_sum = 0.0f32;
        for layer in &self.layers {
            max_sum += layer.amp;
            fn_ += Self::noise(x * layer.freq, y * layer.freq, &layer.p) * layer.amp;
        }
        fn_ / max_sum / 255.0
    }

    /// zombie.erosion.utils.Noise2D.addLayer: the permutation rotated by the seed (modulo 256, through float).
    pub fn add_layer(&mut self, seed: i32, freq: f32, amp: f32) {
        let seed =
            (f64::from(seed) - f64::from(seed as f32 / 256.0).floor() * 256.0).floor() as i32;
        let mut p = vec![0i32; 512];
        for i in 0..256 {
            let s = seed + i;
            let ii = (f64::from(s) - f64::from(s as f32 / 256.0).floor() * 256.0).floor() as i32;
            p[ii as usize] = i32::from(P[i as usize]);
            p[256 + ii as usize] = p[ii as usize];
        }
        self.layers.push(Layer { freq, amp, p });
    }

    /// zombie.erosion.utils.Noise2D.reset
    pub fn reset(&mut self) {
        self.layers.clear();
    }
}
