//! zombie.erosion.obj.ErosionObjSprites: the sprite names of an erosion object by stage, section and season.
use std::collections::HashMap;

/// zombie.erosion.obj.ErosionObjSprites.SECTION_BASE
pub const SECTION_BASE: usize = 0;
/// zombie.erosion.obj.ErosionObjSprites.SECTION_FLOWER
pub const SECTION_FLOWER: usize = 2;
/// zombie.erosion.obj.ErosionObjSprites.SECTION_CHILD
pub const SECTION_CHILD: usize = 3;

/// zombie.erosion.obj.ErosionObjSprites.Entry (a record of stage, section and season).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub stage: i32,
    pub section: i32,
    pub season: i32,
}

/// zombie.erosion.obj.ErosionObjSprites.Sprites: one name or a list the sprite is picked from round robin.
#[derive(Clone, Debug)]
struct Sprites {
    sprites: Vec<String>,
    index: i32,
}

impl Sprites {
    fn new(sprites: Vec<String>) -> Self {
        Sprites { sprites, index: -1 }
    }

    /// zombie.erosion.obj.ErosionObjSprites.Sprites.getNext
    fn get_next(&mut self) -> &str {
        self.index += 1;
        if self.index >= self.sprites.len() as i32 {
            self.index = 0;
        }
        &self.sprites[self.index as usize]
    }
}

/// zombie.erosion.obj.ErosionObjSprites.Section: the sprites of one section by season (six seasons).
#[derive(Clone, Debug, Default)]
struct Section {
    seasons: [Option<Sprites>; 6],
}

/// zombie.erosion.obj.ErosionObjSprites.Stage: four sections, each present or not.
#[derive(Clone, Debug, Default)]
struct Stage {
    sections: [Option<Section>; 4],
}

/// zombie.erosion.obj.ErosionObjSprites
#[derive(Clone, Debug)]
pub struct ErosionObjSprites {
    pub name: String,
    pub stages: i32,
    pub has_snow: bool,
    pub has_flower: bool,
    pub has_child_sprite: bool,
    pub no_season_base: bool,
    pub cycle_time: i32,
    rev_lookup: HashMap<String, Entry>,
    sprites: Vec<Stage>,
}

impl ErosionObjSprites {
    /// zombie.erosion.obj.ErosionObjSprites constructor
    pub fn new(
        stages: i32,
        name: &str,
        has_snow: bool,
        has_flower: bool,
        has_child_sprite: bool,
    ) -> Self {
        let mut sprites = Vec::new();
        for _ in 0..stages {
            let mut stage = Stage::default();
            stage.sections[0] = Some(Section::default());
            if has_snow {
                stage.sections[1] = Some(Section::default());
            }
            if has_flower {
                stage.sections[2] = Some(Section::default());
            }
            if has_child_sprite {
                stage.sections[3] = Some(Section::default());
            }
            sprites.push(stage);
        }
        ErosionObjSprites {
            name: name.to_string(),
            stages,
            has_snow,
            has_flower,
            has_child_sprite,
            no_season_base: false,
            cycle_time: 1,
            rev_lookup: HashMap::new(),
            sprites,
        }
    }

    /// zombie.erosion.obj.ErosionObjSprites.getSprite (int, int, int). The original throws
    /// ArrayIndexOutOfBoundsException for a stage or season outside the arrays; the erosion categories keep to the
    /// bounds, so `None` stands for the original's `null` only.
    fn get_sprite(&mut self, stage: i32, section: usize, season: i32) -> Option<String> {
        let st = self.sprites.get_mut(stage as usize)?;
        let sec = st.sections.get_mut(section)?.as_mut()?;
        let s = sec.seasons.get_mut(season as usize)?.as_mut()?;
        Some(s.get_next().to_string())
    }

    /// zombie.erosion.obj.ErosionObjSprites.getBase
    pub fn get_base(&mut self, stage: i32, season: i32) -> Option<String> {
        self.get_sprite(stage, 0, season)
    }

    /// zombie.erosion.obj.ErosionObjSprites.getFlower
    pub fn get_flower(&mut self, stage: i32) -> Option<String> {
        if self.has_flower {
            self.get_sprite(stage, 2, 0)
        } else {
            None
        }
    }

    /// zombie.erosion.obj.ErosionObjSprites.getChildSprite
    pub fn get_child_sprite(&mut self, stage: i32, season: i32) -> Option<String> {
        if self.has_child_sprite {
            self.get_sprite(stage, 3, season)
        } else {
            None
        }
    }

    /// zombie.erosion.obj.ErosionObjSprites.getEntry
    pub fn get_entry(&self, sprite: &str) -> Option<Entry> {
        self.rev_lookup.get(sprite).copied()
    }

    /// zombie.erosion.obj.ErosionObjSprites.setSprite (int, int, String, int)
    fn set_sprite(&mut self, stage: i32, section: usize, sprite: &str, season: i32) {
        if let Some(sec) = self
            .sprites
            .get_mut(stage as usize)
            .and_then(|s| s.sections[section].as_mut())
        {
            sec.seasons[season as usize] = Some(Sprites::new(vec![sprite.to_string()]));
            self.rev_lookup.insert(
                sprite.to_string(),
                Entry {
                    stage,
                    section: section as i32,
                    season,
                },
            );
        }
    }

    /// zombie.erosion.obj.ErosionObjSprites.setSprite (int, int, ArrayList, int)
    fn set_sprite_list(&mut self, stage: i32, section: usize, sprites: &[String], season: i32) {
        if let Some(sec) = self
            .sprites
            .get_mut(stage as usize)
            .and_then(|s| s.sections[section].as_mut())
        {
            sec.seasons[season as usize] = Some(Sprites::new(sprites.to_vec()));
            for s in sprites {
                self.rev_lookup.insert(
                    s.clone(),
                    Entry {
                        stage,
                        section: section as i32,
                        season,
                    },
                );
            }
        }
    }

    /// zombie.erosion.obj.ErosionObjSprites.setBase (int, String, int)
    pub fn set_base(&mut self, stage: i32, sprite: &str, season: i32) {
        self.set_sprite(stage, SECTION_BASE, sprite, season);
    }

    /// zombie.erosion.obj.ErosionObjSprites.setBase (int, ArrayList, int)
    pub fn set_base_list(&mut self, stage: i32, sprites: &[String], season: i32) {
        self.set_sprite_list(stage, SECTION_BASE, sprites, season);
    }

    /// zombie.erosion.obj.ErosionObjSprites.setFlower (int, String)
    pub fn set_flower(&mut self, stage: i32, sprite: &str) {
        self.set_sprite(stage, SECTION_FLOWER, sprite, 0);
    }

    /// zombie.erosion.obj.ErosionObjSprites.setChildSprite (int, String, int)
    pub fn set_child_sprite(&mut self, stage: i32, sprite: &str, season: i32) {
        self.set_sprite(stage, SECTION_CHILD, sprite, season);
    }
}
