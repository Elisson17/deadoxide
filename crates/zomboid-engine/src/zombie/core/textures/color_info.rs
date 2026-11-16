//! zombie.core.textures.ColorInfo: a mutable RGBA colour (Core's highlight colours are ColorInfo objects).
use std::cell::Cell;

use crate::java::float_to_string;
use crate::se::krka::kahlua::vm::object::{JavaClass, Userdata};
use crate::zombie::lua::exposer::class_named;

/// zombie.core.textures.ColorInfo
#[derive(Debug, Default)]
pub struct ColorInfo {
    pub r: Cell<f32>,
    pub g: Cell<f32>,
    pub b: Cell<f32>,
    pub a: Cell<f32>,
}

impl ColorInfo {
    /// zombie.core.textures.ColorInfo.<init>(float, float, float, float)
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> ColorInfo {
        ColorInfo {
            r: Cell::new(r),
            g: Cell::new(g),
            b: Cell::new(b),
            a: Cell::new(a),
        }
    }

    /// zombie.core.textures.ColorInfo.toColor: `new Color(r, g, b, a)` (clamped to [0, 1]).
    pub fn to_color(&self) -> crate::zombie::core::color::Color {
        crate::zombie::core::color::Color::new_clamped(
            self.r.get(),
            self.g.get(),
            self.b.get(),
            self.a.get(),
        )
    }

    /// zombie.core.textures.ColorInfo.toImmutableColor: `new ImmutableColor(r, g, b, a)`.
    pub fn to_immutable_color(&self) -> crate::zombie::core::immutable_color::ImmutableColor {
        crate::zombie::core::immutable_color::ImmutableColor::new(
            self.r.get(),
            self.g.get(),
            self.b.get(),
            self.a.get(),
        )
    }

    /// zombie.core.textures.ColorInfo.set(float, float, float, float)
    pub fn set(&self, r: f32, g: f32, b: f32, a: f32) {
        self.r.set(r);
        self.g.set(g);
        self.b.set(b);
        self.a.set(a);
    }

    /// zombie.core.textures.ColorInfo.interp: `dest = this + (to - this) * delta`, per channel in float.
    pub fn interp(&self, to: &ColorInfo, delta: f32, dest: &ColorInfo) {
        let r = (to.r.get() - self.r.get()) * delta;
        let g = (to.g.get() - self.g.get()) * delta;
        let b = (to.b.get() - self.b.get()) * delta;
        let a = (to.a.get() - self.a.get()) * delta;
        dest.set(
            self.r.get() + r,
            self.g.get() + g,
            self.b.get() + b,
            self.a.get() + a,
        );
    }
}

impl Userdata for ColorInfo {
    fn class(&self) -> &'static JavaClass {
        class_named("zombie.core.textures.ColorInfo")
    }
    /// zombie.core.textures.ColorInfo.toString
    fn java_to_string(&self) -> String {
        format!(
            "Color ({},{},{},{})",
            float_to_string(self.r.get()),
            float_to_string(self.g.get()),
            float_to_string(self.b.get()),
            float_to_string(self.a.get())
        )
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
