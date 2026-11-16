//! zombie.core.Colors: the named colours of the static initialiser (`AddGameColor`, `addColor`, `addColorCB` in
//! field order) and the game colours XuiManager.ParseScripts adds. The colours are shared objects in the original
//! (`ColNfo.color` is the instance `colorMap` holds, and `AddGameColor` of an existing game colour `set`s it in
//! place); here they live in one pool and the tables hold indices into it.
use std::collections::{HashMap, HashSet};

use super::color::Color;
use crate::java;

/// zombie.core.Colors.ColorSet
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorSet {
    Game,
    Standard,
    ColorBlind,
}

impl ColorSet {
    /// java.lang.Enum.name
    pub fn name(self) -> &'static str {
        match self {
            ColorSet::Game => "Game",
            ColorSet::Standard => "Standard",
            ColorSet::ColorBlind => "ColorBlind",
        }
    }

    /// zombie.core.Colors.ColorSet.getIndex
    pub fn get_index(self) -> i32 {
        self as i32
    }
}

/// zombie.core.Colors.ColNfo, the ported part: the set, the name and the colour (the r/g/b/hex copies are not kept).
#[derive(Clone, Debug, PartialEq)]
pub struct ColNfo {
    pub color_set: ColorSet,
    pub name: String,
    /// Index into the colour pool (`ColNfo.color`).
    pub color: usize,
}

/// zombie.core.Colors: its static tables as one value.
#[derive(Clone, Debug, PartialEq)]
pub struct Colors {
    pool: Vec<Color>,
    info_map: HashMap<String, ColNfo>,
    info_list: Vec<ColNfo>,
    colors: Vec<usize>,
    /// lower-case name -> colour
    color_map: HashMap<String, usize>,
    color_names: Vec<String>,
    color_set: HashSet<String>,
    cb_info_map: HashMap<String, ColNfo>,
    cb_colors: Vec<usize>,
    cb_color_map: HashMap<String, usize>,
    cb_color_names: Vec<String>,
    cb_color_set: HashSet<String>,
}

/// zombie.core.Color(float, float, float): alpha 1, no clamping.
fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}

impl Default for Colors {
    fn default() -> Self {
        Self::new()
    }
}

impl Colors {
    /// zombie.core.Colors.<clinit>: the static Color fields in declaration order (`Fuchsia` and `Aqua` are the
    /// Magenta and Cyan instances, not entries of their own).
    pub fn new() -> Self {
        let mut c = Colors {
            pool: Vec::new(),
            info_map: HashMap::new(),
            info_list: Vec::new(),
            colors: Vec::new(),
            color_map: HashMap::new(),
            color_names: Vec::new(),
            color_set: HashSet::new(),
            cb_info_map: HashMap::new(),
            cb_colors: Vec::new(),
            cb_color_map: HashMap::new(),
            cb_color_names: Vec::new(),
            cb_color_set: HashSet::new(),
        };
        c.add_game_color("UI_Background", rgb(0.0, 0.0, 0.0));
        c.add_game_color("Black", rgb(0.0, 0.0, 0.0));
        c.add_color_cb("SherwoodGreen", rgb(0.0, 0.239, 0.188));
        c.add_color_cb("PthaloGreen", rgb(0.0, 0.341, 0.271));
        c.add_color_cb("TropicalRainForest", rgb(0.0, 0.451, 0.361));
        c.add_color_cb("Observatory", rgb(0.0, 0.569, 0.459));
        c.add_color_cb("JungleGreen", rgb(0.0, 0.686, 0.557));
        c.add_color_cb("Dali", rgb(0.0, 0.796, 0.655));
        c.add_color_cb("AquaMarine", rgb(0.0, 0.922, 0.757));
        c.add_color_cb("LightAqua", rgb(0.525, 1.0, 0.871));
        c.add_color_cb("Submerge", rgb(0.0, 0.188, 0.435));
        c.add_color_cb("Elvis", rgb(0.0, 0.282, 0.62));
        c.add_color_cb("FlatMediumBlue", rgb(0.0, 0.373, 0.8));
        c.add_color_cb("ClearBlue", rgb(0.0, 0.475, 0.98));
        c.add_color_cb("Azure", rgb(0.0, 0.624, 0.98));
        c.add_color_cb("SpiroDiscoBall", rgb(0.0, 0.761, 0.976));
        c.add_color_cb("AquaBlue", rgb(0.0, 0.898, 0.973));
        c.add_color_cb("LightBrilliantCyan", rgb(0.486, 1.0, 0.98));
        c.add_color_cb("DeepAmaranth", rgb(0.373, 0.0353, 0.0784));
        c.add_color_cb("HotChile", rgb(0.525, 0.0314, 0.11));
        c.add_color_cb("Smashing", rgb(0.698, 0.0275, 0.145));
        c.add_color_cb("GeraniumLake", rgb(0.871, 0.051, 0.18));
        c.add_color_cb("RedOrange", rgb(1.0, 0.259, 0.208));
        c.add_color_cb("Crusta", rgb(1.0, 0.529, 0.208));
        c.add_color_cb("GoldenYellow", rgb(1.0, 0.725, 0.208));
        c.add_color_cb("BananaYellow", rgb(1.0, 0.886, 0.224));
        c.add_color_cb("White", rgb(1.0, 1.0, 1.0));
        c.add_color("White", rgb(1.0, 1.0, 1.0));
        c.add_color("Silver", rgb(0.753, 0.753, 0.753));
        c.add_color("Gray", rgb(0.502, 0.502, 0.502));
        c.add_color("Red", rgb(1.0, 0.0, 0.0));
        c.add_color("Maroon", rgb(0.502, 0.0, 0.0));
        c.add_color("Yellow", rgb(1.0, 1.0, 0.0));
        c.add_color("Olive", rgb(0.502, 0.502, 0.0));
        c.add_color("Lime", rgb(0.0, 1.0, 0.0));
        c.add_color("Green", rgb(0.0, 0.502, 0.0));
        c.add_color("Cyan", rgb(0.0, 1.0, 1.0));
        c.add_color("Teal", rgb(0.0, 0.502, 0.502));
        c.add_color("Blue", rgb(0.0, 0.0, 1.0));
        c.add_color("Navy", rgb(0.0, 0.0, 0.502));
        c.add_color("Magenta", rgb(1.0, 0.0, 1.0));
        c.add_color("Purple", rgb(0.502, 0.0, 0.502));
        c.add_color("Orange", rgb(1.0, 0.647, 0.0));
        c.add_color("Pink", rgb(1.0, 0.753, 0.796));
        c.add_color("Brown", rgb(0.647, 0.165, 0.165));
        c.add_color("Gainsboro", rgb(0.863, 0.863, 0.863));
        c.add_color("LightGray", rgb(0.827, 0.827, 0.827));
        c.add_color("DarkGray", rgb(0.663, 0.663, 0.663));
        c.add_color("DimGray", rgb(0.412, 0.412, 0.412));
        c.add_color("LightSlateGray", rgb(0.467, 0.533, 0.6));
        c.add_color("SlateGray", rgb(0.439, 0.502, 0.565));
        c.add_color("DarkSlateGray", rgb(0.184, 0.31, 0.31));
        c.add_color("IndianRed", rgb(0.804, 0.361, 0.361));
        c.add_color("LightCoral", rgb(0.941, 0.502, 0.502));
        c.add_color("Salmon", rgb(0.98, 0.502, 0.447));
        c.add_color("DarkSalmon", rgb(0.914, 0.588, 0.478));
        c.add_color("LightSalmon", rgb(1.0, 0.627, 0.478));
        c.add_color("Crimson", rgb(0.863, 0.0784, 0.235));
        c.add_color("FireBrick", rgb(0.698, 0.133, 0.133));
        c.add_color("DarkRed", rgb(0.545, 0.0, 0.0));
        c.add_color("LightPink", rgb(1.0, 0.714, 0.757));
        c.add_color("HotPink", rgb(1.0, 0.412, 0.706));
        c.add_color("DeepPink", rgb(1.0, 0.0784, 0.576));
        c.add_color("MediumVioletRed", rgb(0.78, 0.0824, 0.522));
        c.add_color("PaleVioletRed", rgb(0.859, 0.439, 0.576));
        c.add_color("Coral", rgb(1.0, 0.498, 0.314));
        c.add_color("Tomato", rgb(1.0, 0.388, 0.278));
        c.add_color("OrangeRed", rgb(1.0, 0.271, 0.0));
        c.add_color("DarkOrange", rgb(1.0, 0.549, 0.0));
        c.add_color("Gold", rgb(1.0, 0.843, 0.0));
        c.add_color("LightYellow", rgb(1.0, 1.0, 0.878));
        c.add_color("LemonChiffon", rgb(1.0, 0.98, 0.804));
        c.add_color("LightGoldenrodYellow", rgb(0.98, 0.98, 0.824));
        c.add_color("PapayaWhip", rgb(1.0, 0.937, 0.835));
        c.add_color("Moccasin", rgb(1.0, 0.894, 0.71));
        c.add_color("PeachPuff", rgb(1.0, 0.855, 0.725));
        c.add_color("PaleGoldenrod", rgb(0.933, 0.91, 0.667));
        c.add_color("Khaki", rgb(0.941, 0.902, 0.549));
        c.add_color("DarkKhaki", rgb(0.741, 0.718, 0.42));
        c.add_color("Lavender", rgb(0.902, 0.902, 0.98));
        c.add_color("Thistle", rgb(0.847, 0.749, 0.847));
        c.add_color("Plum", rgb(0.867, 0.627, 0.867));
        c.add_color("Violet", rgb(0.933, 0.51, 0.933));
        c.add_color("Orchid", rgb(0.855, 0.439, 0.839));
        c.add_color("MediumOrchid", rgb(0.729, 0.333, 0.827));
        c.add_color("MediumPurple", rgb(0.576, 0.439, 0.859));
        c.add_color("RebeccaPurple", rgb(0.4, 0.2, 0.6));
        c.add_color("BlueViolet", rgb(0.541, 0.169, 0.886));
        c.add_color("DarkViolet", rgb(0.58, 0.0, 0.827));
        c.add_color("DarkOrchid", rgb(0.6, 0.196, 0.8));
        c.add_color("DarkMagenta", rgb(0.545, 0.0, 0.545));
        c.add_color("Indigo", rgb(0.294, 0.0, 0.51));
        c.add_color("SlateBlue", rgb(0.416, 0.353, 0.804));
        c.add_color("DarkSlateBlue", rgb(0.282, 0.239, 0.545));
        c.add_color("MediumSlateBlue", rgb(0.482, 0.408, 0.933));
        c.add_color("GreenYellow", rgb(0.678, 1.0, 0.184));
        c.add_color("Chartreuse", rgb(0.498, 1.0, 0.0));
        c.add_color("LawnGreen", rgb(0.486, 0.988, 0.0));
        c.add_color("LimeGreen", rgb(0.196, 0.804, 0.196));
        c.add_color("PaleGreen", rgb(0.596, 0.984, 0.596));
        c.add_color("LightGreen", rgb(0.565, 0.933, 0.565));
        c.add_color("MediumSpringGreen", rgb(0.0, 0.98, 0.604));
        c.add_color("SpringGreen", rgb(0.0, 1.0, 0.498));
        c.add_color("MediumSeaGreen", rgb(0.235, 0.702, 0.443));
        c.add_color("SeaGreen", rgb(0.18, 0.545, 0.341));
        c.add_color("ForestGreen", rgb(0.133, 0.545, 0.133));
        c.add_color("DarkGreen", rgb(0.0, 0.392, 0.0));
        c.add_color("YellowGreen", rgb(0.604, 0.804, 0.196));
        c.add_color("OliveDrab", rgb(0.42, 0.557, 0.137));
        c.add_color("DarkOliveGreen", rgb(0.333, 0.42, 0.184));
        c.add_color("MediumAquamarine", rgb(0.4, 0.804, 0.667));
        c.add_color("DarkSeaGreen", rgb(0.561, 0.737, 0.545));
        c.add_color("LightSeaGreen", rgb(0.125, 0.698, 0.667));
        c.add_color("DarkCyan", rgb(0.0, 0.545, 0.545));
        c.add_color("LightCyan", rgb(0.878, 1.0, 1.0));
        c.add_color("PaleTurquoise", rgb(0.686, 0.933, 0.933));
        c.add_color("Aquamarine", rgb(0.498, 1.0, 0.831));
        c.add_color("Turquoise", rgb(0.251, 0.878, 0.816));
        c.add_color("MediumTurquoise", rgb(0.282, 0.82, 0.8));
        c.add_color("DarkTurquoise", rgb(0.0, 0.808, 0.82));
        c.add_color("CadetBlue", rgb(0.373, 0.62, 0.627));
        c.add_color("SteelBlue", rgb(0.275, 0.51, 0.706));
        c.add_color("LightSteelBlue", rgb(0.69, 0.769, 0.871));
        c.add_color("PowderBlue", rgb(0.69, 0.878, 0.902));
        c.add_color("LightBlue", rgb(0.678, 0.847, 0.902));
        c.add_color("SkyBlue", rgb(0.529, 0.808, 0.922));
        c.add_color("LightSkyBlue", rgb(0.529, 0.808, 0.98));
        c.add_color("DeepSkyBlue", rgb(0.0, 0.749, 1.0));
        c.add_color("DodgerBlue", rgb(0.118, 0.565, 1.0));
        c.add_color("CornFlowerBlue", rgb(0.392, 0.584, 0.929));
        c.add_color("RoyalBlue", rgb(0.255, 0.412, 0.882));
        c.add_color("MediumBlue", rgb(0.0, 0.0, 0.804));
        c.add_color("DarkBlue", rgb(0.0, 0.0, 0.545));
        c.add_color("MidnightBlue", rgb(0.098, 0.098, 0.439));
        c.add_color("Cornsilk", rgb(1.0, 0.973, 0.863));
        c.add_color("BlanchedAlmond", rgb(1.0, 0.922, 0.804));
        c.add_color("Bisque", rgb(1.0, 0.894, 0.769));
        c.add_color("NavajoWhite", rgb(1.0, 0.871, 0.678));
        c.add_color("Wheat", rgb(0.961, 0.871, 0.702));
        c.add_color("BurlyWood", rgb(0.871, 0.722, 0.529));
        c.add_color("Tan", rgb(0.824, 0.706, 0.549));
        c.add_color("RosyBrown", rgb(0.737, 0.561, 0.561));
        c.add_color("SandyBrown", rgb(0.957, 0.643, 0.376));
        c.add_color("Goldenrod", rgb(0.855, 0.647, 0.125));
        c.add_color("DarkGoldenrod", rgb(0.722, 0.525, 0.0431));
        c.add_color("Peru", rgb(0.804, 0.522, 0.247));
        c.add_color("Chocolate", rgb(0.824, 0.412, 0.118));
        c.add_color("SaddleBrown", rgb(0.545, 0.271, 0.0745));
        c.add_color("Sienna", rgb(0.627, 0.322, 0.176));
        c.add_color("Snow", rgb(1.0, 0.98, 0.98));
        c.add_color("HoneyDew", rgb(0.941, 1.0, 0.941));
        c.add_color("MintCream", rgb(0.961, 1.0, 0.98));
        c.add_color("Azure", rgb(0.941, 1.0, 1.0));
        c.add_color("AliceBlue", rgb(0.941, 0.973, 1.0));
        c.add_color("GhostWhite", rgb(0.973, 0.973, 1.0));
        c.add_color("WhiteSmoke", rgb(0.961, 0.961, 0.961));
        c.add_color("SeaShell", rgb(1.0, 0.961, 0.933));
        c.add_color("Beige", rgb(0.961, 0.961, 0.863));
        c.add_color("OldLace", rgb(0.992, 0.961, 0.902));
        c.add_color("FloralWhite", rgb(1.0, 0.98, 0.941));
        c.add_color("Ivory", rgb(1.0, 1.0, 0.941));
        c.add_color("AntiqueWhite", rgb(0.98, 0.922, 0.843));
        c.add_color("Linen", rgb(0.98, 0.941, 0.902));
        c.add_color("LavenderBlush", rgb(1.0, 0.941, 0.961));
        c.add_color("MistyRose", rgb(1.0, 0.894, 0.882));
        c.add_color("Grenadine", rgb(0.674, 0.329, 0.368));
        c.add_color("Cola", rgb(0.235, 0.184, 0.137));
        c.add_color("Ginger", rgb(0.69, 0.396, 0.0));
        c.add_color("FruitPunch", rgb(0.807, 0.239, 0.282));
        c
    }

    fn intern(&mut self, color: Color) -> usize {
        self.pool.push(color);
        self.pool.len() - 1
    }

    /// zombie.core.Colors.addColor
    fn add_color(&mut self, name: &str, color: Color) -> usize {
        let color = self.intern(color);
        let nfo = ColNfo {
            color_set: ColorSet::Standard,
            name: name.to_string(),
            color,
        };
        self.info_map.insert(name.to_string(), nfo.clone());
        self.info_list.push(nfo);
        self.colors.push(color);
        self.color_map.insert(java::to_lower_case(name), color);
        self.color_names.push(name.to_string());
        self.color_set.insert(java::to_lower_case(name));
        color
    }

    /// zombie.core.Colors.AddGameColor: an existing game colour of that name takes the new values in place;
    /// otherwise a new game colour (replacing a standard one of the same name in the name tables).
    pub fn add_game_color(&mut self, name: &str, color: Color) -> usize {
        if let Some(nfo) = self.info_map.get(name) {
            if nfo.color_set == ColorSet::Game {
                let i = nfo.color;
                self.pool[i] = color;
                return i;
            }
        }
        let color = self.intern(color);
        let nfo = ColNfo {
            color_set: ColorSet::Game,
            name: name.to_string(),
            color,
        };
        self.info_map.insert(name.to_string(), nfo.clone());
        self.info_list.push(nfo);
        self.color_map.insert(java::to_lower_case(name), color);
        self.color_names.push(name.to_string());
        self.color_set.insert(java::to_lower_case(name));
        color
    }

    /// zombie.core.Colors.addColorCB
    fn add_color_cb(&mut self, name: &str, color: Color) -> usize {
        let color = self.intern(color);
        let nfo = ColNfo {
            color_set: ColorSet::ColorBlind,
            name: name.to_string(),
            color,
        };
        self.info_list.push(nfo.clone());
        self.cb_info_map.insert(name.to_string(), nfo);
        self.cb_colors.push(color);
        self.cb_color_map.insert(java::to_lower_case(name), color);
        self.cb_color_names.push(name.to_string());
        self.cb_color_set.insert(java::to_lower_case(name));
        color
    }

    /// zombie.core.Colors.GetColorInfo
    pub fn get_color_info(&self, name: &str) -> Option<&ColNfo> {
        self.info_map.get(name)
    }

    /// ColNfo.getColor().set(color): the shared instance takes the values.
    pub fn set_color(&mut self, index: usize, color: Color) {
        self.pool[index] = color;
    }

    /// The colour at an index of the pool (`ColNfo.getColor()`).
    pub fn color_at(&self, index: usize) -> Color {
        self.pool[index]
    }

    /// zombie.core.Colors.GetColorByName: by lower-cased name.
    pub fn get_color_by_name(&self, name: &str) -> Option<Color> {
        self.color_map
            .get(&java::to_lower_case(name))
            .map(|&i| self.pool[i])
    }

    /// zombie.core.Colors.GetColorNames
    pub fn get_color_names(&self) -> &[String] {
        &self.color_names
    }

    /// zombie.core.Colors.GetColorsCount: the standard colours only.
    pub fn get_colors_count(&self) -> usize {
        self.colors.len()
    }

    /// zombie.core.Colors.ColorExists
    pub fn color_exists(&self, name: &str) -> bool {
        self.color_set.contains(&java::to_lower_case(name))
    }

    /// zombie.core.Colors.CB_GetColorByName
    pub fn cb_get_color_by_name(&self, name: &str) -> Option<Color> {
        self.cb_color_map
            .get(&java::to_lower_case(name))
            .map(|&i| self.pool[i])
    }

    /// zombie.core.Colors.CB_GetColorNames
    pub fn cb_get_color_names(&self) -> &[String] {
        &self.cb_color_names
    }

    /// zombie.core.Colors.CB_ColorExists
    pub fn cb_color_exists(&self, name: &str) -> bool {
        self.cb_color_set.contains(&java::to_lower_case(name))
    }

    /// The ColNfo of every colour in creation order (`infoList`).
    pub fn info_list(&self) -> &[ColNfo] {
        &self.info_list
    }

    /// The ColNfo of a colour-blind colour (`CB_infoMap`).
    pub fn cb_info(&self, name: &str) -> Option<&ColNfo> {
        self.cb_info_map.get(name)
    }
}
