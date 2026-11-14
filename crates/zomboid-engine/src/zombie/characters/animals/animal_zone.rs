//! zombie.characters.animals.AnimalZone: a zone of type "Animal" (`map_animals.bin`): a path or area animals use,
//! with the action they take there (`Follow`, `Eat`, `Sleep`...), the animal type, and the junctions where its
//! polyline meets another animal zone's.
use super::animal_zone_junction::AnimalZoneJunction;
use crate::java_nio::{read_string, ByteBuffer};
use crate::zombie::iso::zones::zone::Zone;

/// zombie.characters.animals.AnimalZone (a `Zone` with the animal fields).
#[derive(Clone, Debug, PartialEq)]
pub struct AnimalZone {
    pub zone: Zone,
    pub animal_type: Option<String>,
    pub action: Option<String>,
    /// zombie.characters.animals.AnimalZone.junctions: null until the first junction is added.
    pub junctions: Option<Vec<AnimalZoneJunction>>,
    pub spawned_animals: bool,
    pub spawn_animal: bool,
}

impl Default for AnimalZone {
    /// zombie.characters.animals.AnimalZone(): `spawnAnimal` starts true.
    fn default() -> Self {
        AnimalZone {
            zone: Zone::default(),
            animal_type: None,
            action: None,
            junctions: None,
            spawned_animals: false,
            spawn_animal: true,
        }
    }
}

/// org.joml.Vector2f.length(float, float)
fn vector2f_length(x: f32, y: f32) -> f32 {
    f64::from(x * x + y * y).sqrt() as f32
}

impl AnimalZone {
    /// zombie.characters.animals.AnimalZone.load(ByteBuffer, int, Map, SharedStrings): `Zone.load`, then the action
    /// and animal type (`GameWindow.ReadString`, never null after a load) and the two flags.
    pub fn load(
        b: &mut ByteBuffer,
        world_version: i32,
        string_map: &[String],
    ) -> Result<Self, String> {
        let zone = Zone::load(b, world_version, string_map)?;
        let action = Some(read_string(b)?);
        let animal_type = Some(read_string(b)?);
        let spawn_animal = b.get()? != 0;
        let spawned_animals = b.get()? != 0;
        Ok(AnimalZone {
            zone,
            animal_type,
            action,
            junctions: None,
            spawned_animals,
            spawn_animal,
        })
    }

    /// zombie.characters.animals.AnimalZone.getDistanceOfPointFromStart: the polyline's length up to point `index`,
    /// through square centres, in float. `Err` where the original reads past the points
    /// (`ArrayIndexOutOfBoundsException` from `TIntArrayList.get`).
    pub fn get_distance_of_point_from_start(&self, point_index: i32) -> Result<f32, String> {
        let p = &self.zone.points;
        let n = p.len();
        let get = |i: usize| {
            p.get(i)
                .copied()
                .ok_or_else(|| format!("java.lang.ArrayIndexOutOfBoundsException: Index {i} out of bounds for length {n}"))
        };
        let mut length = 0.0f32;
        let mut i = 0i64;
        while i < i64::from(point_index) * 2 {
            let iu = i as usize;
            let x1 = get(iu)? as f32 + 0.5;
            let y1 = get(iu + 1)? as f32 + 0.5;
            let x2 = get((iu + 2) % n.max(1))? as f32 + 0.5;
            let y2 = get((iu + 3) % n.max(1))? as f32 + 0.5;
            length += vector2f_length(x2 - x1, y2 - y1);
            i += 2;
        }
        Ok(length)
    }

    /// zombie.characters.animals.AnimalZone.Dispose
    pub fn dispose(&mut self) {
        self.zone.dispose();
        self.animal_type = None;
        self.action = None;
        self.junctions = None;
        self.spawned_animals = false;
        self.spawn_animal = true;
    }

    /// zombie.characters.animals.AnimalZone.addJunction(AnimalZoneJunction): `ArrayList.contains` compares by
    /// identity, so a junction just loaded is always new.
    pub fn add_junction(&mut self, junction: AnimalZoneJunction) {
        self.junctions.get_or_insert_with(Vec::new).push(junction);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A polyline through square centres (0,0) -> (3,4) -> (3,10): 5 squares to the second point, 5 + 6 to the third;
    /// a point index past the points is the original's out-of-bounds exception.
    #[test]
    fn distance_from_start_walks_the_polyline() {
        let mut z = AnimalZone::default();
        z.zone.points = vec![0, 0, 3, 4, 3, 10];
        assert_eq!(z.get_distance_of_point_from_start(0), Ok(0.0));
        assert_eq!(z.get_distance_of_point_from_start(1), Ok(5.0));
        assert_eq!(z.get_distance_of_point_from_start(2), Ok(11.0));
        assert!(z.get_distance_of_point_from_start(4).is_err());
    }
}
