//! zombie.erosion.obj.ErosionObj: an object the erosion code places on a square and moves through its stages.
use super::erosion_obj_sprites::{Entry, ErosionObjSprites};
use crate::zombie::core::tile_property_alias_map::TilePropertyAliasMap;
use crate::zombie::iso::iso_grid_square::IsoGridSquare;
use crate::zombie::iso::iso_object::{IsoObject, IsoTree, ObjectKind};
use crate::zombie::iso::sprite::iso_sprite_manager::{IsoSpriteManager, SpriteRef};
use crate::zombie::iso::sprite_details::iso_object_type::IsoObjectType;

/// zombie.iso.objects.IsoTree.LOGS_PER_SIZE
const LOGS_PER_SIZE: [i32; 8] = [1, 1, 2, 3, 4, 5, 6, 8];

/// zombie.erosion.obj.ErosionObj
#[derive(Clone, Debug)]
pub struct ErosionObj {
    sprites: ErosionObjSprites,
    pub name: String,
    pub stages: i32,
    pub has_snow: bool,
    pub has_flower: bool,
    pub has_child_sprite: bool,
    pub bloom_start: f32,
    pub bloom_end: f32,
    pub no_season_base: bool,
    pub cycle_time: i32,
}

/// zombie.iso.objects.IsoTree.initTree on a tree whose sprite is already set: the sprite's tile type becomes `tree`
/// (the sprite is shared), then the size from the sprite's `tree` property and the log yield and damage from it.
/// `Err` where the original throws (no sprite, or a `tree` property that is not a number).
fn init_tree(
    sm: &mut IsoSpriteManager,
    alias: &TilePropertyAliasMap,
    obj: &mut IsoObject,
) -> Result<(), String> {
    let Some(spr) = obj.sprite else {
        return Err("java.lang.NullPointerException: IsoTree.initTree without a sprite".into());
    };
    sm.sprite_mut(spr).set_tile_type(IsoObjectType::tree);
    let prop = sm
        .sprite(spr)
        .properties
        .get(alias, "tree")
        .map(str::to_string);
    let size = match prop {
        Some(v) => {
            let n = crate::java::parse_int(&v).ok_or_else(|| {
                format!("java.lang.NumberFormatException: For input string: \"{v}\"")
            })?;
            n.clamp(1, 8)
        }
        None => 4,
    };
    let log_yield = LOGS_PER_SIZE[(size - 1) as usize];
    let damage = ((log_yield - 1) * 80).max(40);
    obj.kind = ObjectKind::IsoTree(IsoTree {
        size,
        log_yield,
        damage,
    });
    Ok(())
}

impl ErosionObj {
    /// zombie.erosion.obj.ErosionObj constructor
    pub fn new(
        sprites: ErosionObjSprites,
        cycle_time: i32,
        bloom_start: f32,
        bloom_end: f32,
        no_season_base: bool,
    ) -> Self {
        ErosionObj {
            name: sprites.name.clone(),
            stages: sprites.stages,
            has_snow: sprites.has_snow,
            has_flower: sprites.has_flower,
            has_child_sprite: sprites.has_child_sprite,
            bloom_start,
            bloom_end,
            no_season_base,
            cycle_time,
            sprites,
        }
    }

    /// zombie.erosion.obj.ErosionObj.getObject: the topmost object of the square that carries this object's name
    /// (and takes it out of the square when `remove`).
    pub fn get_object(&self, square: &mut IsoGridSquare, remove: bool) -> Option<usize> {
        for i in (0..square.objects.len()).rev() {
            if square.objects[i].name.as_deref() == Some(self.name.as_str()) {
                if remove {
                    square.objects.remove(i);
                }
                return Some(i);
            }
        }
        None
    }

    /// zombie.erosion.obj.ErosionObj.createObject: a new object (a tree for `tree`) with the sprite of the stage and
    /// season, named like this erosion object. The square it is made for is not needed until it is placed.
    pub fn create_object(
        &mut self,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        stage: i32,
        tree: bool,
        season: i32,
    ) -> Result<IsoObject, String> {
        let sprite_name = self
            .sprites
            .get_base(stage, if self.no_season_base { 0 } else { season })
            .unwrap_or_default();
        let mut obj = if tree {
            let mut o = IsoObject::new(
                ObjectKind::IsoTree(IsoTree::default()),
                sm.named_map.get(&sprite_name).copied(),
            );
            init_tree(sm, alias, &mut o)?;
            o
        } else {
            let mut o = IsoObject::new(
                ObjectKind::IsoObject,
                sm.named_map.get(&sprite_name).copied(),
            );
            o.tile = Some(sprite_name);
            o
        };
        obj.name = Some(self.name.clone());
        Ok(obj)
    }

    /// zombie.erosion.obj.ErosionObj.placeObject: makes the object and sets its stage; the caller puts it on the
    /// square (`getObjects().add` for a plain object, `AddTileObject` for a tree). `None` where the original
    /// returns false.
    pub fn place_object(
        &mut self,
        sm: &mut IsoSpriteManager,
        alias: &TilePropertyAliasMap,
        stage: i32,
        tree: bool,
        season: i32,
        bloom: bool,
    ) -> Result<Option<IsoObject>, String> {
        let mut obj = self.create_object(sm, alias, stage, tree, season)?;
        if self.set_stage_object(sm, stage, &mut obj, season, bloom) {
            Ok(Some(obj))
        } else {
            Ok(None)
        }
    }

    fn get_sprite(sm: &mut IsoSpriteManager, name: &str) -> SpriteRef {
        sm.get_sprite(name)
    }

    /// zombie.erosion.obj.ErosionObj.setStageObject
    pub fn set_stage_object(
        &mut self,
        sm: &mut IsoSpriteManager,
        stage: i32,
        obj: &mut IsoObject,
        season: i32,
        bloom: bool,
    ) -> bool {
        if stage >= 0 && stage < self.stages {
            let sprite_name = self
                .sprites
                .get_base(stage, if self.no_season_base { 0 } else { season });
            let Some(sprite_name) = sprite_name else {
                obj.sprite = Some(Self::get_sprite(sm, ""));
                if let Some(a) = obj.attached_anim_sprite.as_mut() {
                    a.clear();
                }
                return true;
            };
            obj.sprite = Some(Self::get_sprite(sm, &sprite_name));
            if self.has_child_sprite || self.has_flower {
                let list = obj.attached_anim_sprite.get_or_insert_with(Vec::new);
                list.clear();
                if self.has_child_sprite && season != 0 {
                    if let Some(n) = self.sprites.get_child_sprite(stage, season) {
                        list.push(Self::get_sprite(sm, &n));
                    }
                }
                if self.has_flower && bloom {
                    if let Some(n) = self.sprites.get_flower(stage) {
                        list.push(Self::get_sprite(sm, &n));
                    }
                }
            }
            true
        } else {
            false
        }
    }

    /// zombie.erosion.obj.ErosionObj.getEntry
    pub fn get_entry(&self, sprite: &str) -> Option<Entry> {
        self.sprites.get_entry(sprite)
    }
}
