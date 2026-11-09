//! Horizon's world for 1.20.1 mods' code (docs/java-bridge-120.md, phase 3): what Horizon asks
//! the 1.20.1 runtime to do in its world (a right-click on a block), and what the runtime asks
//! back while doing it. platform120's `Mirror120` is the other side: the mods' code runs on a
//! 1.20.1 server level whose blocks are Horizon's ([`World120::block_state`]), and what it sets
//! comes back as [`SetBlock`]s for Horizon to apply when the call returns.
//!
//! Block states cross as their names (`farmersdelight:stove[facing=north,lit=false]`): Horizon's
//! names going in (26.3's), 1.20.1's coming back. A property one side lacks is dropped on the way.

use crate::boot120::Runtime120;
use crate::child::Log;
use crate::peer::{Handler, Peer};
use crate::wire::Value;

/// What the 1.20.1 runtime reads of Horizon's world.
pub trait World120 {
    /// The block state at a position of `dimension` (`minecraft:overworld`), by name; `None`
    /// where Horizon has no chunk loaded (read as air).
    fn block_state(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String>;

    /// The block entity at a position as vanilla saves it (`saveWithFullMetadata`: with `id`
    /// and the position), in SNBT; `None` where there is none. A 1.20.1 mod's own block
    /// entity is Horizon's opaque NBT (`BlockEntity::Mod`), so a cooking pot's items and
    /// progress come back as the mod wrote them.
    fn block_entity(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        None
    }
}

/// A block the mods' code set, for Horizon to apply.
#[derive(Clone, Debug, PartialEq)]
pub struct SetBlock {
    pub dimension: String,
    pub pos: [i32; 3],
    /// 1.20.1's name of the state.
    pub state: String,
    /// Level.setBlock's flags (1 neighbours, 2 clients, ...).
    pub flags: i32,
}

/// An item the mods' code dropped into the world, for Horizon to spawn.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnItem {
    pub dimension: String,
    /// Where it was dropped, in world coordinates.
    pub pos: [f64; 3],
    /// Its motion when dropped (`Entity.getDeltaMovement`), blocks per tick.
    pub motion: [f64; 3],
    /// The item's id (`minecraft:music_disc_13`).
    pub item: String,
    pub count: i32,
    pub damage: i32,
}

/// A block entity the mods' code changed, as it saved it, for Horizon to keep.
#[derive(Clone, Debug, PartialEq)]
pub struct SetBlockEntity {
    pub dimension: String,
    pub pos: [i32; 3],
    /// `saveWithFullMetadata` in SNBT, with `id` naming the type.
    pub snbt: String,
}

/// A player's right-click on a block, as vanilla's ServerPlayerGameMode.useItemOn gets it.
#[derive(Clone, Debug)]
pub struct UseBlock {
    pub uuid: String,
    pub name: String,
    pub dimension: String,
    pub pos: [i32; 3],
    /// The face hit, as Direction's 3D data value (0 down, 1 up, 2 north, 3 south, 4 west, 5 east).
    pub face: u8,
    /// Where the ray hit, in world coordinates. The mirror player's eyes are put there, so
    /// vanilla's reach check passes (Horizon checked reach already).
    pub hit: [f64; 3],
    /// 0 main hand, 1 off hand.
    pub hand: u8,
    /// The held item's id (`minecraft:flint_and_steel`), "" for none.
    pub item: String,
    pub count: i32,
    pub damage: i32,
    pub creative: bool,
}

/// What a tick of the mods' block entities did.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Ticked {
    /// How many of the positions had a block entity with a ticker that ran.
    pub ticked: i32,
    /// The blocks the ticking code set, in order.
    pub set: Vec<SetBlock>,
    /// The block entities it changed, in order.
    pub block_entities: Vec<SetBlockEntity>,
    /// The items it dropped or ejected, in order.
    pub spawned: Vec<SpawnItem>,
}

/// What a right-click did.
#[derive(Clone, Debug, PartialEq)]
pub struct Used {
    /// 0 passed (nothing used: Horizon may try the item's own use), 1 consumed, 2 consumed
    /// with the arm swung.
    pub result: i32,
    /// The held item after: id, count, damage.
    pub item: String,
    pub count: i32,
    pub damage: i32,
    /// The blocks set, in order.
    pub set: Vec<SetBlock>,
    /// The block entities the mods' code changed, in order.
    pub block_entities: Vec<SetBlockEntity>,
    /// The items the mods' code dropped, in order.
    pub spawned: Vec<SpawnItem>,
}

/// Horizon's side while the runtime runs a call in the world.
struct InWorld<'a> {
    world: &'a dyn World120,
    set: Vec<SetBlock>,
    block_entities: Vec<SetBlockEntity>,
    spawned: Vec<SpawnItem>,
    log: Log,
}

fn int(args: &[Value], i: usize) -> Result<i32, String> {
    match args.get(i) {
        Some(Value::I32(v)) => Ok(*v),
        other => Err(format!("argument {i}: an int expected, not {other:?}")),
    }
}

fn float(args: &[Value], i: usize) -> Result<f64, String> {
    match args.get(i) {
        Some(Value::F64(v)) => Ok(*v),
        other => Err(format!("argument {i}: a double expected, not {other:?}")),
    }
}

fn text(args: &[Value], i: usize) -> Result<&str, String> {
    args.get(i)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("argument {i}: text expected"))
}

impl Handler for InWorld<'_> {
    fn call(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) -> Result<Vec<Value>, String> {
        match method {
            "blockState" => {
                let dimension = text(&args, 0)?;
                let (x, y, z) = (int(&args, 1)?, int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.world
                        .block_state(dimension, x, y, z)
                        .map_or(Value::Null, Value::Str),
                ])
            }
            "blockEntity" => {
                let dimension = text(&args, 0)?;
                let (x, y, z) = (int(&args, 1)?, int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.world
                        .block_entity(dimension, x, y, z)
                        .map_or(Value::Null, Value::Str),
                ])
            }
            _ => Err(format!("Horizon has no {method} for 1.20.1 mods yet")),
        }
    }

    fn event(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) {
        match method {
            "log" => {
                if let Some(line) = args.first().and_then(Value::as_str) {
                    (self.log)(&format!("[fabric-1.20.1] {line}"));
                }
            }
            "setBlock" => {
                let parsed = (|| -> Result<SetBlock, String> {
                    Ok(SetBlock {
                        dimension: text(&args, 0)?.to_owned(),
                        pos: [int(&args, 1)?, int(&args, 2)?, int(&args, 3)?],
                        state: text(&args, 4)?.to_owned(),
                        flags: int(&args, 5)?,
                    })
                })();
                match parsed {
                    Ok(set) => self.set.push(set),
                    Err(e) => (self.log)(&format!("[fabric-1.20.1] a setBlock unread: {e}")),
                }
            }
            "setBlockEntity" => {
                let parsed = (|| -> Result<SetBlockEntity, String> {
                    Ok(SetBlockEntity {
                        dimension: text(&args, 0)?.to_owned(),
                        pos: [int(&args, 1)?, int(&args, 2)?, int(&args, 3)?],
                        snbt: text(&args, 4)?.to_owned(),
                    })
                })();
                match parsed {
                    Ok(set) => self.block_entities.push(set),
                    Err(e) => {
                        (self.log)(&format!("[fabric-1.20.1] a setBlockEntity unread: {e}"));
                    }
                }
            }
            "spawnItem" => {
                let parsed = (|| -> Result<SpawnItem, String> {
                    Ok(SpawnItem {
                        dimension: text(&args, 0)?.to_owned(),
                        pos: [float(&args, 1)?, float(&args, 2)?, float(&args, 3)?],
                        motion: [float(&args, 4)?, float(&args, 5)?, float(&args, 6)?],
                        item: text(&args, 7)?.to_owned(),
                        count: int(&args, 8)?,
                        damage: int(&args, 9)?,
                    })
                })();
                match parsed {
                    Ok(item) => self.spawned.push(item),
                    Err(e) => (self.log)(&format!("[fabric-1.20.1] a spawnItem unread: {e}")),
                }
            }
            _ => {}
        }
    }
}

impl Runtime120 {
    /// One tick of the mods' block entities at `positions` of `dimension`: their own ticking
    /// code (a cooking pot cooking), run in the runtime on `world`. What it set, changed and
    /// dropped comes back for Horizon to apply, as a right-click's does.
    pub fn tick_block_entities(
        &mut self,
        world: &dyn World120,
        dimension: &str,
        positions: &[i32],
    ) -> Result<Ticked, String> {
        let args = [
            Value::Str(dimension.to_owned()),
            Value::I32s(positions.to_vec()),
        ];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("tickBlockEntities", &args, &mut handler)?;
        Ok(Ticked {
            ticked: match values.first() {
                Some(Value::I32(v)) => *v,
                other => return Err(format!("tickBlockEntities: answer 0 is {other:?}")),
            },
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// A player's right-click on a block, run by the mods' code in the 1.20.1 runtime with
    /// `world` as its world.
    pub fn use_block(&mut self, world: &dyn World120, used: &UseBlock) -> Result<Used, String> {
        let args = [
            Value::Str(used.uuid.clone()),
            Value::Str(used.name.clone()),
            Value::Str(used.dimension.clone()),
            Value::I32(used.pos[0]),
            Value::I32(used.pos[1]),
            Value::I32(used.pos[2]),
            Value::I32(used.face.into()),
            Value::F64(used.hit[0]),
            Value::F64(used.hit[1]),
            Value::F64(used.hit[2]),
            Value::I32(used.hand.into()),
            Value::Str(used.item.clone()),
            Value::I32(used.count),
            Value::I32(used.damage),
            Value::Bool(used.creative),
        ];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("useBlock", &args, &mut handler)?;
        let int_at = |i: usize| match values.get(i) {
            Some(Value::I32(v)) => Ok(*v),
            other => Err(format!("useBlock: answer {i} is {other:?}")),
        };
        Ok(Used {
            result: int_at(0)?,
            item: values
                .get(1)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            count: int_at(2)?,
            damage: int_at(3)?,
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }
}
