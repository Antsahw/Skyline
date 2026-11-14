//! Structures of a type the generator can't make itself, made by someone who can.
//!
//! A mod's own structure type (a Fabric 1.20.1 mod's `deeperdarker:maze`) has no Rust
//! implementation: its pieces are the mod's code. When the game runs those mods somewhere (the
//! 1.20.1 runtime), it installs a [`ForeignStructures`]: where one of the structure's sets puts a
//! start, the generator asks it for the start (`Structure.findValidGenerationPoint`, heights from
//! this generator), keeps the pieces' boxes as [`ForeignPiece`]s (for the chunks they reach,
//! the starts' references and the biome check, as its own starts'), and when a chunk they reach
//! is decorated it asks it to place the start there (`StructureStart.placeInChunk`) in the region
//! being decorated. Without one, such structures don't generate, as before.
use super::BoundingBox;
use super::legacy::{LegacyPiece, PieceContext, Result};
use crate::heightmap::HeightmapType;
use crate::worldgen::foreign::ForeignWorld;
use std::sync::{Arc, RwLock};

/// A start someone else made and keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignStart {
    /// What the maker keeps it under.
    pub handle: i64,
    /// `GenerationStub.position`: where its biome is checked.
    pub position: [i32; 3],
    /// Each piece's box: min x, y, z, max x, y, z.
    pub boxes: Vec<[i32; 6]>,
}

/// What making a start asks of this generator.
pub trait ForeignStartWorld {
    /// `ChunkGenerator.getBaseHeight`: a heightmap's height (`WORLD_SURFACE_WG`...) of the
    /// terrain this generator makes at a column.
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32>;
}

/// Whoever makes and places the structures the generator can't.
pub trait ForeignStructures: Send + Sync {
    /// Whether it makes structures of this type.
    fn generates(&self, structure_type: &str) -> bool;
    /// The start of `structure` in `chunk`, if it has one there.
    fn start(
        &self,
        structure: &str,
        world_seed: i64,
        chunk: [i32; 2],
        world: &mut dyn ForeignStartWorld,
    ) -> Option<ForeignStart>;
    /// Places a start it made in `chunk`, being decorated (`step`: the structure's generation
    /// step); whether it placed it.
    fn place(
        &self,
        handle: i64,
        world_seed: i64,
        chunk: [i32; 2],
        step: i32,
        world: &mut dyn ForeignWorld,
    ) -> bool;
}

static FOREIGN: RwLock<Option<Arc<dyn ForeignStructures>>> = RwLock::new(None);

/// Installs (or, with `None`, removes) the maker of foreign structures.
pub fn set_foreign_structures(maker: Option<Arc<dyn ForeignStructures>>) {
    *FOREIGN.write().unwrap_or_else(|e| e.into_inner()) = maker;
}

/// The maker installed now, if it makes structures of `structure_type`.
pub fn foreign_structures(structure_type: &str) -> Option<Arc<dyn ForeignStructures>> {
    FOREIGN
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
        .filter(|f| f.generates(structure_type))
}

/// One piece of a foreign start: its box. The first places the whole start in each chunk it's
/// decorated in (`after_place`, which runs once per chunk on the first piece), as
/// `StructureStart.placeInChunk` places every piece reaching the chunk.
#[derive(Debug)]
pub struct ForeignPiece {
    pub handle: i64,
    pub bounding_box: BoundingBox,
    pub first: bool,
    pub step: i32,
    pub world_seed: i64,
}

impl LegacyPiece for ForeignPiece {
    fn bounding_box(&self) -> BoundingBox {
        self.bounding_box
    }
    fn type_name(&self) -> &'static str {
        "horizon:foreign"
    }
    fn post_process(&self, _context: &mut PieceContext<'_>) -> Result<()> {
        Ok(())
    }
    fn after_place(&self, context: &mut PieceContext<'_>, _pieces: &[BoundingBox]) -> Result<()> {
        if !self.first {
            return Ok(());
        }
        let Some(maker) = FOREIGN.read().unwrap_or_else(|e| e.into_inner()).clone() else {
            return Ok(());
        };
        let chunk = [context.chunk.x, context.chunk.z];
        let mut world = PieceRegion {
            context,
            failed: None,
        };
        maker.place(self.handle, self.world_seed, chunk, self.step, &mut world);
        match world.failed {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// The chunk a piece is placed in, as a [`ForeignWorld`]: states by name.
struct PieceRegion<'c, 'a> {
    context: &'c mut PieceContext<'a>,
    /// The region's own failure (a chunk not there): the placement's, after the call.
    failed: Option<crate::worldgen::placement::PlacementQueryError>,
}

impl PieceRegion<'_, '_> {
    fn keep<T>(&mut self, r: Result<T>) -> Option<T> {
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                self.failed.get_or_insert(e);
                None
            }
        }
    }
}

impl ForeignWorld for PieceRegion<'_, '_> {
    fn block(&mut self, position: [i32; 3]) -> Option<String> {
        let r = self.context.world.block(position);
        let id = self.keep(r)?;
        crate::worldgen::foreign::state_string(self.context.resources.registry, id)
    }
    fn biome(&mut self, _position: [i32; 3]) -> Option<String> {
        // A piece's region has biome ids, not names: the maker's default.
        None
    }
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32> {
        let kind = HeightmapType::ALL
            .into_iter()
            .find(|k| k.name() == heightmap)?;
        let r = self.context.world.height(kind, x, z);
        self.keep(r)
    }
    fn set_block(&mut self, position: [i32; 3], state: &str) -> bool {
        if !self.context.chunk_box.is_inside(position) {
            return false;
        }
        let Some(id) =
            crate::worldgen::foreign::parse_state_string(self.context.resources.registry, state)
        else {
            return false;
        };
        let r = self.context.world.set(position, id, 2);
        self.keep(r).unwrap_or(false)
    }
    fn set_block_entity(&mut self, position: [i32; 3], nbt: crate::nbt::Compound) -> bool {
        if !self.context.chunk_box.is_inside(position) {
            return false;
        }
        let r = self.context.world.block_entity(position, nbt);
        self.keep(r).is_some()
    }
}
