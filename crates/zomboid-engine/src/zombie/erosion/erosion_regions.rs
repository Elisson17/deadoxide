//! zombie.erosion.ErosionRegions: the four regions (which tiles a category of objects may sit on) and their
//! categories.
use super::categories::erosion_category::ErosionCategory;
use super::categories::flowerbed::Flowerbed;
use super::categories::nature_bush::NatureBush;
use super::categories::nature_generic::NatureGeneric;
use super::categories::nature_plants::NaturePlants;
use super::categories::nature_trees::NatureTrees;
use super::categories::street_cracks::StreetCracks;
use super::categories::wall_cracks::WallCracks;
use super::categories::wall_vines::WallVines;
use super::season::erosion_ice_queen::ErosionIceQueen;
use crate::zombie::iso::sprite::iso_sprite_manager::IsoSpriteManager;

/// zombie.erosion.ErosionRegions.Region
pub struct Region {
    pub id: i32,
    pub tile_name_match: Option<String>,
    pub check_exterior: bool,
    pub is_exterior: bool,
    pub has_wall: bool,
    pub categories: Vec<Box<dyn ErosionCategory>>,
}

impl Region {
    /// zombie.erosion.ErosionRegions.Region constructor
    pub fn new(
        id: i32,
        tile_match: Option<&str>,
        check_ext: bool,
        is_ext: bool,
        has_wall: bool,
    ) -> Self {
        Region {
            id,
            tile_name_match: tile_match.map(str::to_string),
            check_exterior: check_ext,
            is_exterior: is_ext,
            has_wall,
            categories: Vec::new(),
        }
    }

    /// zombie.erosion.ErosionRegions.Region.addCategory
    pub fn add_category(mut self, id: i32, mut cat: Box<dyn ErosionCategory>) -> Self {
        cat.base_mut().id = id;
        cat.base_mut().region = self.id;
        self.categories.push(cat);
        self
    }

    /// zombie.erosion.ErosionRegions.Region.init
    pub fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen) {
        for c in &mut self.categories {
            c.init(sm, ice_queen);
        }
    }
}

/// zombie.erosion.ErosionRegions: the static list of regions.
#[derive(Default)]
pub struct ErosionRegions {
    pub regions: Vec<Region>,
}

impl ErosionRegions {
    /// zombie.erosion.ErosionRegions.addRegion
    fn add_region(&mut self, mut region: Region) {
        region.id = self.regions.len() as i32;
        self.regions.push(region);
    }

    /// zombie.erosion.ErosionRegions.getCategory
    pub fn get_category(
        &mut self,
        region_id: i32,
        category_id: i32,
    ) -> Result<&mut Box<dyn ErosionCategory>, String> {
        self.regions
            .get_mut(region_id as usize)
            .and_then(|r| r.categories.get_mut(category_id as usize))
            .ok_or_else(|| {
                format!("java.lang.IndexOutOfBoundsException: erosion category {region_id}/{category_id}")
            })
    }

    /// zombie.erosion.ErosionRegions.init
    pub fn init(&mut self, sm: &mut IsoSpriteManager, ice_queen: &mut ErosionIceQueen) {
        self.regions.clear();
        self.add_region(
            Region::new(0, Some("blends_natural_01"), true, true, false)
                .add_category(0, Box::<NatureTrees>::default())
                .add_category(1, Box::<NatureBush>::default())
                .add_category(2, Box::<NaturePlants>::default())
                .add_category(3, Box::<NatureGeneric>::default()),
        );
        self.add_region(
            Region::new(1, Some("blends_street"), true, true, false)
                .add_category(0, Box::<StreetCracks>::default()),
        );
        self.add_region(
            Region::new(2, None, false, false, true)
                .add_category(0, Box::<WallVines>::default())
                .add_category(1, Box::<WallCracks>::default()),
        );
        self.add_region(
            Region::new(3, None, true, true, false).add_category(0, Box::<Flowerbed>::default()),
        );
        for r in &mut self.regions {
            r.init(sm, ice_queen);
        }
    }
}
