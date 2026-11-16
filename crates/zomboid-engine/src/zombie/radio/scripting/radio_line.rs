//! zombie.radio.scripting.RadioLine: one line of a broadcast with its colour and effect codes.

/// zombie.radio.scripting.RadioLine
#[derive(Clone, Debug, PartialEq)]
pub struct RadioLine {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub text: String,
    pub effects: String,
    air_time: f32,
}

impl RadioLine {
    /// `RadioLine(txt, red, green, blue, fx)`: a null text keeps "<!text missing!>", a null `fx` keeps "".
    pub fn new(txt: Option<&str>, red: f32, green: f32, blue: f32, fx: Option<&str>) -> Self {
        RadioLine {
            r: red,
            g: green,
            b: blue,
            text: txt.unwrap_or("<!text missing!>").to_string(),
            effects: fx.unwrap_or("").to_string(),
            air_time: -1.0,
        }
    }

    /// `isCustomAirTime`
    pub fn is_custom_air_time(&self) -> bool {
        self.air_time > 0.0
    }

    pub fn air_time(&self) -> f32 {
        self.air_time
    }

    pub fn set_air_time(&mut self, air_time: f32) {
        self.air_time = air_time;
    }
}
