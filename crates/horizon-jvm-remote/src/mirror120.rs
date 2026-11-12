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

    /// A heightmap's height at a column (`MOTION_BLOCKING`, ...): the y above its top block;
    /// `None` where Horizon doesn't say (the level's bottom, to the mods).
    fn height(&self, _dimension: &str, _heightmap: &str, _x: i32, _z: i32) -> Option<i32> {
        None
    }

    /// The biome at a position, by id; `None` where Horizon doesn't say (plains, to the mods).
    fn biome(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        None
    }
}

/// The chunks Horizon's generator is decorating, for a mod's feature placed in them
/// ([`Runtime120::place_feature`]). Unlike [`World120`], what the feature sets goes in at once:
/// the feature reads its own blocks and the heightmaps over them as it goes.
pub trait Region120 {
    /// The state at a position, by 26.3's name; `None` outside the region (air, to the mods).
    fn block_state(&mut self, x: i32, y: i32, z: i32) -> Option<String>;
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32>;
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Option<String>;
    /// Sets a state the mod set, by 1.20.1's name.
    fn set_block(&mut self, pos: [i32; 3], state: &str);
    /// Keeps a block entity the mod made, as 1.20.1 saves it (SNBT).
    fn set_block_entity(&mut self, pos: [i32; 3], snbt: &str);
}

/// Where a mod's placed feature goes ([`Runtime120::place_feature`]): what seeds vanilla's
/// random for it in `applyBiomeDecoration`, and its origin.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaceFeature {
    pub dimension: String,
    pub feature: String,
    pub world_seed: i64,
    pub chunk: [i32; 2],
    pub index: i32,
    pub step: i32,
    pub origin: [i32; 3],
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
    /// The stack as 1.20.1 saves it (`ItemStack.save`), in SNBT: `{id,Count,tag}`. Horizon
    /// reads it with vanilla's own fixes from 1.20.1's data version to its own, so what a mod
    /// put on the stack (an enchantment, a name) arrives with it.
    pub snbt: String,
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
    /// Where the player's feet are, and which way they look (yaw, pitch, in degrees as
    /// Minecraft counts them). A block placed faces the way they look, and is placed only
    /// where they don't stand. `None`: the mirror player's eyes go to `hit`, looking south.
    pub stance: Option<Stance>,
}

/// Where a player stands and looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    pub feet: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

fn stance_values(stance: Option<Stance>) -> [Value; 5] {
    match stance {
        Some(s) => [
            Value::F64(s.feet[0]),
            Value::F64(s.feet[1]),
            Value::F64(s.feet[2]),
            Value::F64(f64::from(s.yaw)),
            Value::F64(f64::from(s.pitch)),
        ],
        None => [
            Value::F64(f64::NAN),
            Value::F64(f64::NAN),
            Value::F64(f64::NAN),
            Value::F64(f64::NAN),
            Value::F64(f64::NAN),
        ],
    }
}

/// What [`Runtime120::packet_probe`] saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PacketProbe {
    pub client_got: Option<i32>,
    pub server_got: Option<i32>,
    pub can_send_down: bool,
    pub can_send_up: bool,
}

/// What a server tick's events did ([`Runtime120::server_tick`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ServerTicked {
    /// Some mod listens to ticks (Fabric's `ServerTickEvents`).
    pub listening: bool,
    pub set: Vec<SetBlock>,
    pub block_entities: Vec<SetBlockEntity>,
    pub spawned: Vec<SpawnItem>,
}

/// A right-click with an item at nothing ([`Runtime120::use_item`]).
#[derive(Clone, Debug)]
pub struct UseItem {
    pub uuid: String,
    pub name: String,
    pub dimension: String,
    pub off_hand: bool,
    /// The held item as 1.20.1 item NBT.
    pub held: String,
    pub creative: bool,
    pub stance: Option<Stance>,
}

/// What a right-click with an item did.
#[derive(Clone, Debug, PartialEq)]
pub struct UsedItem {
    /// 0 passed (Horizon goes on), 1 taken, 2 taken with a swing of the arm.
    pub result: i32,
    /// The held item after, as 1.20.1 item NBT ("" for nothing).
    pub held: String,
    /// A menu, or a screen of the mod's own (`screen:<class>`), opened by it.
    pub opened: Option<OpenedMenu>,
    pub set: Vec<SetBlock>,
    pub block_entities: Vec<SetBlockEntity>,
    pub spawned: Vec<SpawnItem>,
}

/// A player's break of a block ([`Runtime120::break_block`]).
#[derive(Clone, Debug)]
pub struct BreakBlock {
    pub uuid: String,
    pub name: String,
    pub dimension: String,
    pub pos: [i32; 3],
    /// The block that was there, for [`BreakHow::After`] (the block is gone by then).
    pub state: String,
    /// The held item as 1.20.1 item NBT, "" for nothing: a tool's enchantments decide the
    /// drops (silk touch, fortune).
    pub held: String,
    pub creative: bool,
    pub stance: Option<Stance>,
    pub how: BreakHow,
}

/// What a [`BreakBlock`] asks of the mods.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakHow {
    /// A 1.20.1 mod's block: 1.20.1's own `ServerPlayerGameMode.destroyBlock` breaks it.
    Theirs,
    /// Another block Horizon is about to break: Fabric's `PlayerBlockBreakEvents.BEFORE`.
    Before,
    /// Another block Horizon just broke: `PlayerBlockBreakEvents.AFTER`.
    After,
}

/// What a break did.
#[derive(Clone, Debug, PartialEq)]
pub struct Broke {
    /// The break goes ahead (or went); false when a mod stopped it.
    pub goes: bool,
    /// The held item after, as 1.20.1 item NBT ("" for nothing): worn by the break.
    pub held: String,
    pub set: Vec<SetBlock>,
    pub block_entities: Vec<SetBlockEntity>,
    pub spawned: Vec<SpawnItem>,
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
    /// The menu the click opened, if it opened one (a cooking pot, a chest): Horizon shows
    /// its screen.
    pub opened: Option<OpenedMenu>,
}

/// What a hopper's ask of a mod's block found ([`Runtime120::hopper_insert`],
/// [`Runtime120::hopper_extract`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hopper<T> {
    /// The block holds no items on that face (Fabric's `ItemStorage.SIDED` found none), or
    /// its storage threw.
    NoStorage,
    /// It does, and nothing moved.
    Nothing,
    Moved(T),
}

/// A hopper's move, and what it changed besides (the block entity it took from or gave to).
#[derive(Clone, Debug, PartialEq)]
pub struct Moved<T> {
    pub hopper: Hopper<T>,
    pub set: Vec<SetBlock>,
    pub block_entities: Vec<SetBlockEntity>,
    pub spawned: Vec<SpawnItem>,
}

/// Stacks as the runtime takes them: `[[id, count, damage] | null, ...]`.
fn stacks_json(stacks: &[Option<(&str, i32, i32)>]) -> String {
    serde_json::Value::Array(
        stacks
            .iter()
            .map(|slot| match slot {
                Some((item, count, damage)) => serde_json::json!([item, count, damage]),
                None => serde_json::Value::Null,
            })
            .collect(),
    )
    .to_string()
}

/// What an input on a screen did./// What an input on a screen did.
#[derive(Clone, Debug, PartialEq)]
pub struct ScreenInput {
    /// Whether the screen is still open (Escape, the inventory key, or a `close` shut it).
    pub open: bool,
    /// The player's inventory slots it changed (0 to 35, hotbar first), each the stack now
    /// there as 1.20.1 item NBT, or `None` for a slot it emptied.
    pub slots: Vec<(usize, Option<String>)>,
    /// The blocks set, in order.
    pub set: Vec<SetBlock>,
    /// The block entities the mods' code changed, in order (the pot's contents).
    pub block_entities: Vec<SetBlockEntity>,
    /// The items the mods' code dropped, in order.
    pub spawned: Vec<SpawnItem>,
}

/// A player's 36 main inventory slots (hotbar first) as [`Runtime120::screen_frame`] and
/// [`Runtime120::screen_input`] take them: each stack's item, count and damage, or nothing.
pub fn inventory_json(slots: &[Option<(&str, i32, i32)>]) -> String {
    stacks_json(&slots[..slots.len().min(36)])
}

/// A menu a mod opened for a player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedMenu {
    /// The menu's type, as registered: `farmersdelight:cooking_pot`.
    pub kind: String,
    /// Its title as a text component's JSON, as the open-screen packet would carry it. Empty
    /// when the mod gave none.
    pub title: String,
}

/// Horizon's side while the runtime places a feature in the chunks being decorated.
struct InRegion<'a> {
    region: &'a mut dyn Region120,
    log: Log,
}

impl Handler for InRegion<'_> {
    fn call(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) -> Result<Vec<Value>, String> {
        match method {
            "blockState" => {
                let (x, y, z) = (int(&args, 1)?, int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.region
                        .block_state(x, y, z)
                        .map_or(Value::Null, Value::Str),
                ])
            }
            "blockStates" => {
                let positions = ints(&args, 1)?;
                let states = positions
                    .chunks_exact(3)
                    .map(|p| {
                        self.region
                            .block_state(p[0], p[1], p[2])
                            .unwrap_or_default()
                    })
                    .collect();
                Ok(vec![Value::Strs(states)])
            }
            // A new chunk has none but what the generator put there, which the mods don't read.
            "blockEntity" => Ok(vec![Value::Null]),
            "height" => {
                let heightmap = text(&args, 1)?;
                let (x, z) = (int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.region
                        .height(heightmap, x, z)
                        .map_or(Value::Null, Value::I32),
                ])
            }
            "biome" => {
                let (x, y, z) = (int(&args, 1)?, int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.region.biome(x, y, z).map_or(Value::Null, Value::Str),
                ])
            }
            _ => Err(format!(
                "Horizon has no {method} for 1.20.1 mods' world generation yet"
            )),
        }
    }

    fn event(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) {
        let at = |args: &[Value]| -> Result<[i32; 3], String> {
            Ok([int(args, 1)?, int(args, 2)?, int(args, 3)?])
        };
        match method {
            "log" => {
                if let Some(line) = args.first().and_then(Value::as_str) {
                    (self.log)(&format!("[fabric-1.20.1] {line}"));
                }
            }
            "setBlock" => match (at(&args), text(&args, 4)) {
                (Ok(pos), Ok(state)) => self.region.set_block(pos, state),
                _ => (self.log)("[fabric-1.20.1] a feature's setBlock unread"),
            },
            "setBlockEntity" => match (at(&args), text(&args, 4)) {
                (Ok(pos), Ok(snbt)) => self.region.set_block_entity(pos, snbt),
                _ => (self.log)("[fabric-1.20.1] a feature's setBlockEntity unread"),
            },
            // Items and the rest have no place in a chunk being generated.
            _ => {}
        }
    }
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

fn ints(args: &[Value], i: usize) -> Result<&[i32], String> {
    match args.get(i) {
        Some(Value::I32s(v)) => Ok(v),
        other => Err(format!(
            "argument {i}: an int array expected, not {other:?}"
        )),
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
            // Many positions in one call. A read costs a round trip whatever it carries, so a
            // mod looking at what is around it pays one instead of one each.
            "blockStates" => {
                let dimension = text(&args, 0)?;
                let positions = ints(&args, 1)?;
                let mut states = Vec::with_capacity(positions.len() / 3);
                for p in positions.chunks_exact(3) {
                    // "" where Horizon has no chunk there, which the other side reads as air.
                    states.push(
                        self.world
                            .block_state(dimension, p[0], p[1], p[2])
                            .unwrap_or_default(),
                    );
                }
                Ok(vec![Value::Strs(states)])
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
            "height" => {
                let (dimension, heightmap) = (text(&args, 0)?, text(&args, 1)?);
                let (x, z) = (int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.world
                        .height(dimension, heightmap, x, z)
                        .map_or(Value::Null, Value::I32),
                ])
            }
            "biome" => {
                let dimension = text(&args, 0)?;
                let (x, y, z) = (int(&args, 1)?, int(&args, 2)?, int(&args, 3)?);
                Ok(vec![
                    self.world
                        .biome(dimension, x, y, z)
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
                        snbt: text(&args, 7)?.to_owned(),
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

    /// One frame of the screen of the menu `player` has open, drawn at `width` by `height`
    /// with the mouse at (`mouse_x`, `mouse_y`): the operations Horizon's screen drawing reads,
    /// as JSON. `None` when they have no menu open, or its mod registered no screen for it.
    ///
    /// `inventory` is the player's 36 main slots as Horizon has them ([`inventory_json`]): the
    /// screen shows those. `at` is where the player stands: a menu they walked away from, or
    /// whose block is gone, closes, as a server closes it, and the answer is `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn screen_frame(
        &mut self,
        world: &dyn World120,
        player: &str,
        inventory: &str,
        at: [f64; 3],
        width: i32,
        height: i32,
        mouse_x: i32,
        mouse_y: i32,
    ) -> Result<Option<String>, String> {
        let args = [
            Value::Str(player.to_owned()),
            Value::Str(inventory.to_owned()),
            Value::F64(at[0]),
            Value::F64(at[1]),
            Value::F64(at[2]),
            Value::I32(width),
            Value::I32(height),
            Value::I32(mouse_x),
            Value::I32(mouse_y),
        ];
        // A screen reads the world as it draws (a cooking pot asks whether the block under it
        // is hot, for its flame), so a frame is a call in the world like any other.
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("screenFrame", &args, &mut handler)?;
        let frame = values.first().and_then(Value::as_str).unwrap_or_default();
        Ok((!frame.is_empty()).then(|| frame.to_owned()))
    }

    /// One input of `player`'s on their open screen: `input` is a JSON object with `kind`
    /// (`press`, `release`, `drag`, `move`, `scroll`, `key`, `char` or `close`), and as the
    /// kind needs, `x`, `y` (the screen's coordinates), `button` (GLFW's: 0 left, 1 right),
    /// `dx`, `dy`, `v` (the scroll), `key`, `scancode`, `char` and `mods` (GLFW's bits).
    /// `inventory` is the player's 36 main slots as Horizon has them ([`inventory_json`]).
    pub fn screen_input(
        &mut self,
        world: &dyn World120,
        player: &str,
        inventory: &str,
        input: &str,
    ) -> Result<ScreenInput, String> {
        let args = [
            Value::Str(player.to_owned()),
            Value::Str(inventory.to_owned()),
            Value::Str(input.to_owned()),
        ];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("screenInput", &args, &mut handler)?;
        let open = match values.first() {
            Some(Value::Bool(open)) => *open,
            other => return Err(format!("screenInput: answer 0 is {other:?}")),
        };
        let changed = values.get(1).and_then(Value::as_str).unwrap_or("{}");
        let changed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(changed)
            .map_err(|e| format!("screenInput: the changed slots don't read: {e}"))?;
        let mut slots: Vec<(usize, Option<String>)> = changed
            .into_iter()
            .filter_map(|(slot, snbt)| {
                let snbt = snbt.as_str()?;
                Some((
                    slot.parse().ok()?,
                    (!snbt.is_empty()).then(|| snbt.to_owned()),
                ))
            })
            .collect();
        slots.sort_by_key(|(slot, _)| *slot);
        Ok(ScreenInput {
            open,
            slots,
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// A round trip through the mods' own packets, for tests: the server sends `uuid`'s player
    /// 41, the client half's receiver answers 42, the server's receiver keeps it. What each side
    /// got (`None`: nothing), and whether each side's `canSend` said yes.
    pub fn packet_probe(
        &mut self,
        world: &dyn World120,
        uuid: &str,
        name: &str,
    ) -> Result<PacketProbe, String> {
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values(
            "packetProbe",
            &[Value::Str(uuid.to_owned()), Value::Str(name.to_owned())],
            &mut handler,
        )?;
        let int = |i: usize| match values.get(i) {
            Some(Value::I32(n)) if *n >= 0 => Some(*n),
            _ => None,
        };
        let yes = |i: usize| matches!(values.get(i), Some(Value::Bool(true)));
        Ok(PacketProbe {
            client_got: int(0),
            server_got: int(1),
            can_send_down: yes(2),
            can_send_up: yes(3),
        })
    }

    /// One server tick of `dimension` at Horizon's game time: the mods hear it through Fabric
    /// API's tick events (the server's start and level loads first, once). [`ServerTicked`]
    /// says whether any mod listens to ticks at all: when none does, a caller can ask less
    /// often.
    pub fn server_tick(
        &mut self,
        world: &dyn World120,
        dimension: &str,
        game_time: i64,
    ) -> Result<ServerTicked, String> {
        let args = [Value::Str(dimension.to_owned()), Value::I64(game_time)];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("serverTick", &args, &mut handler)?;
        let listening = match values.first() {
            Some(Value::Bool(b)) => *b,
            other => return Err(format!("serverTick: answer 0 is {other:?}")),
        };
        Ok(ServerTicked {
            listening,
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// The mods' placed features (of their own namespaces), which [`Self::place_feature`]
    /// places where Horizon's generator can't.
    pub fn mod_features(&mut self) -> Result<Vec<String>, String> {
        match self.call_answers("modFeatures", &[])?.first() {
            Some(Value::Strs(ids)) => Ok(ids.clone()),
            other => Err(format!("modFeatures: answer 0 is {other:?}")),
        }
    }

    /// Places a mod's placed feature in the chunks Horizon's generator is decorating, as
    /// vanilla's `applyBiomeDecoration` would (`PlacedFeature.placeWithBiomeCheck`, its random
    /// seeded as vanilla seeds it): the mod's own feature and placement code. Whether it placed
    /// anything.
    pub fn place_feature(
        &mut self,
        region: &mut dyn Region120,
        place: &PlaceFeature,
    ) -> Result<bool, String> {
        let mut handler = InRegion {
            region,
            log: self.log.clone(),
        };
        let args = [
            Value::Str(place.dimension.clone()),
            Value::Str(place.feature.clone()),
            Value::I64(place.world_seed),
            Value::I32(place.chunk[0]),
            Value::I32(place.chunk[1]),
            Value::I32(place.index),
            Value::I32(place.step),
            Value::I32(place.origin[0]),
            Value::I32(place.origin[1]),
            Value::I32(place.origin[2]),
        ];
        let values = self.call_values("placeFeature", &args, &mut handler)?;
        Ok(matches!(values.first(), Some(Value::Bool(true))))
    }

    /// The keys the 1.20.1 mods' key bindings are set to, as GLFW's key codes.
    pub fn mod_keys(&mut self, world: &dyn World120) -> Result<Vec<i32>, String> {
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("modKeys", &[], &mut handler)?;
        match values.first() {
            Some(Value::I32s(keys)) => Ok(keys.clone()),
            other => Err(format!("modKeys: answer 0 is {other:?}")),
        }
    }

    /// `uuid`'s player pressed (`down`) or let go of a key the mods bind (GLFW's code), while
    /// playing: their next client tick reads it ([`Runtime120::server_tick`]).
    pub fn key_press(
        &mut self,
        world: &dyn World120,
        uuid: &str,
        name: &str,
        key: i32,
        down: bool,
    ) -> Result<(), String> {
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let args = [
            Value::Str(uuid.to_owned()),
            Value::Str(name.to_owned()),
            Value::I32(key),
            Value::Bool(down),
        ];
        self.call_values("keyPress", &args, &mut handler)
            .map(|_| ())
    }

    /// For tests: registers (once) a key binding on K and a client-tick listener counting its
    /// clicks, through Fabric's own API; gives the clicks counted so far.
    pub fn key_probe(&mut self, world: &dyn World120) -> Result<i32, String> {
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        match self.call_values("keyProbe", &[], &mut handler)?.first() {
            Some(Value::I32(n)) => Ok(*n),
            other => Err(format!("keyProbe: answer 0 is {other:?}")),
        }
    }

    /// Horizon's player `uuid` left the game: their screen closes, the mods hear Fabric's
    /// `ServerPlayConnectionEvents.DISCONNECT`, and the mirror forgets them. False when the
    /// mods never met them.
    pub fn player_left(&mut self, world: &dyn World120, uuid: &str) -> Result<bool, String> {
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values =
            self.call_values("playerLeft", &[Value::Str(uuid.to_owned())], &mut handler)?;
        Ok(matches!(values.first(), Some(Value::Bool(true))))
    }

    /// A player right-clicks with a mod's item at nothing ([`UseItem`]): 1.20.1's own
    /// `ServerPlayerGameMode.useItem`. A use held over many ticks (eating, a bow) is let go and
    /// passed: Horizon runs those itself.
    pub fn use_item(&mut self, world: &dyn World120, u: &UseItem) -> Result<UsedItem, String> {
        let args: Vec<Value> = [
            Value::Str(u.uuid.clone()),
            Value::Str(u.name.clone()),
            Value::Str(u.dimension.clone()),
            Value::I32(i32::from(u.off_hand)),
            Value::Str(u.held.clone()),
            Value::Bool(u.creative),
        ]
        .into_iter()
        .chain(stance_values(u.stance))
        .collect();
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("useItem", &args, &mut handler)?;
        let result = match values.first() {
            Some(Value::I32(code)) => *code,
            other => return Err(format!("useItem: answer 0 is {other:?}")),
        };
        let text = |i: usize| {
            values
                .get(i)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let kind = text(2);
        Ok(UsedItem {
            result,
            held: text(1),
            opened: (!kind.is_empty()).then(|| OpenedMenu {
                kind,
                title: text(3),
            }),
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// A player breaks a block ([`BreakBlock`]): a 1.20.1 mod's, by 1.20.1's own code, or
    /// another, with Fabric's break events around Horizon's own break.
    pub fn break_block(&mut self, world: &dyn World120, b: &BreakBlock) -> Result<Broke, String> {
        let how = match b.how {
            BreakHow::Theirs => 0,
            BreakHow::Before => 1,
            BreakHow::After => 2,
        };
        let args: Vec<Value> = [
            Value::Str(b.uuid.clone()),
            Value::Str(b.name.clone()),
            Value::Str(b.dimension.clone()),
            Value::I32(b.pos[0]),
            Value::I32(b.pos[1]),
            Value::I32(b.pos[2]),
            Value::Str(b.state.clone()),
            Value::Str(b.held.clone()),
            Value::Bool(b.creative),
        ]
        .into_iter()
        .chain(stance_values(b.stance))
        .chain([Value::I32(how)])
        .collect();
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("breakBlock", &args, &mut handler)?;
        let goes = match values.first() {
            Some(Value::I32(code)) => *code != 0,
            other => return Err(format!("breakBlock: answer 0 is {other:?}")),
        };
        Ok(Broke {
            goes,
            held: values
                .get(1)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// A Horizon hopper (or dropper) pushes one item into the mods' block at `pos` through
    /// `face` (`Direction.get3DDataValue`: the side of the block the hopper touches), by the
    /// block's item storage (Fabric Transfer API's `ItemStorage.SIDED`, as its patch of
    /// vanilla's `ejectItems` asks). `stacks` are the hopper's slots in order, each item,
    /// count and damage; [`Hopper::Moved`] carries the index of the slot whose item went in.
    pub fn hopper_insert(
        &mut self,
        world: &dyn World120,
        dimension: &str,
        pos: [i32; 3],
        face: u8,
        stacks: &[Option<(&str, i32, i32)>],
    ) -> Result<Moved<usize>, String> {
        let args = [
            Value::Str(dimension.to_owned()),
            Value::I32(pos[0]),
            Value::I32(pos[1]),
            Value::I32(pos[2]),
            Value::I32(i32::from(face)),
            Value::Str(stacks_json(stacks)),
        ];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("hopperInsert", &args, &mut handler)?;
        let hopper = match values.first() {
            Some(Value::I32(i)) if *i >= 0 => Hopper::Moved(*i as usize),
            Some(Value::I32(-1)) => Hopper::Nothing,
            Some(Value::I32(_)) => Hopper::NoStorage,
            other => return Err(format!("hopperInsert: answer 0 is {other:?}")),
        };
        Ok(Moved {
            hopper,
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// A Horizon hopper pulls one item out of the mods' block at `pos` through `face`: the
    /// first it offers that the hopper has room for (`any_room`: an empty slot; else one of
    /// `partial`, its stacks not full yet). [`Hopper::Moved`] carries the item as 1.20.1 NBT.
    pub fn hopper_extract(
        &mut self,
        world: &dyn World120,
        dimension: &str,
        pos: [i32; 3],
        face: u8,
        any_room: bool,
        partial: &[(&str, i32, i32)],
    ) -> Result<Moved<String>, String> {
        let partial: Vec<Option<(&str, i32, i32)>> = partial.iter().copied().map(Some).collect();
        let args = [
            Value::Str(dimension.to_owned()),
            Value::I32(pos[0]),
            Value::I32(pos[1]),
            Value::I32(pos[2]),
            Value::I32(i32::from(face)),
            Value::Bool(any_room),
            Value::Str(stacks_json(&partial)),
        ];
        let mut handler = InWorld {
            world,
            set: Vec::new(),
            block_entities: Vec::new(),
            spawned: Vec::new(),
            log: self.log.clone(),
        };
        let values = self.call_values("hopperExtract", &args, &mut handler)?;
        let hopper = match (values.first(), values.get(1).and_then(Value::as_str)) {
            (Some(Value::I32(0)), Some(item)) if !item.is_empty() => Hopper::Moved(item.to_owned()),
            (Some(Value::I32(-1)), _) => Hopper::Nothing,
            (Some(Value::I32(_)), _) => Hopper::NoStorage,
            (other, _) => return Err(format!("hopperExtract: answer 0 is {other:?}")),
        };
        Ok(Moved {
            hopper,
            set: handler.set,
            block_entities: handler.block_entities,
            spawned: handler.spawned,
        })
    }

    /// What a screen's drawing records, as the frame JSON Horizon's screen drawing reads.
    /// Proves the recording works: the mixins on `GuiGraphics` bound, and the operations carry
    /// what the drawing needs. No mod, screen or window is involved.
    pub fn screen_probe(&mut self) -> Result<String, String> {
        self.call_with("screenProbe", &[])
    }

    /// The shape of each block at `positions` (x, y, z each), as the mod gives it at that
    /// position: one box list per position, inside the block. Empty where the mod answered
    /// with no shape at all, which is also what a block with nothing there answers.
    ///
    /// A block whose shape is its block entity's (every LibMultiPart container, so every Wired
    /// Redstone cable) has no shape in the registry: the exporter asks each state with no world
    /// and no block entity, and the answer there is nothing. This is how Horizon gets the real
    /// one.
    pub fn block_shapes(
        &mut self,
        world: &dyn World120,
        dimension: &str,
        positions: &[i32],
    ) -> Result<Vec<Vec<[f64; 6]>>, String> {
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
        let values = self.call_values("blockShapes", &args, &mut handler)?;
        let flat = match values.first() {
            Some(Value::F64s(v)) => v,
            other => return Err(format!("blockShapes: answer 0 is {other:?}")),
        };
        let mut out = Vec::with_capacity(positions.len() / 3);
        let mut at = 0usize;
        while at < flat.len() {
            let boxes = flat[at] as usize;
            at += 1;
            let end = at
                .checked_add(boxes.checked_mul(6).ok_or("blockShapes: too many boxes")?)
                .ok_or("blockShapes: too many boxes")?;
            if end > flat.len() {
                return Err("blockShapes: a box list runs past the answer".into());
            }
            out.push(
                flat[at..end]
                    .chunks_exact(6)
                    .map(|b| [b[0], b[1], b[2], b[3], b[4], b[5]])
                    .collect(),
            );
            at = end;
        }
        Ok(out)
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
        ]
        .into_iter()
        .chain(stance_values(used.stance))
        .collect::<Vec<_>>();
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
            opened: values
                .get(4)
                .and_then(Value::as_str)
                .filter(|kind| !kind.is_empty())
                .map(|kind| OpenedMenu {
                    kind: kind.to_owned(),
                    title: values
                        .get(5)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                }),
        })
    }
}
