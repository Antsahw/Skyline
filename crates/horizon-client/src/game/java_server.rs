//! The Java mods' server side in the game (docs/java-bridge.md): the game that simulates the
//! world (in-frame single player, the server thread, a LAN host, the dedicated server) hands
//! each tick to the mods' mirror (crate horizon-jvm), with who joined and left and what players
//! said, broke and placed since the last one; a player's command for a mod's command reaches
//! them at once. What the mods ask back (`events::Action`) is applied here: routed actions,
//! menus, block entities, keys, entities. With no Java mods on, each hook here is one check.
//! (Was the Java half of `server_mods.rs`, which upstream deleted with WebAssembly code mods.)
use super::{Game, lan::LanHost};
#[cfg(not(feature = "java-mods"))]
use horizon_world::registry::BlockStateId;

/// Who did something the Java mods hear about, resolved at the tick's end.
#[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
pub(super) enum Who {
    /// The window's own player, in-frame.
    Local,
    /// A connection's player (the server thread's owner is one).
    Conn(u32),
    Uuid(u128),
}

/// What Java mods make of a player's break of a block (`Game::java_break`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
pub(super) enum JavaBreak {
    /// A mod stopped it: the client puts the block back on the server's acknowledgement.
    Cancelled,
    /// The game breaks it; `after`: then `Game::java_after_break`.
    Horizon { after: bool },
    /// The mods' vanilla code broke it (a mod patches the inside of breaking: a vein miner):
    /// its removal, drops and the tool's wear come as Java's actions; nothing more to do.
    Java,
}

/// What players did during a tick, for the Java mods at its end.
#[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
pub(super) enum ModEvent {
    Chat(Who, String),
    Block {
        who: Who,
        pos: [i32; 3],
        state: String,
        placed: bool,
    },
}

/// The players the Java mods saw last tick (joins and leaves), and what players did this tick:
/// `ModsState::java`.
#[derive(Default)]
pub(super) struct JavaServerState {
    #[cfg(feature = "java-mods")]
    players: Vec<JavaPlayer>,
    #[cfg(feature = "java-mods")]
    events: Vec<ModEvent>,
    /// Blocks Java mods set, by dimension, made before that level's next tick.
    #[cfg(feature = "java-mods")]
    block_writes: Vec<(String, [i32; 3], String)>,
    /// The connection a Java mod's level event being played now leaves out (its source
    /// player's: `Action::LevelEvent`'s `except`), for `lan_record_events`.
    #[cfg(feature = "java-mods")]
    event_except: Option<u32>,
}

/// A player on this game's server as the Java mods see them.
#[cfg(feature = "java-mods")]
#[derive(Clone)]
struct JavaPlayer {
    id: String,
    name: String,
    x: f64,
    y: f64,
    z: f64,
    dimension: String,
    /// 0 survival, 1 creative, 2 adventure, 3 spectator.
    game_mode: i32,
    health: f32,
}

thread_local! {
    /// The player a server turn acts for now (`as_guest`), by UUID: whom a hook from inside
    /// the game's own code (the head of hurtServer) is about.
    static JAVA_ACTOR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Sets the player acting now (None: no one); the one before, to put back.
pub(super) fn set_java_actor(actor: Option<String>) -> Option<String> {
    JAVA_ACTOR.with(|a| a.replace(actor))
}

/// Random ticks that reached a block only a Java mod knows, for the mods at the end of the
/// level's block tick (Game::java_random_ticks).
#[cfg(feature = "java-mods")]
static JAVA_RANDOM_TICKS: std::sync::Mutex<Vec<horizon_core::position::BlockPos>> =
    std::sync::Mutex::new(Vec::new());
/// Which states are a Java mod's (not vanilla's), by state id: made with the first random tick.
#[cfg(feature = "java-mods")]
static JAVA_STATES: std::sync::OnceLock<Vec<bool>> = std::sync::OnceLock::new();
/// Vanilla states whose random tick is the Java mods' (a mod patches the block's `randomTick`
/// or what it calls: Serene Seasons' crops), by state id: made with JAVA_STATES.
#[cfg(feature = "java-mods")]
static JAVA_ROUTED_TICKS: std::sync::OnceLock<Vec<bool>> = std::sync::OnceLock::new();

/// The chunks Java mods' tickets keep in each dimension (`Action::ChunkTickets`), kept here
/// too for a level that opens later.
#[cfg(feature = "java-mods")]
static JAVA_TICKETS: std::sync::Mutex<Option<std::collections::HashMap<String, Vec<(i32, i32)>>>> =
    std::sync::Mutex::new(None);

/// The connection of each player the Java mods saw last (by UUID, as they name them): whom a
/// Java action that names a player is about, where no hub is at hand.
#[cfg(feature = "java-mods")]
static JAVA_CONNS: std::sync::Mutex<Vec<(String, u32)>> = std::sync::Mutex::new(Vec::new());

impl Game<'_> {
    /// The connection the level event being played now leaves out, when a Java mod made it
    /// with a player as its source (vanilla's broadcast leaves its source out).
    pub(super) fn java_event_except(&self) -> Option<u32> {
        #[cfg(feature = "java-mods")]
        {
            self.mods.java.event_except
        }
        #[cfg(not(feature = "java-mods"))]
        {
            None
        }
    }
}

/// The chunks Java mods' chunk loaders keep in `dimension` (none without Java mods).
pub(super) fn java_tickets(dimension: &str) -> Vec<(i32, i32)> {
    #[cfg(feature = "java-mods")]
    {
        JAVA_TICKETS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|m| m.get(dimension).cloned())
            .unwrap_or_default()
    }
    #[cfg(not(feature = "java-mods"))]
    {
        let _ = dimension;
        Vec::new()
    }
}

/// The actions besides breaking, using and random ticks that Java mods patch
/// (`events::routed_actions`: "item_pickup", "experience_orbs"), asked once Fabric runs.
#[cfg(feature = "java-mods")]
static JAVA_ROUTED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

/// Whether Java mods patch `action` (an `events::routed_actions` name), so the game sends it to
/// them.
#[cfg(feature = "java-mods")]
pub(super) fn java_routed(action: &str) -> bool {
    if JAVA_ROUTED.get().is_none() {
        if !horizon_jvm::events::ready() {
            return false;
        }
        let routed = horizon_jvm::events::routed_actions().unwrap_or_else(|e| {
            log::warn!("Java mods: routed actions: {e}");
            Vec::new()
        });
        if !routed.is_empty() {
            log::info!("Java mods: {} go to the mods", routed.join(", "));
        }
        let _ = JAVA_ROUTED.set(routed);
    }
    JAVA_ROUTED
        .get()
        .is_some_and(|r| r.iter().any(|a| a == action))
}

/// Precipitation ticks' columns (their tops) kept for Java mods that patch the biomes'
/// weather, answered at the end of the level's block tick (Game::java_random_ticks).
#[cfg(feature = "java-mods")]
static JAVA_PRECIPITATION: std::sync::Mutex<Vec<horizon_core::position::BlockPos>> =
    std::sync::Mutex::new(Vec::new());

/// The precipitation tick at the column topped by `top` is kept for the Java mods (true), when
/// they patch the biomes' weather (`events::routed_actions`' "precipitation").
pub(super) fn java_precipitation(top: horizon_core::position::BlockPos) -> bool {
    #[cfg(feature = "java-mods")]
    {
        if !JAVA_ROUTED
            .get()
            .is_some_and(|r| r.iter().any(|a| a == "precipitation"))
        {
            return false;
        }
        let mut kept = JAVA_PRECIPITATION.lock().unwrap_or_else(|e| e.into_inner());
        if kept.len() >= 4096 {
            return false;
        }
        kept.push(top);
        true
    }
    #[cfg(not(feature = "java-mods"))]
    {
        let _ = top;
        false
    }
}

/// True when the tick is the mods' instead of Horizon's (a routed vanilla block's).
#[cfg(not(feature = "java-mods"))]
pub(super) fn java_random_tick(
    _pos: horizon_core::position::BlockPos,
    _state: horizon_world::registry::BlockStateId,
) -> bool {
    false
}

/// True when the tick is the mods' instead of Horizon's (a routed vanilla block's); a Java
/// mod's own block is kept for the mods too (Game::java_random_ticks), and Horizon still runs
/// its template block's tick. Asked where native mods get theirs (GameLevel::before_random_tick).
#[cfg(feature = "java-mods")]
pub(super) fn java_random_tick(
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
) -> bool {
    let Some(states) = JAVA_STATES.get() else {
        return false;
    };
    let routed = JAVA_ROUTED_TICKS
        .get()
        .and_then(|r| r.get(state.0 as usize).copied())
        .unwrap_or(false);
    if routed || states.get(state.0 as usize).copied().unwrap_or(false) {
        let mut ticks = JAVA_RANDOM_TICKS.lock().unwrap_or_else(|e| e.into_inner());
        if ticks.len() < 65_536 {
            ticks.push(pos);
        } else {
            return false;
        }
    }
    routed
}

/// Blocks only Java mods know that a world's `set` placed or removed, for the mods (their block
/// entities exist from their placement, as vanilla's do: Game::java_random_ticks).
#[cfg(feature = "java-mods")]
static JAVA_SETS: std::sync::Mutex<Vec<[i32; 3]>> = std::sync::Mutex::new(Vec::new());

/// The states that hold a Java mod's fluid (its `LiquidBlock`s: BuildCraft's oil, Create's
/// honey), by state id: made with JAVA_STATES. The mods' mirror runs their flow
/// (`HorizonTicks`); Horizon's fluid code leaves them (`LevelAccess::foreign_fluid`).
#[cfg(feature = "java-mods")]
static JAVA_FLUID_STATES: std::sync::OnceLock<Vec<bool>> = std::sync::OnceLock::new();

/// `LevelAccess::foreign_fluid`: whether `state` holds a Java mod's fluid.
pub(super) fn java_fluid(state: horizon_world::registry::BlockStateId) -> bool {
    #[cfg(feature = "java-mods")]
    {
        JAVA_FLUID_STATES
            .get()
            .and_then(|s| s.get(state.0 as usize).copied())
            .unwrap_or(false)
    }
    #[cfg(not(feature = "java-mods"))]
    {
        let _ = state;
        false
    }
}

/// `LevelAccess::foreign_fluid_changed`: a Java mod's fluid at `pos` was placed or a neighbour
/// of it changed; the mods schedule its tick (`HorizonServer.blockEntities`, with the blocks set).
pub(super) fn java_fluid_changed(pos: horizon_core::position::BlockPos) {
    #[cfg(feature = "java-mods")]
    {
        let mut sets = JAVA_SETS.lock().unwrap_or_else(|e| e.into_inner());
        if sets.len() < 65_536 {
            sets.push([pos.x, pos.y, pos.z]);
        }
    }
    #[cfg(not(feature = "java-mods"))]
    let _ = pos;
}

/// horizon_client::world::SET_HOOK: a Java mod's block placed or removed is kept for the mods.
#[cfg(feature = "java-mods")]
pub(super) fn java_block_set(pos: (i32, i32, i32), previous: u32, state: u32) {
    let Some(states) = JAVA_STATES.get() else {
        return;
    };
    let java = |id: u32| states.get(id as usize).copied().unwrap_or(false);
    if java(previous) || java(state) {
        let mut sets = JAVA_SETS.lock().unwrap_or_else(|e| e.into_inner());
        if sets.len() < 65_536 {
            sets.push([pos.0, pos.1, pos.2]);
        }
    }
}

/// A player is about to be hurt (`LivingEntity.hurtServer` past its guards, where native mods
/// hear it: native_hooks.rs `hurt_server`): the Java mods' head of `hurtServer` and Fabric's
/// `ALLOW_DAMAGE`, for the player acting now. They may change `amount`; true cancels the hit.
pub(super) fn java_hurt(hurting: &horizon_client::health::Hurting<'_>, amount: &mut f32) -> bool {
    #[cfg(feature = "java-mods")]
    {
        let horizon_client::health::Hurting::Player(_, source) = hurting else {
            return false;
        };
        let Some(uuid) = JAVA_ACTOR.with(|a| a.borrow().clone()) else {
            return false;
        };
        match horizon_jvm::events::hurt(&uuid, source.damage_type, *amount) {
            Ok(Some(a)) => {
                *amount = a;
                false
            }
            Ok(None) => true,
            Err(e) => {
                log::warn!("Java mods: a hit failed: {e}");
                false
            }
        }
    }
    #[cfg(not(feature = "java-mods"))]
    {
        let _ = (hurting, amount);
        false
    }
}

impl Game<'_> {
    /// Adds `effect` to the player acting now unless a Java mod's head of
    /// `LivingEntity.addEffect` refuses it: the effects Horizon adds itself (beacons, conduits,
    /// raids' Hero of the Village, Bad Omen, the Warden's darkness). Whether it was added.
    pub(super) fn add_effect_asking_mods(
        &mut self,
        effect: horizon_client::effects::EffectInstance,
    ) -> bool {
        if self.java_add_effect(&effect) {
            return false;
        }
        self.potions.effects.add(effect);
        true
    }
}

/// Java mods' blocks a hopper found no item storage on, by dimension, position and face: asked
/// again after this game time (a hopper beside a mod's decorative block doesn't call Java each
/// tick; a block entity that wasn't there yet is found a little later).
#[cfg(feature = "java-mods")]
static JAVA_NO_STORAGE: std::sync::Mutex<
    Option<std::collections::HashMap<(&'static str, [i32; 3], u8), i64>>,
> = std::sync::Mutex::new(None);

/// The blocks the mods' container entities (a mod's chest minecart) were in at the last
/// snapshot, by dimension: Horizon's hoppers and droppers ask Java there too.
#[cfg(feature = "java-mods")]
static JAVA_CONTAINER_BLOCKS: std::sync::Mutex<
    Option<std::collections::HashMap<String, std::collections::HashSet<[i32; 3]>>>,
> = std::sync::Mutex::new(None);

impl Game<'_> {
    /// Whether the block at `pos` is a Java mod's that a hopper may ask about `face`, or a mod's
    /// container entity is in it.
    #[cfg(feature = "java-mods")]
    fn java_hopper_block(&self, pos: [i32; 3], face: horizon_core::geometry::Direction) -> bool {
        if !horizon_jvm::events::enabled() {
            return false;
        }
        let Some(states) = JAVA_STATES.get() else {
            return false;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        if !states.get(state.0 as usize).copied().unwrap_or(false) {
            // Not a mod's block: a mod's container entity there, as the last snapshot saw them
            // (no wait after a miss: a cart moves on).
            return JAVA_CONTAINER_BLOCKS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .and_then(|m| m.get(self.dim.id()))
                .is_some_and(|at| at.contains(&pos));
        }
        let key = (self.dim.id(), pos, face as u8);
        let mut cache = JAVA_NO_STORAGE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = cache.get_or_insert_with(Default::default);
        match cache.get(&key) {
            Some(&until) if self.world.game_time < until => false,
            Some(_) => {
                cache.remove(&key);
                true
            }
            None => true,
        }
    }

    #[cfg(feature = "java-mods")]
    fn java_hopper_no_storage(&self, pos: [i32; 3], face: horizon_core::geometry::Direction) {
        let mut cache = JAVA_NO_STORAGE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = cache.get_or_insert_with(Default::default);
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert((self.dim.id(), pos, face as u8), self.world.game_time + 40);
    }

    /// A hopper pushes one item of its slots (`stacks`, in order) into the Java mod's block at
    /// `pos` through `face`, by its item storage (Fabric's `ItemStorage.SIDED`, as Fabric
    /// Transfer API's patch of `HopperBlockEntity.ejectItems` does). None when the block isn't
    /// a Java mod's or holds no items there; else the slot whose item went in, if one did.
    pub(super) fn java_hopper_insert(
        &mut self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
        stacks: &[Option<&horizon_client::items::ItemStack>],
    ) -> Option<Option<usize>> {
        #[cfg(feature = "java-mods")]
        {
            use horizon_jvm::events::HopperMove;
            if !self.java_hopper_block(pos, face) {
                return None;
            }
            let snbt: Vec<Option<String>> = stacks
                .iter()
                .map(|s| {
                    s.map(|s| {
                        let one = horizon_client::items::ItemStack {
                            count: 1,
                            ..s.clone()
                        };
                        horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(&one))
                    })
                })
                .collect();
            let refs: Vec<Option<&str>> = snbt.iter().map(Option::as_deref).collect();
            let dim = self.dim.id();
            let moved = self.with_java_world(|| {
                horizon_jvm::events::hopper_insert(dim, pos, face as i32, &refs)
            });
            match moved {
                Ok(HopperMove::Moved(slot)) => Some(Some(slot)),
                Ok(HopperMove::Nothing) => Some(None),
                Ok(HopperMove::NoStorage) => {
                    self.java_hopper_no_storage(pos, face);
                    None
                }
                Err(e) => {
                    log::warn!("Java mods: a hopper's push into {pos:?} failed: {e}");
                    self.java_hopper_no_storage(pos, face);
                    None
                }
            }
        }
        #[cfg(not(feature = "java-mods"))]
        {
            let _ = (pos, face, stacks);
            None
        }
    }

    /// A hopper pulls one item out of the Java mod's block at `pos` through `face` (Fabric
    /// Transfer API's patch of `suckInItems`): the first it offers that fits (`any_room`: the
    /// hopper has an empty slot; else one of `partial`, its stacks not full yet). None when
    /// the block isn't a Java mod's or holds no items there; else the item, if one came.
    pub(super) fn java_hopper_extract(
        &mut self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
        any_room: bool,
        partial: &[&horizon_client::items::ItemStack],
    ) -> Option<Option<horizon_client::items::ItemStack>> {
        #[cfg(feature = "java-mods")]
        {
            use horizon_jvm::events::HopperMove;
            if !self.java_hopper_block(pos, face) {
                return None;
            }
            let snbt: Vec<String> = partial
                .iter()
                .map(|s| {
                    let one = horizon_client::items::ItemStack {
                        count: 1,
                        ..(*s).clone()
                    };
                    horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(&one))
                })
                .collect();
            let refs: Vec<&str> = snbt.iter().map(String::as_str).collect();
            let dim = self.dim.id();
            let moved = self.with_java_world(|| {
                horizon_jvm::events::hopper_extract(dim, pos, face as i32, any_room, &refs)
            });
            match moved {
                Ok(HopperMove::Moved(snbt)) => Some(
                    horizon_world::nbt::parse_snbt(&snbt)
                        .ok()
                        .and_then(|(tag, _)| horizon_items::item_nbt::item_from_nbt(&tag)),
                ),
                Ok(HopperMove::Nothing) => Some(None),
                Ok(HopperMove::NoStorage) => {
                    self.java_hopper_no_storage(pos, face);
                    None
                }
                Err(e) => {
                    log::warn!("Java mods: a hopper's pull from {pos:?} failed: {e}");
                    self.java_hopper_no_storage(pos, face);
                    None
                }
            }
        }
        #[cfg(not(feature = "java-mods"))]
        {
            let _ = (pos, face, any_room, partial);
            None
        }
    }

    /// Whether the Java mod's block (or a mod's container entity) at `pos` holds items on
    /// `face`: Fabric's `ItemStorage.SIDED` finds a storage there (asked with no item).
    pub(super) fn java_item_storage(
        &mut self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
    ) -> bool {
        self.java_hopper_insert(pos, face, &[]).is_some()
    }
}

#[cfg(not(feature = "java-mods"))]
impl Game<'_> {
    pub(super) fn tick_java_server(&mut self) {}
    pub(super) fn java_block_event(&mut self, _pos: [i32; 3], _state: BlockStateId, _placed: bool) {
    }
    pub(super) fn java_player_chat(&mut self, _who: Who, _text: &str) {}
    pub(super) fn local_java_command(&mut self, _line: &str) -> bool {
        false
    }
    pub(super) fn guest_java_command(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _line: &str,
    ) -> bool {
        false
    }
    pub(super) fn apply_java_level_writes(&mut self) {}
    pub(super) fn java_break(&mut self, _pos: [i32; 3], _state: BlockStateId) -> JavaBreak {
        JavaBreak::Horizon { after: false }
    }
    pub(super) fn java_after_break(&mut self, _pos: [i32; 3], _state: BlockStateId) {}
    pub(super) fn java_attack_block(&mut self, _pos: [i32; 3], _face: i32) -> bool {
        false
    }
    pub(super) fn java_crafted(&mut self, _takes: &[horizon_client::menu::CraftedTake]) {}
    pub(super) fn java_attack_mob(&mut self, _id: u32) -> bool {
        false
    }
    pub(super) fn java_mob_attack(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _gi: usize,
        _id: i32,
    ) -> bool {
        false
    }
    pub(super) fn java_select_slot(&mut self, _slot: i32) -> bool {
        false
    }
    pub(super) fn java_ride(&mut self, _vehicle: u32) -> bool {
        false
    }
    pub(super) fn java_dismount(&mut self, _vehicle: u32, _after: bool) -> bool {
        false
    }
    pub(super) fn java_fall_routed(&self) -> bool {
        false
    }
    pub(super) fn java_add_effect(
        &mut self,
        _effect: &horizon_client::effects::EffectInstance,
    ) -> bool {
        false
    }
    pub(super) fn java_add_effect_to(
        &mut self,
        _player: Option<u128>,
        _effect: &horizon_client::effects::EffectInstance,
    ) -> bool {
        false
    }
    pub(super) fn java_jump_refused(&mut self) -> bool {
        false
    }
    pub(super) fn java_jumped(&mut self) {}
    pub(super) fn java_player_teleport(
        &mut self,
        _player: u128,
        _dim: &str,
        _at: [f64; 3],
        _look: [f32; 2],
        _end: bool,
    ) -> bool {
        false
    }
    pub(super) fn java_award_refused(
        &mut self,
        _id: &str,
        _criterion: &str,
        _done: &[&str],
    ) -> bool {
        false
    }
    pub(super) fn java_fall(
        &mut self,
        _distance: f64,
        _multiplier: f32,
        _stalagmite: bool,
        _safe: f64,
        _damage_multiplier: f64,
    ) -> Option<f32> {
        None
    }
    pub(super) fn java_orb_touch(
        &mut self,
        _orb: &horizon_client::entities::ExperienceOrb,
    ) -> Option<(u32, i32)> {
        None
    }
    pub(super) fn java_item_touch(
        &mut self,
        _item: &horizon_client::entities::ItemEntity,
    ) -> Option<u32> {
        None
    }
    pub(super) fn java_player_died(
        &mut self,
        _player: u128,
        _message: Option<&str>,
        _conn: Option<u32>,
    ) {
    }
    pub(super) fn java_use_block(
        &mut self,
        _pos: [i32; 3],
        _face: i32,
        _point: [f64; 3],
        _off_hand: bool,
    ) -> Option<bool> {
        None
    }
    pub(super) fn java_random_ticks(&mut self) {}
    pub(super) fn java_menu_click(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _p: &horizon_protocol::packets::play::ContainerClick,
    ) -> bool {
        false
    }
    pub(super) fn java_menu_closed(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _window: i32,
    ) -> bool {
        false
    }
    pub(super) fn java_screen_input(&mut self, _host: &mut LanHost, _conn: u32, _data: &[u8]) {}
    pub(super) fn save_java_mods(&mut self) {}
    pub(super) fn java_world_closing(&mut self) {}
    pub(super) fn java_entity_attack(&mut self, _host: &mut LanHost, _conn: u32, _id: i32) -> bool {
        false
    }
    pub(super) fn java_entity_interact(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _p: &horizon_protocol::packets::play::Interact,
    ) -> bool {
        false
    }
}

#[cfg(feature = "java-mods")]
mod run {
    use super::{Game, JavaPlayer, LanHost, ModEvent, Who};
    use horizon_client::chunk_map::ChunkMap;
    use horizon_client::{command::args::GameMode as Mode, lan::data::component_nbt, text::Text};
    use horizon_protocol::packets::play::{self, Clientbound as C};
    use horizon_world::registry::{BlockRegistry, BlockStateId};
    use serde_json::Value;

    /// What Java mods hear of a tick (crate horizon-jvm).
    use horizon_jvm::events::Event as JavaEvent;

    /// Java's hurts of Horizon players (a mod's mob's bite), applied at the next Java tick.
    #[cfg(feature = "java-mods")]
    static PENDING_HURTS: std::sync::Mutex<Vec<horizon_jvm::events::Action>> =
        std::sync::Mutex::new(Vec::new());
    /// Teleports Java mods asked for while the hub was out of `self.lan`, done next tick.
    #[cfg(feature = "java-mods")]
    static PENDING_TELEPORTS: std::sync::Mutex<Vec<horizon_jvm::events::Action>> =
        std::sync::Mutex::new(Vec::new());

    /// A Java mod's chat line (`Action::Tell`, `Broadcast`): a JSON component, resolved with
    /// the game's language (mods' lang files are in the content root) and sent on as it is to
    /// players of the server; anything else (an older Java side) as literal text.
    fn java_chat(text: &str) -> Text {
        match serde_json::from_str::<Value>(text) {
            Ok(v) if v.is_object() || v.is_array() || v.is_string() => {
                horizon_client::packs::component(&v).with_component(v)
            }
            _ => Text::literal(text),
        }
    }

    /// Whether Java mods run here: the bridge started Fabric with its platform mod.
    fn java_mods_on() -> bool {
        horizon_jvm::events::enabled()
    }

    /// The Java mods' entity types (`/summon` of one goes to their server), once the bridge
    /// has its registries.
    fn java_entity_types() -> &'static std::collections::HashSet<String> {
        static NONE: std::sync::LazyLock<std::collections::HashSet<String>> =
            std::sync::LazyLock::new(Default::default);
        {
            static TYPES: std::sync::OnceLock<std::collections::HashSet<String>> =
                std::sync::OnceLock::new();
            if let Some(types) = TYPES.get() {
                return types;
            }
            if !horizon_jvm::events::enabled() {
                return &NONE;
            }
            match horizon_jvm::events::entity_types() {
                Ok(types) => TYPES.get_or_init(|| types.into_iter().collect()),
                Err(e) => {
                    log::warn!("Java mods: their entity types could not be read: {e}");
                    &NONE
                }
            }
        }
    }

    /// A vanilla command name: mods may not take it.
    fn vanilla_command(name: &str) -> bool {
        let d = horizon_client::command::vanilla::game_dispatcher();
        d.nodes[0]
            .children
            .iter()
            .any(|&c| d.nodes[c].name() == name)
    }

    /// The commands Java mods add (not vanilla's): their names, and their tree as a root of
    /// the commands report, merged into every player's command tree.
    struct JavaCommands {
        names: std::collections::HashSet<String>,
        tree: Value,
    }

    /// The Java mods' commands, once the bridge has loaded the server's data (they don't
    /// change after).
    fn java_commands() -> Option<&'static JavaCommands> {
        {
            static COMMANDS: std::sync::OnceLock<JavaCommands> = std::sync::OnceLock::new();
            if let Some(commands) = COMMANDS.get() {
                return Some(commands);
            }
            if !horizon_jvm::events::enabled() {
                return None;
            }
            let names: Vec<String> = horizon_jvm::events::command_names()
                .map_err(|e| log::warn!("Java mods: no commands: {e}"))
                .ok()?
                .into_iter()
                // Vanilla's, the dedicated server's and single player's own included: those
                // stay Horizon's.
                .filter(|n| {
                    !vanilla_command(n)
                        && !horizon_client::lan::commands::DEDICATED_ONLY.contains(&n.as_str())
                        && !horizon_client::lan::commands::INTEGRATED_ONLY.contains(&n.as_str())
                })
                .collect();
            let tree = horizon_jvm::events::command_tree(&names)
                .map_err(|e| log::warn!("Java mods: no command tree: {e}"))
                .ok()
                .and_then(|t| serde_json::from_str(&t).ok())?;
            if !names.is_empty() {
                log::info!("Java mods: commands /{}", names.join(", /"));
            }
            Some(COMMANDS.get_or_init(|| JavaCommands {
                names: names.into_iter().collect(),
                tree,
            }))
        }
    }

    /// Blocks queued for levels that don't tick (another dimension, in-frame)
    /// are kept up to this many; the oldest go first.
    const MAX_QUEUED_WRITES: usize = 1 << 16;

    /// Horizon's levels as Java mods read them during a call (crate horizon-jvm's `world`):
    /// borrowed, not lent, so nothing moves.
    struct JavaBlocks<'a> {
        registry: &'static BlockRegistry,
        dims: Vec<(&'static str, &'a ChunkMap)>,
        /// The level running now (the first of `dims`) and its block entities.
        entities: &'a horizon_client::block_entities::BlockEntities,
        biomes: &'a horizon_client::biome::Biomes,
    }
    impl horizon_jvm::world::Blocks for JavaBlocks<'_> {
        fn block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(x >> 4, z >> 4))?;
            Some(state_string(self.registry, chunk.block(x & 15, y, z & 15)))
        }
        fn block_entity(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
            let (current, chunks) = self.dims.first()?;
            if *current != dimension {
                return None;
            }
            let e = self.entities.get(&(x, y, z))?;
            let state = chunks.get(&(x >> 4, z >> 4))?.block(x & 15, y, z & 15);
            let name = self.registry.state(state)?.name.clone();
            // BlockEntity.saveWithFullMetadata: no chunk-only keepPacked.
            let mut tag = horizon_client::save::encode_block_entity((x, y, z), e, &name);
            if let horizon_world::nbt::Tag::Compound(c) = &mut tag {
                c.remove(&"keepPacked".into());
            }
            Some(horizon_client::snbt::to_snbt(&tag))
        }
        fn light(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<u8> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            Some(chunks.get(&(x >> 4, z >> 4))?.light(x & 15, y, z & 15))
        }
        fn column(
            &self,
            dimension: &str,
            cx: i32,
            cz: i32,
            min_y: i32,
            out: &mut [i32],
        ) -> Option<Vec<String>> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(cx, cz))?;
            // Whole sections unpacked at once (Java asks for one section at a time, or the
            // whole column): a few states each, found by a short search, not a hash.
            thread_local! {
                /// Each state's place in the palette being made (-1: none yet), by state id.
                static PLACES: std::cell::RefCell<Vec<i32>> = const { std::cell::RefCell::new(Vec::new()) };
            }
            let mut ids: Vec<u32> = Vec::new();
            let mut unpacked = [0u32; 4096];
            for (n, part) in out.chunks_mut(4096).enumerate() {
                let base = min_y + n as i32 * 16;
                let s = (base - horizon_client::chunk::MIN_Y) >> 4;
                let aligned = base & 15 == 0 && part.len() == 4096;
                match chunk.sections.get(s as usize) {
                    Some(section) if aligned && s >= 0 => section.unpack(&mut unpacked),
                    _ => {
                        for (i, v) in unpacked.iter_mut().enumerate().take(part.len()) {
                            let (x, z, y) = (
                                (i & 15) as i32,
                                ((i >> 4) & 15) as i32,
                                base + (i >> 8) as i32,
                            );
                            *v = chunk.block(x, y, z).0;
                        }
                    }
                }
                PLACES.with_borrow_mut(|places| {
                    for (slot, &state) in part.iter_mut().zip(unpacked.iter()) {
                        let i = state as usize;
                        if i >= places.len() {
                            places.resize(i + 1, -1);
                        }
                        if places[i] < 0 {
                            places[i] = ids.len() as i32;
                            ids.push(state);
                        }
                        *slot = places[i];
                    }
                });
            }
            PLACES.with_borrow_mut(|places| {
                for &id in &ids {
                    places[id as usize] = -1;
                }
            });
            Some(
                ids.into_iter()
                    .map(|id| state_string(self.registry, BlockStateId(id)))
                    .collect(),
            )
        }
        fn section_ids(
            &self,
            dimension: &str,
            cx: i32,
            sy: i32,
            cz: i32,
            out: &mut [i32],
        ) -> Option<bool> {
            let Some((_, chunks)) = self.dims.iter().find(|(d, _)| *d == dimension) else {
                return Some(false);
            };
            let Some(chunk) = chunks.get(&(cx, cz)) else {
                return Some(false);
            };
            let s = sy - (horizon_client::chunk::MIN_Y >> 4);
            let mut unpacked = [0u32; 4096];
            match usize::try_from(s).ok().and_then(|s| chunk.sections.get(s)) {
                Some(section) => section.unpack(&mut unpacked),
                // Above or below the world: air.
                None => unpacked.fill(0),
            }
            for (slot, id) in out.iter_mut().zip(unpacked) {
                *slot = id as i32;
            }
            Some(true)
        }
        fn chunk_revision(&self, dimension: &str, cx: i32, cz: i32) -> i64 {
            let Some((_, chunks)) = self.dims.iter().find(|(d, _)| *d == dimension) else {
                return -2;
            };
            chunks
                .get(&(cx, cz))
                .map_or(-2, |chunk| chunk.block_revision.get() as i64)
        }
        fn surface(&self, dimension: &str, x: i32, z: i32) -> Option<i32> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(x >> 4, z >> 4))?;
            let registry = self.registry;
            Some(chunk.world_surface(x & 15, z & 15, |v| {
                registry.state(BlockStateId(v)).is_some_and(|s| {
                    matches!(
                        s.name.as_str(),
                        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                    )
                })
            }))
        }
        fn state_name(&self, id: i32) -> Option<String> {
            let id = u32::try_from(id).ok()?;
            self.registry
                .state(BlockStateId(id))
                .map(|_| state_string(self.registry, BlockStateId(id)))
        }
        fn column_biomes(
            &self,
            dimension: &str,
            cx: i32,
            cz: i32,
            min_y: i32,
            out: &mut [i32],
        ) -> Option<Vec<String>> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(cx, cz))?;
            let mut palette = Vec::new();
            let mut index: std::collections::HashMap<u16, i32> = std::collections::HashMap::new();
            for (i, slot) in out.iter_mut().enumerate() {
                let (qx, qz, qy) = ((i & 3) as i32, ((i >> 2) & 3) as i32, (i >> 4) as i32);
                let biome = chunk.biome(qx * 4, min_y + qy * 4, qz * 4);
                *slot = *index.entry(biome).or_insert_with(|| {
                    palette.push(self.biomes.get(biome).name.clone());
                    palette.len() as i32 - 1
                });
            }
            Some(palette)
        }
    }

    /// A guest's item in vanilla's `Inventory` slot `slot`: 0-35 the inventory, 36 feet to
    /// 39 head (Horizon keeps the armour head first), 40 the off hand.
    fn guest_slot(
        g: &mut horizon_client::lan::guest::Guest,
        slot: usize,
    ) -> Option<&mut Option<horizon_client::items::ItemStack>> {
        match slot {
            0..=35 => g.inventory.get_mut(slot),
            36..=39 => g.armor.get_mut(39 - slot),
            40 => Some(&mut g.offhand),
            _ => None,
        }
    }

    /// The hash of each player's inventory as Java mods last had it.
    static JAVA_INVENTORIES: std::sync::Mutex<Option<std::collections::HashMap<u128, u64>>> =
        std::sync::Mutex::new(None);

    /// A guest's inventory for Java mods (vanilla's order, SNBT per slot), when it changed
    /// since they last had it.
    fn java_inventory(g: &horizon_client::lan::guest::Guest) -> Option<Vec<String>> {
        use std::hash::{Hash, Hasher};
        let slots: Vec<String> = (0..horizon_jvm::events::INVENTORY_SLOTS)
            .map(|v| {
                let stack = match v {
                    0..=35 => g.inventory.get(v).cloned().flatten(),
                    36..=39 => g.armor[39 - v].clone(),
                    _ => g.offhand.clone(),
                };
                stack.map_or_else(String::new, |stack| {
                    horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(&stack))
                })
            })
            .collect();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        slots.hash(&mut hasher);
        let hash = hasher.finish();
        let mut seen = JAVA_INVENTORIES.lock().unwrap_or_else(|e| e.into_inner());
        let seen = seen.get_or_insert_with(Default::default);
        (seen.insert(g.uuid, hash) != Some(hash)).then_some(slots)
    }

    /// Whether each player was dead at the last tick Java mods heard of (for deaths and
    /// respawns).
    static JAVA_DEAD: std::sync::Mutex<Option<std::collections::HashMap<String, bool>>> =
        std::sync::Mutex::new(None);

    /// What killed each player who died since the last tick (noted at its death message).
    static JAVA_DEATH_CAUSES: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> =
        std::sync::Mutex::new(None);

    /// Items Java mods dropped, by dimension, made when their level is in place.
    /// The windows of Java mods' menus, by player: (connection, window).
    static JAVA_WINDOWS: std::sync::Mutex<std::collections::BTreeMap<String, (u32, i32)>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    /// The menu window under a player's client-only screen (JEI's recipes over a mod's
    /// menu), by player: back in [`JAVA_WINDOWS`] when the client-only screen closes.
    static JAVA_UNDER: std::sync::Mutex<std::collections::BTreeMap<String, (u32, i32)>> =
        std::sync::Mutex::new(std::collections::BTreeMap::new());

    /// The player whose Java mod's menu has `window` open on `conn`.
    fn java_window_of(host: &LanHost, conn: u32, window: i32) -> Option<String> {
        let uuid = host
            .hub
            .guests
            .iter()
            .find(|g| g.conn == conn)
            .map(|g| uuid_string(g.uuid))?;
        let windows = JAVA_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
        (windows.get(&uuid) == Some(&(conn, window))).then_some(uuid)
    }

    static JAVA_DROPS: std::sync::Mutex<Vec<(String, horizon_jvm::events::Action)>> =
        std::sync::Mutex::new(Vec::new());

    /// `minecraft:oak_log[axis=y]`.
    fn state_string(registry: &BlockRegistry, state: BlockStateId) -> String {
        let Some(s) = registry.state(state) else {
            return "minecraft:air".into();
        };
        if s.properties.is_empty() {
            return s.name.clone();
        }
        let props: Vec<String> = s
            .properties
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{}[{}]", s.name, props.join(","))
    }

    /// `UUID.toString()`.
    fn uuid_string(u: u128) -> String {
        let h = format!("{u:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        )
    }

    impl Game<'_> {
        /// The player's profile id: the saved id, else the offline id of the player's name
        /// (upstream dropped its `player_uuid`; the mirror's ids were made with it).
        fn player_uuid(&self) -> u128 {
            self.level.player_uuid.map_or_else(
                || horizon_protocol::crypto::offline_uuid(crate::game::chat::PLAYER_NAME),
                |[a, b, c, d]| {
                    (u128::from(a as u32) << 96)
                        | (u128::from(b as u32) << 64)
                        | (u128::from(c as u32) << 32)
                        | u128::from(d as u32)
                },
            )
        }

        /// The window's own player's id as `UUID.toString()`.
        fn java_local_id(&self) -> String {
            uuid_string(self.player_uuid())
        }

        /// A Java mod's line in the window's chat; not through system_message: the mods must
        /// not hear their own lines. Written by this client, so the Chat option doesn't hide it.
        fn add_java_chat_line(&mut self, line: Text) {
            let focused = self.screen == crate::game::Screen::Chat;
            self.ui.chat.add(
                &self.gui.atlas,
                line,
                None,
                horizon_client::chat::ChatSource::Client,
                self.ticks,
                focused,
            );
        }

        /// The players on this game's server: the window's own player
        /// in-frame, then every connection's. `host` is the hub when it is
        /// out of `self.lan`.
        fn java_players(&self, host: Option<&LanHost>) -> Vec<JavaPlayer> {
            let mut out = Vec::new();
            if self.options.execution_role.is_client()
                && self.remote.is_none()
                && let Some(p) = &self.player
            {
                out.push(JavaPlayer {
                    id: self.java_local_id(),
                    name: crate::game::chat::PLAYER_NAME.to_owned(),
                    x: p.position.x,
                    y: p.position.y,
                    z: p.position.z,
                    dimension: self.dim.id().to_owned(),
                    game_mode: match self.mode {
                        Mode::Survival => 0,
                        Mode::Creative => 1,
                        Mode::Adventure => 2,
                        Mode::Spectator => 3,
                    },
                    health: p.health.health(),
                });
            }
            let guests = host
                .or(self.lan.as_ref())
                .into_iter()
                .flat_map(|h| &h.hub.guests);
            let conns: Vec<(String, u32)> = guests
                .clone()
                .filter(|g| g.placed)
                .map(|g| (uuid_string(g.uuid), g.conn))
                .collect();
            if !conns.is_empty() {
                *super::JAVA_CONNS.lock().unwrap_or_else(|e| e.into_inner()) = conns;
            }
            out.extend(guests.filter(|g| g.placed).map(|g| JavaPlayer {
                id: uuid_string(g.uuid),
                name: g.name.clone(),
                x: g.position[0],
                y: g.position[1],
                z: g.position[2],
                dimension: g.dimension.clone(),
                game_mode: g.game_mode,
                health: g.health,
            }));
            out
        }

        /// Whether what players do now reaches the Java mods: they run and this game
        /// simulates the world.
        fn hears_players(&self) -> bool {
            java_mods_on() && self.remote.is_none()
        }

        /// The tick's end: joins and leaves, the tick's events, then the mods' tick.
        pub(in crate::game) fn tick_java_server(&mut self) {
            super::super::java_profile::tick();
            self.send_java_commands();
            let owner = self.lan.as_ref().and_then(|h| h.hub.singleplayer_owner);
            self.java_device_own_player(owner);
            if !self.hears_players() {
                return;
            }
            let players = self.java_players(None);
            let before = std::mem::take(&mut self.mods.java.players);
            let mut java_events = Vec::new();
            for p in before
                .iter()
                .filter(|p| !players.iter().any(|q| q.id == p.id))
            {
                java_events.push(JavaEvent::Left { uuid: p.id.clone() });
            }
            for p in players
                .iter()
                .filter(|p| !before.iter().any(|q| q.id == p.id))
            {
                java_events.push(JavaEvent::Joined { uuid: p.id.clone() });
            }
            for event in std::mem::take(&mut self.mods.java.events) {
                let who = |w: &Who| {
                    let id = match w {
                        Who::Local => self.java_local_id(),
                        Who::Uuid(u) => uuid_string(*u),
                        Who::Conn(c) => {
                            let guests = self.lan.iter().flat_map(|h| &h.hub.guests);
                            let g = guests.into_iter().find(|g| g.conn == *c);
                            g.map_or_else(String::new, |g| uuid_string(g.uuid))
                        }
                    };
                    players.iter().chain(&before).find(|p| p.id == id).cloned()
                };
                match event {
                    ModEvent::Chat(w, text) => {
                        let Some(p) = who(&w) else { continue };
                        java_events.push(JavaEvent::Chat { uuid: p.id, text });
                    }
                    ModEvent::Block {
                        who: w,
                        pos,
                        state,
                        placed,
                    } => {
                        let Some(p) = who(&w) else { continue };
                        java_events.push(JavaEvent::Block {
                            uuid: p.id,
                            position: pos,
                            state,
                            placed,
                        });
                    }
                }
            }
            self.tick_java_mods(&players, &java_events);
            self.mods.java.players = players;
        }

        /// Hands the tick to the Java mods (crate horizon-jvm: Fabric API's events), then sends
        /// what they asked: system messages to a player or everyone.
        fn tick_java_mods(&mut self, players: &[JavaPlayer], events: &[JavaEvent]) {
            // Teleports Java mods asked for while the hub was out of `self.lan`.
            let pending =
                std::mem::take(&mut *PENDING_TELEPORTS.lock().unwrap_or_else(|e| e.into_inner()));
            for action in pending {
                self.java_teleport(action);
            }
            let hurts =
                std::mem::take(&mut *PENDING_HURTS.lock().unwrap_or_else(|e| e.into_inner()));
            for action in hurts {
                self.java_hurt(action);
            }
            // A connection's facing, or the window's own player's in-frame.
            let rotation = |id: &str| -> [f32; 2] {
                self.lan
                    .iter()
                    .flat_map(|h| &h.hub.guests)
                    .find(|g| uuid_string(g.uuid) == id)
                    .map_or([self.yaw, self.pitch], |g| [g.yaw, g.pitch])
            };
            // A connection's food, or the window's own player's in-frame.
            let food = |id: &str| -> (i32, f32) {
                self.lan
                    .iter()
                    .flat_map(|h| &h.hub.guests)
                    .find(|g| uuid_string(g.uuid) == id)
                    .map(|g| (g.food, g.saturation))
                    .or_else(|| {
                        self.player
                            .as_ref()
                            .map(|p| (p.health.food.food, p.health.food.saturation))
                    })
                    .unwrap_or((20, 5.0))
            };
            let players: Vec<horizon_jvm::events::Player> = players
                .iter()
                .map(|p| horizon_jvm::events::Player {
                    uuid: p.id.clone(),
                    name: p.name.clone(),
                    position: [p.x, p.y, p.z],
                    yaw: rotation(&p.id)[0],
                    pitch: rotation(&p.id)[1],
                    dimension: p.dimension.clone(),
                    game_mode: p.game_mode,
                    health: p.health,
                    food: food(&p.id).0,
                    saturation: food(&p.id).1,
                    // A connection's level (ops, the LAN host's grant); the window's own player
                    // in-frame owns its world.
                    permission: self
                        .lan
                        .iter()
                        .flat_map(|h| &h.hub.guests)
                        .find(|g| uuid_string(g.uuid) == p.id)
                        .map_or(4, |g| g.permission),
                    sneaking: self
                        .lan
                        .iter()
                        .flat_map(|h| &h.hub.guests)
                        .any(|g| uuid_string(g.uuid) == p.id && g.sneaking),
                    // A connection's keys (PlayerInput): a mod's mob it rides steers by them.
                    input: self
                        .lan
                        .iter()
                        .flat_map(|h| &h.hub.guests)
                        .find(|g| uuid_string(g.uuid) == p.id)
                        .map_or(0, |g| g.input),
                    inventory: self
                        .lan
                        .iter()
                        .flat_map(|h| &h.hub.guests)
                        .find(|g| uuid_string(g.uuid) == p.id)
                        .and_then(java_inventory),
                })
                .collect();
            // Not `options.dedicated`: single player's server thread sets it too.
            let dedicated = matches!(
                self.options.execution_role,
                crate::game::execution_role::ExecutionRole::DedicatedServer
            );
            // Deaths and respawns, from each player's health since the last tick.
            let mut events = events.to_vec();
            {
                let mut dead = JAVA_DEAD.lock().unwrap_or_else(|e| e.into_inner());
                let dead = dead.get_or_insert_with(Default::default);
                for p in &players {
                    // A connection's player is dead once the server's ServerPlayer.die ran for it
                    // (guest_die: its message and drops made); the window's own at no health.
                    let now = self
                        .lan
                        .iter()
                        .flat_map(|h| &h.hub.guests)
                        .find(|g| uuid_string(g.uuid) == p.uuid)
                        .map_or(p.health <= 0.0, |g| g.dead);
                    let was = dead.insert(p.uuid.clone(), now).unwrap_or(false);
                    if now && !was {
                        let conn = self
                            .lan
                            .iter()
                            .flat_map(|h| &h.hub.guests)
                            .find(|g| uuid_string(g.uuid) == p.uuid)
                            .map(|g| g.conn);
                        let tracker = match conn {
                            Some(conn) => self.guest_combat.get(&conn),
                            None => Some(&self.combat),
                        };
                        // Noted where the death message is made, else the tracker's last hit.
                        let noted = JAVA_DEATH_CAUSES
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .as_mut()
                            .and_then(|m| {
                                let by_conn = conn.and_then(|c| m.remove(&format!("conn:{c}")));
                                m.remove(&p.uuid).or(by_conn)
                            });
                        let cause = noted.unwrap_or_else(|| {
                            tracker
                                .and_then(|t| t.last_damage_type())
                                .unwrap_or("minecraft:generic")
                                .to_string()
                        });
                        events.push(JavaEvent::Died {
                            uuid: p.uuid.clone(),
                            cause,
                        });
                    } else if was && !now {
                        events.push(JavaEvent::Respawned {
                            uuid: p.uuid.clone(),
                        });
                    }
                }
            }
            // The Java side's world folder: the Overworld's, whichever level is in place (a
            // player alone in the Nether or a mod's level ticks only that one). Its files (mods'
            // block entities and entities by level, saved data) have always been there; the
            // level in place's folder made the mirror take a level change for a new world (its
            // entities unloaded mid-iteration: a ConcurrentModificationException each tick).
            let (ticks, root) = (
                self.ticks as i64,
                horizon_client::save::dimension_dir(&self.world_root, "minecraft:overworld"),
            );
            // The level in place keeps what Java mods' chunk loaders last asked for, though its
            // streaming was made after they asked (a world loading, a level swapped in).
            let wanted = super::java_tickets(self.dim.id());
            let held = self.streaming.ticketed();
            if wanted.len() != held.len() || wanted.iter().any(|c| !held.contains(c)) {
                self.streaming.set_ticketed(wanted);
            }
            let (game_time, seed) = (self.world.game_time, self.level.seed);
            // Horizon's game rules and difficulty, when they changed: the mods' spawner, their
            // mobs' griefing and despawning in peaceful read the mirror's (HorizonServer.gameRules).
            let rules = {
                use horizon_client::gamerules::{RULES, Value};
                static LAST: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());
                let now: String = RULES
                    .iter()
                    .filter_map(|r| {
                        Some(match self.ui.rules.get(r.name)? {
                            Value::Bool(b) => format!("minecraft:{}={b}", r.name),
                            Value::Int(i) => format!("minecraft:{}={i}", r.name),
                        })
                    })
                    .chain(std::iter::once(format!(
                        "difficulty={}",
                        self.ui.difficulty
                    )))
                    .collect::<Vec<_>>()
                    .join(";");
                let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
                (*last != now).then(|| {
                    *last = now.clone();
                    now
                })
            };
            // The entities' snapshot in the same view as the tick: what the tick read of the
            // world (whole sections, HorizonServer.section) serves it too.
            let neighbours = self.java_neighbours_json(&players);
            let (ticked, entities) = self.with_java_world(|| {
                if let Some(rules) = &rules
                    && let Err(e) = horizon_jvm::events::game_rules(rules)
                {
                    log::warn!("Java mods: game rules not passed: {e}");
                }
                // Horizon's mobs near players, for the mods' mobs to see (HorizonNeighbours).
                if let Err(e) = horizon_jvm::events::horizon_mobs(&neighbours) {
                    log::warn!("Java mods: Horizon's mobs not passed: {e}");
                }
                // What the server sent the window's own player, for the mods' client
                // connection (recording mods read it there): before the tick, whose client
                // tick records the player's own moves after them.
                if let Some((uuid, bodies)) = horizon_client::lan::packet_tap::take()
                    && let Err(e) = horizon_jvm::events::client_packets(&uuid_string(uuid), &bodies)
                {
                    log::warn!("Java mods: the client's packets not passed: {e}");
                }
                let ticked = horizon_jvm::events::game_time(game_time, seed).and_then(|()| {
                    horizon_jvm::events::server_tick(ticks, dedicated, &root, &players, &events)
                });
                let entities = java_mods_on().then(horizon_jvm::events::entities);
                (ticked, entities)
            });
            if let Err(e) = ticked {
                log::warn!("Java mods: a tick failed: {e}");
            }
            self.apply_java_actions(None);
            match entities {
                Some(Ok(json)) => self.send_java_entities(&json),
                Some(Err(e)) => log::warn!("Java mods: their entities could not be read: {e}"),
                None => {}
            }
        }

        /// The mods' entities after their tick (`HorizonEntityRender.snapshot`), to each
        /// player whose level they are in, within 160 blocks of it, on
        /// java_entities::CHANNEL, with the model trees its game has not had; the window's own
        /// player in-frame gets them straight.
        fn send_java_entities(&mut self, json: &str) {
            use serde_json::value::RawValue;
            use std::collections::{HashMap, HashSet};
            /// Every model tree Java gave, by key, and per connection those sent to it.
            struct Sent {
                trees: HashMap<String, Box<RawValue>>,
                to: HashMap<u32, HashSet<String>>,
                /// Connections last sent some entities (an empty list clears theirs).
                showing: HashSet<u32>,
                /// Each entity's last whole entry (Java sends "keep" while it looks the same),
                /// and per connection the entities whose look its game has.
                looks: HashMap<i64, String>,
                has: HashMap<u32, HashSet<i64>>,
            }
            /// A snapshot entry as Java routes it (`HorizonEntityRender.routed`): where, its
            /// model trees, then the entry passed on untouched.
            #[derive(serde::Deserialize)]
            struct Entry<'a> {
                x: f64,
                y: f64,
                z: f64,
                #[serde(default, borrow)]
                keys: Vec<&'a str>,
                /// A mod entity's id (block entities have none).
                #[serde(default)]
                id: Option<i64>,
                /// It looks as when last sent whole: `e` has its position only.
                #[serde(default)]
                keep: bool,
                /// What mods draw on a player (their layers): the player, by UUID (its own
                /// game draws it in third person only).
                #[serde(default, borrow)]
                owner: Option<&'a str>,
                #[serde(borrow)]
                e: &'a RawValue,
            }
            #[derive(serde::Deserialize)]
            struct Snapshot<'a> {
                #[serde(default, borrow)]
                dims: HashMap<&'a str, Vec<Entry<'a>>>,
                #[serde(default)]
                models: HashMap<String, Box<RawValue>>,
                /// What mods draw in each player's first person, by UUID.
                #[serde(default, borrow)]
                hands: HashMap<&'a str, &'a RawValue>,
                /// Players riding a mod's mob, by UUID: `{"id", "p": the seat}`.
                #[serde(default, borrow)]
                rides: HashMap<&'a str, &'a RawValue>,
                /// The mods' living entities as Horizon's mobs see them, per dimension.
                #[serde(default)]
                seen: HashMap<String, Vec<crate::game::java_entities::Neighbour>>,
                /// The blocks the mods' container entities are in, per dimension
                /// (`HorizonHoppers.containers`).
                #[serde(default)]
                containers: HashMap<String, Vec<[i32; 3]>>,
            }
            static SENT: std::sync::Mutex<Option<Sent>> = std::sync::Mutex::new(None);
            let _timed =
                super::super::java_profile::Timed::named("(Rust) entities: parse and send");
            super::super::java_profile::bytes("entities: snapshot from Java", json.len());
            let Ok(mut snapshot) = serde_json::from_str::<Snapshot>(json) else {
                return;
            };
            let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
            let sent = sent.get_or_insert_with(|| Sent {
                trees: HashMap::new(),
                to: HashMap::new(),
                showing: HashSet::new(),
                looks: HashMap::new(),
                has: HashMap::new(),
            });
            sent.trees.extend(snapshot.models);
            *crate::game::java_entities::NEIGHBOURS
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(std::mem::take(&mut snapshot.seen));
            *crate::game::java_server::JAVA_CONTAINER_BLOCKS
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(
                std::mem::take(&mut snapshot.containers)
                    .into_iter()
                    .map(|(dim, at)| (dim, at.into_iter().collect()))
                    .collect(),
            );
            // Whole entries kept for players that haven't had an entity's look; entities gone
            // go with theirs.
            let present: HashSet<i64> = snapshot
                .dims
                .values()
                .flatten()
                .filter_map(|e| e.id)
                .collect();
            sent.looks.retain(|id, _| present.contains(id));
            for e in snapshot.dims.values().flatten() {
                if let (Some(id), false) = (e.id, e.keep) {
                    sent.looks.insert(id, e.e.get().to_owned());
                }
            }
            // A player's entries: those of its level within 160 blocks (the entries' JSON,
            // passed on as Java wrote it).
            let near = |dimension: &str, at: [f64; 3]| -> Vec<&Entry> {
                snapshot
                    .dims
                    .get(dimension)
                    .into_iter()
                    .flatten()
                    .filter(|e| {
                        let d = [e.x - at[0], e.y - at[1], e.z - at[2]];
                        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] <= 160.0 * 160.0
                    })
                    .collect()
            };
            let hands = &snapshot.hands;
            let rides = &snapshot.rides;
            let payload = |sent: &mut Sent,
                           who: u32,
                           uuid: Option<&str>,
                           riders: &str,
                           entries: &[&Entry]|
             -> Vec<u8> {
                let had = sent.has.remove(&who).unwrap_or_default();
                let mut has = HashSet::with_capacity(entries.len());
                let mut out = String::with_capacity(
                    entries.iter().map(|e| e.e.get().len() + 1).sum::<usize>() + 64,
                );
                out.push_str("{\"entities\":[");
                let mut first = true;
                let mut own = Vec::new();
                for e in entries {
                    if e.owner.is_some() && e.owner == uuid {
                        own.push(e.e.get());
                        continue;
                    }
                    // A "keep" its game can't keep (it hasn't had the look): the last whole one.
                    let entry = match e.id {
                        Some(id) if e.keep && !had.contains(&id) => match sent.looks.get(&id) {
                            Some(whole) => whole.as_str(),
                            None => continue,
                        },
                        _ => e.e.get(),
                    };
                    if let Some(id) = e.id {
                        has.insert(id);
                    }
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    out.push_str(entry);
                }
                sent.has.insert(who, has);
                out.push_str("],\"own\":[");
                out.push_str(&own.join(","));
                out.push(']');
                if let Some(r) = uuid.and_then(|u| rides.get(u)) {
                    out.push_str(",\"ride\":");
                    out.push_str(r.get());
                }
                // Who rides which mod's mob, by player entity id: their games seat
                // them (the sitting pose) as on a vehicle of their own.
                if riders.len() > 2 {
                    out.push_str(",\"riders\":");
                    out.push_str(riders);
                }
                if let Some(h) = uuid.and_then(|u| hands.get(u)) {
                    out.push_str(",\"hands\":");
                    out.push_str(h.get());
                }
                out.push_str(",\"models\":{");
                let have = sent.to.entry(who).or_default();
                let mut first = true;
                // The trees and textures the first person's models use (`keys`).
                #[derive(serde::Deserialize)]
                struct HandKeys {
                    #[serde(default)]
                    keys: Vec<String>,
                }
                let hand_keys: Vec<String> = uuid
                    .and_then(|u| hands.get(u))
                    .and_then(|h| serde_json::from_str::<HandKeys>(h.get()).ok())
                    .map(|h| h.keys)
                    .unwrap_or_default();
                let hand_keys: Vec<&str> = hand_keys.iter().map(String::as_str).collect();
                for keys in entries
                    .iter()
                    .map(|e| &e.keys)
                    .chain(std::iter::once(&hand_keys))
                {
                    for key in keys {
                        if have.insert((*key).to_owned())
                            && let Some(tree) = sent.trees.get(*key)
                        {
                            if !first {
                                out.push(',');
                            }
                            first = false;
                            out.push_str(&serde_json::to_string(key).unwrap_or_default());
                            out.push(':');
                            out.push_str(tree.get());
                        }
                    }
                }
                out.push_str("}}");
                out.into_bytes()
            };
            let Some(host) = self.lan.as_mut() else {
                // In-frame: the window's own player.
                if self.serving.is_none() && self.remote.is_none() {
                    let at = self
                        .player
                        .as_ref()
                        .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
                    let entities = near(self.dim.id(), at);
                    let data = payload(sent, u32::MAX, None, "{}", &entities);
                    self.remote_java_entities(&data);
                }
                return;
            };
            // Connections gone: what they had goes with them (a new one starts empty).
            let conns: HashSet<u32> = host.hub.guests.iter().map(|g| g.conn).collect();
            sent.to.retain(|c, _| conns.contains(c) || *c == u32::MAX);
            sent.has.retain(|c, _| conns.contains(c) || *c == u32::MAX);
            sent.showing.retain(|c| conns.contains(c));
            // The players riding mods' mobs: `{"<player entity id>": <mob id>}`.
            #[derive(serde::Deserialize)]
            struct RideId {
                id: i64,
            }
            let mut riders = String::from("{");
            for g in &host.hub.guests {
                let Some(r) = rides.get(uuid_string(g.uuid).as_str()) else {
                    continue;
                };
                let Ok(r) = serde_json::from_str::<RideId>(r.get()) else {
                    continue;
                };
                if riders.len() > 1 {
                    riders.push(',');
                }
                riders.push_str(&format!("\"{}\":{}", g.entity_id, r.id));
            }
            riders.push('}');
            for g in &host.hub.guests {
                let entities = near(&g.dimension, g.position);
                // (What mods draw in its first person counts: a gun held with no mod entity near.)
                let drawn =
                    !entities.is_empty() || hands.contains_key(uuid_string(g.uuid).as_str());
                if !drawn && !sent.showing.contains(&g.conn) {
                    continue;
                }
                if !drawn {
                    sent.showing.remove(&g.conn);
                } else {
                    sent.showing.insert(g.conn);
                }
                let uuid = uuid_string(g.uuid);
                let data = payload(sent, g.conn, Some(&uuid), &riders, &entities);
                super::super::java_profile::bytes("entities: payload to players", data.len());
                host.hub.send_to(
                    g.conn,
                    &C::CustomPayload(horizon_protocol::packets::configuration::CustomPayload {
                        channel: crate::game::java_entities::CHANNEL.into(),
                        data,
                    }),
                );
            }
        }

        /// Sends what the Java mods asked since the last call: system messages to a player or
        /// everyone. `host` is the hub when it is out of `self.lan`.
        /// Each player's game is told which keys Java mods bind (game/java_screen.rs's KEYS),
        /// so it sends them while playing; again when they change (mods that register theirs
        /// once the client runs: JourneyMap's map key), read every 100 calls.
        fn send_java_keys(host: &mut LanHost) {
            #[derive(Default)]
            struct Told {
                keys: Option<Vec<i32>>,
                calls: u32,
                /// The players' games told the keys as they are, and those greeted at all.
                told: std::collections::HashSet<u32>,
                greeted: std::collections::HashSet<u32>,
            }
            static TOLD: std::sync::Mutex<Option<Told>> = std::sync::Mutex::new(None);
            let mut state = TOLD.lock().unwrap_or_else(|e| e.into_inner());
            let state = state.get_or_insert_with(Default::default);
            let placed: Vec<u32> = host
                .hub
                .guests
                .iter()
                .filter(|g| g.placed)
                .map(|g| g.conn)
                .collect();
            if placed.is_empty() {
                return;
            }
            state.calls = state.calls.wrapping_add(1);
            if state.keys.is_none() || state.calls.is_multiple_of(100) {
                let keys = horizon_jvm::events::bound_keys().unwrap_or_else(|e| {
                    log::warn!("Java mods: their key bindings can't be read: {e}");
                    Vec::new()
                });
                if state.keys.as_ref() != Some(&keys) {
                    state.keys = Some(keys);
                    state.told.clear();
                }
            }
            let keys = state.keys.clone().unwrap_or_default();
            // The Java mods with a settings screen, for the players' Mods pages (read once:
            // the mods' entrypoints don't change while the game runs).
            static CONFIG_MODS: std::sync::OnceLock<Vec<(String, String)>> =
                std::sync::OnceLock::new();
            let config_mods = CONFIG_MODS.get_or_init(|| {
                horizon_jvm::events::config_mods().unwrap_or_else(|e| {
                    log::warn!("Java mods: their settings screens can't be listed: {e}");
                    Vec::new()
                })
            });
            for conn in placed {
                if state.greeted.insert(conn) {
                    // The inventory overlay is there (REI's item list): its first, empty, frame.
                    Self::send_java_overlay_frame(host, conn, "[]");
                    if !config_mods.is_empty() {
                        host.hub.send_to(
                            conn,
                            &C::CustomPayload(
                                horizon_protocol::packets::configuration::CustomPayload {
                                    channel: super::super::java_screen::CONFIG_MODS.into(),
                                    data: serde_json::to_vec(config_mods).unwrap_or_default(),
                                },
                            ),
                        );
                    }
                }
                if !state.told.insert(conn) || keys.is_empty() {
                    continue;
                }
                host.hub.send_to(
                    conn,
                    &C::CustomPayload(horizon_protocol::packets::configuration::CustomPayload {
                        channel: super::super::java_screen::KEYS.into(),
                        data: serde_json::to_vec(&keys).unwrap_or_default(),
                    }),
                );
            }
        }

        fn apply_java_actions(&mut self, mut host: Option<&mut LanHost>) {
            if let Some(h) = host.as_deref_mut() {
                Self::send_java_keys(h);
            } else if let Some(h) = self.lan.as_mut() {
                Self::send_java_keys(h);
            }
            let mut wrote = false;
            for action in horizon_jvm::events::take_actions() {
                wrote |= matches!(
                    action,
                    horizon_jvm::events::Action::SetBlock { .. }
                        | horizon_jvm::events::Action::DropItem { .. }
                        | horizon_jvm::events::Action::SpawnEntity { .. }
                );
                match action {
                    horizon_jvm::events::Action::Tell { uuid, text } => {
                        self.tell_server_player(host.as_deref_mut(), &uuid, java_chat(&text));
                    }
                    horizon_jvm::events::Action::Overlay { uuid, text } => {
                        self.overlay_server_player(host.as_deref_mut(), &uuid, java_chat(&text));
                    }
                    horizon_jvm::events::Action::GameRule { id, value } => {
                        // GameRules.set from Java: Horizon's rule takes the value (as
                        // `/gamerule` sets it), so the world plays by the pack's rules.
                        use horizon_client::gamerules::{self, Value};
                        if let Some(rule) = gamerules::rule(&id) {
                            let parsed = match rule.default {
                                Value::Bool(_) => value.parse().ok().map(Value::Bool),
                                Value::Int(_) => value.parse().ok().map(Value::Int),
                            };
                            if let Some(v) = parsed {
                                self.ui.rules.set(rule.name, v);
                                self.sync_rules();
                            }
                        }
                    }
                    horizon_jvm::events::Action::WorldSpawn {
                        dimension,
                        position,
                        yaw,
                        pitch,
                    } => {
                        // LevelData.setRespawnData, as /setworldspawn sets it (the
                        // overworld's: Horizon's world spawn is the overworld's).
                        if dimension == "minecraft:overworld" {
                            let old = self.level.world_spawn.replace(position);
                            self.level.world_spawn_angle = [yaw, pitch];
                            // Players respawning at the world spawn follow it (as
                            // /setworldspawn's `spawn`), and so do the ones not placed yet.
                            let floor = |s: horizon_core::geometry::Vec3| {
                                [s.x.floor() as i32, s.y.floor() as i32, s.z.floor() as i32]
                            };
                            if self.spawn.map(floor) == old || self.spawn.is_none() {
                                let [x, y, z] = position;
                                self.spawn = Some(horizon_core::geometry::Vec3::new(
                                    f64::from(x) + 0.5,
                                    f64::from(y),
                                    f64::from(z) + 0.5,
                                ));
                                self.level.spawn = Some([f64::from(x), f64::from(y), f64::from(z)]);
                            }
                        }
                    }
                    teleport @ horizon_jvm::events::Action::Teleport { .. } => {
                        if host.is_some() {
                            // The hub is out of `self.lan` (a connection's command): the
                            // player's /tp needs it, so the next tick does it.
                            PENDING_TELEPORTS
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push(teleport);
                        } else {
                            self.java_teleport(teleport);
                        }
                    }
                    horizon_jvm::events::Action::Abilities {
                        uuid,
                        game_mode,
                        flags,
                        flying_speed,
                        walking_speed,
                    } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let to = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn);
                        if let Some(conn) = to {
                            host.hub.set_mod_abilities(
                                conn,
                                horizon_client::lan::guest::ModAbilities {
                                    game_mode,
                                    flags,
                                    flying_speed,
                                    walking_speed,
                                },
                            );
                        }
                    }
                    horizon_jvm::events::Action::TabName { uuid, json } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let Some(id) = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.uuid)
                        else {
                            continue;
                        };
                        let name = json
                            .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
                            .map(|v| {
                                Self::uuid_int_arrays(horizon_client::lan::data::component_nbt(&v))
                            });
                        host.hub.set_tab_name(id, name);
                    }
                    horizon_jvm::events::Action::OpenMenu { uuid, menu, title } => {
                        if let Some(h) = host.as_deref_mut() {
                            self.java_open_menu(h, &uuid, &menu, title.as_deref());
                        } else if let Some(mut lan) = self.lan.take() {
                            self.java_open_menu(&mut lan, &uuid, &menu, title.as_deref());
                            self.lan = Some(lan);
                        }
                    }
                    horizon_jvm::events::Action::SetBlockEntity {
                        dimension,
                        position,
                        snbt,
                    } => self.set_java_block_entity(&dimension, position, &snbt),
                    horizon_jvm::events::Action::ScreenFrame { uuid, frame } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let window = JAVA_WINDOWS
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get(&uuid)
                            .copied();
                        if let Some((conn, window)) = window {
                            Self::send_java_screen_frame(host, conn, window, &frame);
                        }
                    }
                    horizon_jvm::events::Action::OverlayFrame { uuid, frame } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        if let Some(conn) = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn)
                        {
                            Self::send_java_overlay_frame(host, conn, &frame);
                        }
                    }
                    horizon_jvm::events::Action::HudFrame { uuid, frame } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        if let Some(conn) = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn)
                        {
                            Self::send_java_hud_frame(host, conn, &frame);
                        }
                    }
                    horizon_jvm::events::Action::ClientScreen { uuid, open } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let Some(conn) = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn)
                        else {
                            continue;
                        };
                        let mut windows = JAVA_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
                        let mut under = JAVA_UNDER.lock().unwrap_or_else(|e| e.into_inner());
                        let client = (conn, super::super::java_screen::CLIENT_WINDOW);
                        if open {
                            // Its frames and input go by the client-only screen's window; a
                            // menu's window under it waits.
                            if let Some(menu) = windows.insert(uuid.clone(), client)
                                && menu != client
                            {
                                under.insert(uuid, menu);
                            }
                        } else if windows.get(&uuid) == Some(&client) {
                            windows.remove(&uuid);
                            Self::send_java_screen_close(host, conn);
                            // Back to the menu's screen, if its menu is still open (the
                            // menu's next frame shows it in place of the stand-in).
                            if let Some(menu) = under.remove(&uuid) {
                                windows.insert(uuid, menu);
                            }
                        }
                    }
                    horizon_jvm::events::Action::Food {
                        uuid,
                        food,
                        saturation,
                    } => {
                        let guest = host.as_deref_mut().or(self.lan.as_mut()).and_then(|h| {
                            h.hub
                                .guests
                                .iter_mut()
                                .find(|g| uuid_string(g.uuid) == uuid)
                        });
                        if let Some(g) = guest {
                            g.food = food.clamp(0, 20);
                            g.saturation = saturation.max(0.0);
                        } else if uuid == self.java_local_id()
                            && let Some(p) = self.player.as_mut()
                        {
                            p.health.food.food = food.clamp(0, 20);
                            p.health.food.saturation = saturation.max(0.0);
                        }
                    }
                    horizon_jvm::events::Action::MenuContent {
                        uuid,
                        open,
                        items,
                        carried,
                    } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let Some(conn) = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn)
                        else {
                            continue;
                        };
                        let mut windows = JAVA_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
                        let Some(items) = items else {
                            if let Some((c, w)) = windows.remove(&uuid)
                                && w != super::super::java_screen::CLIENT_WINDOW
                            {
                                host.hub.close_window(c, w);
                            }
                            // A menu under a client-only screen closed too.
                            if let Some((c, w)) = JAVA_UNDER
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&uuid)
                            {
                                host.hub.close_window(c, w);
                            }
                            continue;
                        };
                        if let Some((menu, title)) = open {
                            let title = title
                                .and_then(|j| serde_json::from_str::<Value>(&j).ok())
                                .unwrap_or(Value::Null);
                            let title = horizon_client::lan::data::component_nbt(&title);
                            match host.hub.open_window(conn, &menu, title) {
                                Some(w) => {
                                    windows.insert(uuid.clone(), (conn, w));
                                }
                                None => {
                                    log::warn!("Java mods: a menu of type {menu} can't be shown")
                                }
                            }
                        }
                        let Some(&(c, w)) = windows.get(&uuid) else {
                            continue;
                        };
                        // A client-only screen over the menu: no window to fill.
                        if w == super::super::java_screen::CLIENT_WINDOW {
                            continue;
                        }
                        let item = |s: &Option<String>| {
                            let snbt = s.as_deref()?;
                            match horizon_world::nbt::parse_snbt(snbt) {
                                Ok((tag, _)) => horizon_items::item_nbt::item_from_nbt(&tag),
                                Err(e) => {
                                    log::warn!("Java mods: a mod's item {snbt}: {e}");
                                    None
                                }
                            }
                        };
                        let stacks = items.iter().map(item).collect();
                        let carried = item(&carried);
                        host.hub.send_window_content(c, w, stacks, carried);
                    }
                    horizon_jvm::events::Action::SetBlock {
                        dimension,
                        position: [x, y, z],
                        state,
                    } => {
                        let queued = &mut self.mods.java.block_writes;
                        queued.push((dimension, [x, y, z], state));
                        if queued.len() > MAX_QUEUED_WRITES {
                            let over = queued.len() - MAX_QUEUED_WRITES;
                            queued.drain(..over);
                            log::warn!(
                                "Java mods: more blocks set than wait for their level: {over} dropped"
                            );
                        }
                    }
                    drop @ (horizon_jvm::events::Action::DropItem { .. }
                    | horizon_jvm::events::Action::LevelEvent { .. }
                    | horizon_jvm::events::Action::SpawnEntity { .. }) => {
                        let (horizon_jvm::events::Action::DropItem { dimension, .. }
                        | horizon_jvm::events::Action::LevelEvent { dimension, .. }
                        | horizon_jvm::events::Action::SpawnEntity { dimension, .. }) = &drop
                        else {
                            continue;
                        };
                        let mut drops = JAVA_DROPS.lock().unwrap_or_else(|e| e.into_inner());
                        if drops.len() < 4096 {
                            drops.push((dimension.clone(), drop));
                        }
                    }
                    hurt @ (horizon_jvm::events::Action::HurtPlayer { .. }
                    | horizon_jvm::events::Action::HurtMob { .. }) => {
                        PENDING_HURTS
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .push(hurt);
                    }
                    horizon_jvm::events::Action::Packet { uuid, data } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        // The packet's id (a VarInt), then its body, as vanilla encoded it.
                        let (mut id, mut shift, mut at) = (0i32, 0, 0);
                        while let Some(&b) = data.get(at) {
                            id |= i32::from(b & 0x7F) << shift;
                            at += 1;
                            shift += 7;
                            if b & 0x80 == 0 || shift >= 35 {
                                break;
                            }
                        }
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            host.hub.send_to(conn, &C::Unknown(id, data[at..].to_vec()));
                        }
                    }
                    horizon_jvm::events::Action::SetSlot { uuid, slot, item } => {
                        let Some(host) = host.as_deref_mut().or(self.lan.as_mut()) else {
                            continue;
                        };
                        let stack = match &item {
                            Some(snbt) => match horizon_world::nbt::parse_snbt(snbt) {
                                Ok((tag, _)) => horizon_items::item_nbt::item_from_nbt(&tag),
                                Err(e) => {
                                    log::warn!("Java mods: a mod's item {snbt}: {e}");
                                    continue;
                                }
                            },
                            None => None,
                        };
                        if let Some(g) = host
                            .hub
                            .guests
                            .iter_mut()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            && let Some(at) = guest_slot(g, slot)
                        {
                            // The hub sends the changed slot with the tick's other changes.
                            *at = stack;
                        }
                    }
                    horizon_jvm::events::Action::ChunkTickets { dimension, chunks } => {
                        super::JAVA_TICKETS
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .get_or_insert_with(Default::default)
                            .insert(dimension.clone(), chunks.clone());
                        let dim = super::super::Dim::from_id(&dimension);
                        if dim == self.dim {
                            self.streaming.set_ticketed(chunks);
                        } else if let Some(level) = self.levels.parked_mut(dim) {
                            level.set_ticketed(chunks);
                        }
                    }
                    horizon_jvm::events::Action::Broadcast { text } => {
                        let line = java_chat(&text);
                        let text = line.resolved().plain();
                        if self.options.dedicated.is_some() {
                            super::super::dedicated::log("INFO", &text);
                        } else {
                            log::info!("{text}");
                        }
                        for p in self.java_players(host.as_deref()) {
                            self.tell_server_player(host.as_deref_mut(), &p.id, line.clone());
                        }
                    }
                }
            }
            // The level in place gets the mods' blocks and drops now; the others' wait.
            if wrote {
                self.apply_java_level_writes();
            }
        }

        /// Opens Horizon's own menu for one a Java mod opened (`ServerPlayer.openMenu`):
        /// a crafting grid, the player's ender chest, a stonecutter, grindstone, loom,
        /// cartography or smithing table, with no block behind it (`/workbench`). Other
        /// menus are the mod's own; Horizon can't show those yet.
        fn java_open_menu(
            &mut self,
            host: &mut LanHost,
            uuid: &str,
            menu: &str,
            title: Option<&str>,
        ) {
            use super::super::stations::{Kind, Station};
            let Some(g) = host.hub.guests.iter().find(|g| uuid_string(g.uuid) == uuid) else {
                return;
            };
            let conn = g.conn;
            let at = g.position.map(|v| v.floor() as i32);
            let ender_items = g.ender_items.clone();
            let title_json = title
                .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
                .unwrap_or(serde_json::Value::Null);
            let station = match menu {
                "minecraft:stonecutter" => Some(Kind::Stonecutter),
                "minecraft:grindstone" => Some(Kind::Grindstone),
                "minecraft:loom" => Some(Kind::Loom),
                "minecraft:cartography_table" => Some(Kind::Cartography),
                "minecraft:smithing" => Some(Kind::Smithing),
                _ => None,
            };
            if menu == "minecraft:crafting" {
                self.lan_open_crafting(host, conn, at);
            } else if let Some(kind) = station {
                self.lan_open_station(host, conn, &Station::new(kind, at));
            } else if menu == "minecraft:generic_9x3"
                && title_json.to_string().contains("container.enderchest")
            {
                // PlayerEnderChestContainer: the guest's own ender chest items.
                self.guest_container_closed(host, conn);
                self.lan_station_closed(host, conn, None);
                self.lan_crafting_closed(host, conn, None);
                host.hub.open_unplaced_container(
                    conn,
                    menu,
                    horizon_client::lan::data::component_nbt(&title_json),
                    horizon_client::lan::menu::Slots::Ender,
                    ender_items,
                );
            } else {
                static TOLD: std::sync::OnceLock<
                    std::sync::Mutex<std::collections::HashSet<String>>,
                > = std::sync::OnceLock::new();
                let told = TOLD.get_or_init(Default::default);
                if told
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(menu.to_string())
                {
                    log::info!(
                        "Java mods: a mod opened a {menu} menu; Horizon shows only vanilla's own menus yet"
                    );
                }
                return;
            }
            let window = host.hub.guest(conn).map(|g| g.next_window);
            host.play.entry(conn).or_default().mod_window = window;
        }

        /// UUIDs in a component (a hover's `show_entity`) as vanilla's NBT has them: int arrays
        /// (UUIDUtil.CODEC), not the lists of ints JSON gives.
        fn uuid_int_arrays(tag: horizon_world::nbt::Tag) -> horizon_world::nbt::Tag {
            use horizon_world::nbt::Tag;
            match tag {
                Tag::Compound(entries) => Tag::Compound(
                    entries
                        .into_iter()
                        .map(|(k, v)| {
                            let v = match v {
                                Tag::List(items)
                                    if k.to_string().is_ok_and(|k| k == "uuid")
                                        && items.len() == 4 =>
                                {
                                    let ints: Option<Vec<i32>> = items
                                        .iter()
                                        .map(|t| match t {
                                            Tag::Int(i) => Some(*i),
                                            _ => None,
                                        })
                                        .collect();
                                    ints.map_or(Tag::List(items), Tag::IntArray)
                                }
                                other => Self::uuid_int_arrays(other),
                            };
                            (k, v)
                        })
                        .collect(),
                ),
                Tag::List(items) => {
                    Tag::List(items.into_iter().map(Self::uuid_int_arrays).collect())
                }
                other => other,
            }
        }

        #[cfg(feature = "java-mods")]
        /// A Java hurt of a Horizon player (`Natives.hurtPlayer`): as Horizon's own mobs hurt
        /// it (`hurt_player_from`: armour, blocking, knockback from where it came, death; the
        /// attacker named as a projectile with no owner is), as that guest; the window's own
        /// player in-frame. A type Horizon doesn't know (a mod's)
        /// counts as a mob's attack, or generic damage when it came from nowhere.
        fn java_hurt(&mut self, action: horizon_jvm::events::Action) {
            if let horizon_jvm::events::Action::HurtMob {
                dimension,
                id,
                damage_type,
                amount,
                from,
                by,
            } = action
            {
                self.java_hurt_mob(&dimension, id, &damage_type, amount, from, by);
                return;
            }
            let horizon_jvm::events::Action::HurtPlayer {
                uuid,
                damage_type,
                amount,
                from,
                by,
            } = action
            else {
                return;
            };
            let named = by.map(horizon_client::text::Text::literal);
            let kind = if self.motion.health.damage.id(&damage_type).is_some() {
                damage_type
            } else if from.is_some() {
                "minecraft:mob_attack".to_owned()
            } else {
                "minecraft:generic".to_owned()
            };
            if let Some(mut host) = self.lan.take() {
                let conn = host
                    .hub
                    .guests
                    .iter()
                    .find(|g| uuid_string(g.uuid) == uuid)
                    .map(|g| g.conn);
                if let Some(conn) = conn {
                    self.as_guest(&mut host, conn, |g| {
                        g.combat.projectile_direct = named;
                        g.hurt_player_from(amount, from, &kind, Some(None));
                        g.combat.projectile_direct = None;
                    });
                    self.lan = Some(host);
                    return;
                }
                self.lan = Some(host);
            }
            if self.serving.is_none() && self.remote.is_none() {
                self.combat.projectile_direct = named;
                self.hurt_player_from(amount, from, &kind, Some(None));
                self.combat.projectile_direct = None;
            }
        }

        /// A mod's mob hurt one of Horizon's (`Natives.hurtMob`, its shadow in the mirror): the
        /// mob takes it as a mob's attack (its armour, invulnerable ticks, death) and is knocked
        /// back from where it came, in its own level (in place or parked).
        #[cfg(feature = "java-mods")]
        fn java_hurt_mob(
            &mut self,
            dimension: &str,
            id: i32,
            damage_type: &str,
            amount: f32,
            from: Option<[f64; 3]>,
            by: Option<i32>,
        ) {
            use horizon_client::mobs::{Damage, DamageKind, Who};
            let Ok(id) = u32::try_from(id) else {
                return;
            };
            let kind = match damage_type {
                "minecraft:mob_attack" | "minecraft:mob_attack_no_aggro" => DamageKind::MobAttack,
                _ if from.is_some() => DamageKind::MobAttack,
                _ => DamageKind::Generic,
            };
            let damage = Damage {
                kind,
                attacker: by.and_then(|b| u32::try_from(b).ok()).map(Who::Mob),
                from,
            };
            let hurt = |m: &mut horizon_client::mobs::Mob| {
                if !m.alive() || m.removed {
                    return;
                }
                if m.damage(amount, damage) {
                    // LivingEntity.hurtServer's knockback: 0.4 away from the attacker.
                    if let Some(f) = from {
                        m.knockback(0.4, f[0] - m.position.x, f[2] - m.position.z, &damage);
                    }
                    m.needs_sync = true;
                }
            };
            if self.dim.id() == dimension {
                if let Some(m) = self.mobs.iter_mut().find(|m| m.id == id) {
                    hurt(m);
                }
                return;
            }
            let dim = super::super::Dim::from_id(dimension);
            if let Some(level) = self.levels.parked_mut(dim)
                && let Some(m) = level.mobs.iter_mut().find(|m| m.id == id)
            {
                hurt(m);
            }
        }

        /// Horizon's mobs within 32 blocks of a player, per dimension, for the mods' mobs to
        /// see (`HorizonNeighbours`): `{"<dimension>": [[id, "type", x, y, z, yaw, health]]}`,
        /// at most 128 a level.
        #[cfg(feature = "java-mods")]
        fn java_neighbours_json(&self, players: &[horizon_jvm::events::Player]) -> String {
            use std::fmt::Write;
            let mut out = String::from("{");
            let level = |out: &mut String, dim: &str, mobs: &[horizon_client::mobs::Mob]| {
                let near: Vec<[f64; 3]> = players
                    .iter()
                    .filter(|p| p.dimension == dim)
                    .map(|p| p.position)
                    .collect();
                if near.is_empty() {
                    return;
                }
                if out.len() > 1 {
                    out.push(',');
                }
                let _ = write!(out, "{}:[", serde_json::to_string(dim).unwrap_or_default());
                let mut n = 0;
                for m in mobs.iter().filter(|m| m.alive() && !m.removed) {
                    let at = [m.position.x, m.position.y, m.position.z];
                    if !near.iter().any(|p| {
                        (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2) + (p[2] - at[2]).powi(2)
                            <= 32.0 * 32.0
                    }) {
                        continue;
                    }
                    if n > 0 {
                        out.push(',');
                    }
                    let _ = write!(
                        out,
                        "[{},\"minecraft:{}\",{},{},{},{},{}]",
                        m.id,
                        m.kind.id.trim_start_matches("minecraft:"),
                        at[0],
                        at[1],
                        at[2],
                        m.yaw,
                        m.health
                    );
                    n += 1;
                    if n >= 128 {
                        break;
                    }
                }
                out.push(']');
            };
            level(&mut out, self.dim.id(), &self.mobs);
            for dim in self.levels.parked_dims() {
                if let Some(mobs) = self.levels.parked_mobs(dim) {
                    level(&mut out, dim.id(), mobs);
                }
            }
            out.push('}');
            out
        }

        /// Moves a player as a Java mod asked: Horizon's own `/tp`, run by the server in the
        /// target dimension (`execute in`), so a connection, the window's player and other
        /// dimensions all go the way vanilla's teleports go.
        #[cfg(feature = "java-mods")]
        fn java_teleport(&mut self, action: horizon_jvm::events::Action) {
            let horizon_jvm::events::Action::Teleport {
                uuid,
                dimension,
                position: [x, y, z],
                yaw,
                pitch,
            } = action
            else {
                return;
            };
            let Some(name) = self
                .java_players(None)
                .into_iter()
                .find(|p| p.id == uuid)
                .map(|p| p.name)
            else {
                return;
            };
            let line = format!("execute in {dimension} run tp {name} {x} {y} {z} {yaw} {pitch}");
            // The server's own command (a function's rights), not the player's.
            let permission = std::mem::replace(&mut self.ui.registries.permission, 4);
            let source = self.server_source();
            self.exec = Default::default();
            let result = self.execute_line(&line, source);
            self.ui.registries.permission = permission;
            if let Err(e) = result {
                log::warn!(
                    "Java mods: a teleport failed ({line}): {}",
                    e.message.plain()
                );
            }
        }

        /// Players' command trees list the Java mods' commands after vanilla's and the native
        /// mods' (`register_native_commands`), once they load (a while after the world opens) and
        /// again whenever the hub's tree is set anew.
        fn send_java_commands(&mut self) {
            /// The hub's key for a tree with the Java mods' commands merged in.
            const JAVA_KEY: u64 = 1 << 63;
            let Some(java) = java_commands() else {
                return;
            };
            let Some(host) = self.lan.as_mut() else {
                return;
            };
            if host.hub.mod_commands_key() == Some(JAVA_KEY) {
                return;
            }
            let mut tree = host.hub.mod_commands().clone();
            if tree.is_null() {
                tree = java.tree.clone();
            } else if let (Some(ours), Some(theirs)) = (
                tree["children"].as_object_mut(),
                java.tree["children"].as_object(),
            ) {
                // A native mod's command of the same name wins, as it runs first.
                for (name, node) in theirs {
                    ours.entry(name.clone()).or_insert_with(|| node.clone());
                }
            }
            host.hub.set_mod_commands(JAVA_KEY, tree);
        }

        /// `/line` from the player with this id, when a Java mod owns the command: run in the
        /// JVM as that player, its answers sent back. False when no Java mod owns it.
        fn java_command(&mut self, host: Option<&mut LanHost>, uuid: &str, line: &str) -> bool {
            if !java_mods_on() {
                return false;
            }
            let name = line.split_whitespace().next().unwrap_or_default();
            let Some(java) = java_commands() else {
                return false;
            };
            // A command naming a mod's entity type (`/summon mod:x`, `@e[type=mod:x]`): the
            // mods' server runs it (vanilla's command); Horizon knows no such type.
            let types = java_entity_types();
            let names_type = line
                .split(|c: char| {
                    c.is_whitespace() || matches!(c, '[' | ']' | ',' | '=' | '!' | '{')
                })
                .any(|word| types.contains(word));
            if !java.names.contains(name) && !names_type {
                // `@e`/`@n` may mean mods' entities too: the mods run the command first, for
                // their entities only and quietly; Horizon then runs it for its own.
                if !types.is_empty() && (line.contains("@e") || line.contains("@n")) {
                    let ran =
                        self.with_java_world(|| horizon_jvm::events::run_for_entities(uuid, line));
                    if let Err(e) = ran {
                        log::warn!("Java mods: /{line} for their entities failed: {e}");
                    }
                    self.apply_java_actions(host);
                }
                return false;
            }
            {
                match self.with_java_world(|| horizon_jvm::events::run_command(uuid, line)) {
                    Ok(true) => {}
                    Ok(false) => return false,
                    Err(e) => {
                        log::warn!("Java mods: /{line} failed: {e}");
                        return false;
                    }
                }
                self.apply_java_actions(host);
                true
            }
        }

        /// Runs `f`, a call into Java, with the levels as what Java mods read of the world.
        #[track_caller]
        pub(in crate::game) fn with_java_world<R>(&mut self, f: impl FnOnce() -> R) -> R {
            let _timed = super::super::java_profile::Timed::here();
            let registry = self.pack.assets.registry;
            let mut dims: Vec<(&'static str, &ChunkMap)> =
                vec![(self.dim.id(), &self.world.chunks)];
            for dim in self.levels.parked_dims() {
                if let Some(world) = self.levels.parked_world(dim) {
                    dims.push((dim.id(), &world.chunks));
                }
            }
            let entities = &self.world.block_entities;
            horizon_jvm::world::with_blocks(
                &JavaBlocks {
                    registry,
                    dims,
                    entities,
                    biomes: self.res.biomes(),
                },
                f,
            )
        }

        /// The slot changes Java mods made to the player acting now, made at once in the
        /// inventory in place (a guest's is swapped into the game for its turn, and put back
        /// after it): what the game then does to it (an axe's wear for the break) adds to them.
        fn apply_java_actor_slots(&mut self, uuid: &str) {
            let mut later = Vec::new();
            for action in horizon_jvm::events::take_actions() {
                if let horizon_jvm::events::Action::SetBlockEntity {
                    dimension,
                    position,
                    snbt,
                } = &action
                {
                    // At once: the next call reads it.
                    self.set_java_block_entity(dimension, *position, snbt);
                    continue;
                }
                if let horizon_jvm::events::Action::Food {
                    uuid: of,
                    food,
                    saturation,
                } = &action
                    && of == uuid
                    && let Some(p) = self.player.as_mut()
                {
                    // The acting player's food is in `self.player` for the call.
                    p.health.food.food = (*food).clamp(0, 20);
                    p.health.food.saturation = saturation.max(0.0);
                    continue;
                }
                let horizon_jvm::events::Action::SetSlot {
                    uuid: of,
                    slot,
                    item,
                } = &action
                else {
                    later.push(action);
                    continue;
                };
                if of != uuid {
                    later.push(action);
                    continue;
                }
                let stack = match item {
                    Some(snbt) => match horizon_world::nbt::parse_snbt(snbt) {
                        Ok((tag, _)) => horizon_items::item_nbt::item_from_nbt(&tag),
                        Err(_) => continue,
                    },
                    None => None,
                };
                let at = match *slot {
                    0..=35 => self.inventory.get_mut(*slot),
                    36..=39 => self.armor.get_mut(39 - *slot),
                    40 => Some(&mut self.offhand),
                    _ => None,
                };
                if let Some(at) = at {
                    *at = stack;
                }
            }
            for action in later {
                horizon_jvm::events::push_action(action);
            }
        }

        /// The blocks a Java mod set in the level in place while it used a block or an item
        /// (a mod's `BlockItem.useOn` placing its block in the mirror), made now, before the
        /// click's correction and its acknowledgement go to the player: vanilla's
        /// `useItemOn` sets the block in the handler. Left for the next tick, the correction
        /// sent air where the player's game had predicted the block and the acknowledgement
        /// made its game take that air: the placed block vanished until the next tick's update
        /// brought it back, and stayed gone where that update didn't come.
        fn apply_java_use_writes(&mut self) {
            let dim = self.dim.id();
            let mut later = Vec::new();
            for action in horizon_jvm::events::take_actions() {
                match action {
                    horizon_jvm::events::Action::SetBlock {
                        dimension,
                        position,
                        state,
                    } if dimension == dim => self
                        .mods
                        .java
                        .block_writes
                        .push((dimension, position, state)),
                    other => later.push(other),
                }
            }
            for action in later {
                horizon_jvm::events::push_action(action);
            }
            self.apply_java_level_writes();
        }

        /// `player` died (where native mods hear it: `native_player_died`, with the death
        /// message's key): what killed it, for the Java mods' death hooks at the next tick. The
        /// damage type the message names, else the last hit of its tracker (the guest's of
        /// `conn`, or the window player's); the tracker is cleared after.
        pub(in crate::game) fn java_player_died(
            &mut self,
            player: u128,
            message: Option<&str>,
            conn: Option<u32>,
        ) {
            if !java_mods_on() {
                return;
            }
            let named = message
                .and_then(|k| k.strip_prefix("death.attack."))
                .and_then(|id| {
                    let id = id.split('.').next().unwrap_or(id);
                    self.motion
                        .health
                        .damage
                        .entries()
                        .find(|(_, d)| d.message_id == id)
                        .map(|(_, d)| d.name.clone())
                });
            let tracker = match conn {
                Some(conn) => self.guest_combat.get(&conn),
                None => Some(&self.combat),
            };
            let cause = named.or_else(|| {
                tracker
                    .and_then(|t| t.last_damage_type())
                    .map(str::to_owned)
            });
            if let Some(cause) = cause {
                JAVA_DEATH_CAUSES
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert_with(Default::default)
                    .insert(uuid_string(player), cause);
            }
        }

        /// The player acting now uses (right-clicks) the block `pos` on `face` (vanilla's
        /// `Direction` id) at `point`: vanilla's `useItemOn` head for Java mods (Fabric API's
        /// `UseBlockCallback`, library events). Some(swing) when a mod took the click, so the
        /// game does nothing more.
        pub(in crate::game) fn java_use_block(
            &mut self,
            pos: [i32; 3],
            face: i32,
            point: [f64; 3],
            off_hand: bool,
        ) -> Option<bool> {
            {
                if !java_mods_on() || !self.hears_players() {
                    return None;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let (dim, selected) = (self.dim.id(), self.selected as i32);
                let used = self.with_java_world(|| {
                    horizon_jvm::events::use_block(
                        &uuid,
                        dim,
                        pos,
                        face,
                        point,
                        i32::from(off_hand),
                        selected,
                    )
                });
                self.apply_java_actor_slots(&uuid);
                self.apply_java_use_writes();
                match used {
                    Ok(horizon_jvm::events::UseResult::Pass) => None,
                    Ok(horizon_jvm::events::UseResult::Taken) => Some(false),
                    Ok(horizon_jvm::events::UseResult::TakenSwing) => Some(true),
                    Err(e) => {
                        log::warn!("Java mods: using a block failed: {e}");
                        None
                    }
                }
            }
        }

        /// The player acting now uses the item in a hand on nothing (vanilla's `useItem`, for
        /// Java mods: `UseItemCallback`, a mod item's own use), looking at `yaw`/`pitch`.
        /// Some(swing) when a mod took the click, so the game does nothing more.
        pub(in crate::game) fn java_use_item(
            &mut self,
            off_hand: bool,
            yaw: f32,
            pitch: f32,
        ) -> Option<bool> {
            {
                if !java_mods_on() || !self.hears_players() {
                    return None;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let (dim, selected) = (self.dim.id(), self.selected as i32);
                let used = self.with_java_world(|| {
                    horizon_jvm::events::use_item(
                        &uuid,
                        dim,
                        i32::from(off_hand),
                        selected,
                        yaw,
                        pitch,
                    )
                });
                self.apply_java_actor_slots(&uuid);
                self.apply_java_use_writes();
                match used {
                    Ok(horizon_jvm::events::UseResult::Pass) => None,
                    Ok(horizon_jvm::events::UseResult::Taken) => Some(false),
                    Ok(horizon_jvm::events::UseResult::TakenSwing) => Some(true),
                    Err(e) => {
                        log::warn!("Java mods: using an item failed: {e}");
                        None
                    }
                }
            }
        }

        /// A Java mod's change to a vanilla block entity (a storage mod took from a chest): the
        /// block entity there is now what the mod saved, in the level running now.
        fn set_java_block_entity(&mut self, dimension: &str, [x, y, z]: [i32; 3], snbt: &str) {
            if dimension != self.dim.id() {
                return;
            }
            let tag = match horizon_world::nbt::parse_snbt(snbt) {
                Ok((tag, _)) => tag,
                Err(e) => {
                    log::warn!("Java mods: a block entity at {x} {y} {z}: {e}");
                    return;
                }
            };
            if let Some((_, entity)) = horizon_client::save::decode_block_entity(&tag) {
                self.world.block_entities.insert((x, y, z), entity);
                // Redraw and resend what shows it.
                let state = self.world.block(x, y, z);
                let _ = self.world.set(x, y, z, state);
            }
        }

        /// The random ticks this level's blocks of Java mods got: the mods run them
        /// (`BlockState.randomTick`), on the world as it is now.
        pub(in crate::game) fn java_random_ticks(&mut self) {
            {
                if !java_mods_on() {
                    return;
                }
                if super::JAVA_STATES.get().is_none() {
                    // The mods' Mixins are read once Fabric runs (the routed ticks below).
                    if !horizon_jvm::events::ready() {
                        return;
                    }
                    let registry = self.pack.assets.registry;
                    let states = (0..registry.len() as u32)
                        .map(|id| {
                            registry
                                .state(BlockStateId(id))
                                .is_some_and(|s| !s.name.starts_with("minecraft:"))
                        })
                        .collect();
                    // Vanilla blocks whose random tick a mod patches: vanilla's code with
                    // the patches runs them (HorizonServer.routedRandomTicks).
                    let routed: std::collections::HashSet<String> =
                        match horizon_jvm::events::routed_random_ticks() {
                            Ok(names) => names.into_iter().collect(),
                            Err(e) => {
                                log::warn!("Java mods: routed random ticks: {e}");
                                Default::default()
                            }
                        };
                    if !routed.is_empty() {
                        log::info!(
                            "Java mods: random ticks of {} vanilla blocks go to the mods",
                            routed.len()
                        );
                    }
                    let routed = (0..registry.len() as u32)
                        .map(|id| {
                            registry.state(BlockStateId(id)).is_some_and(|s| {
                                let block = s.name.split('[').next().unwrap_or(&s.name);
                                routed.contains(block)
                            })
                        })
                        .collect();
                    let _ = super::JAVA_ROUTED_TICKS.set(routed);
                    // The mods' fluids: their blocks' states (every state of a LiquidBlock).
                    let fluids: std::collections::HashSet<String> =
                        match horizon_jvm::events::mod_fluid_blocks() {
                            Ok(names) => names.into_iter().collect(),
                            Err(e) => {
                                log::warn!("Java mods: their fluids: {e}");
                                Default::default()
                            }
                        };
                    if !fluids.is_empty() {
                        log::info!(
                            "Java mods: {} fluid blocks flow by the mods' code",
                            fluids.len()
                        );
                    }
                    let fluids = (0..registry.len() as u32)
                        .map(|id| {
                            registry.state(BlockStateId(id)).is_some_and(|s| {
                                let block = s.name.split('[').next().unwrap_or(&s.name);
                                fluids.contains(block)
                            })
                        })
                        .collect();
                    let _ = super::JAVA_FLUID_STATES.set(fluids);
                    let _ = super::JAVA_STATES.set(states);
                }
                // Java mods' blocks placed or removed: their block entities made or put away.
                let sets = std::mem::take(
                    &mut *super::JAVA_SETS.lock().unwrap_or_else(|e| e.into_inner()),
                );
                if !sets.is_empty() {
                    let dim = self.dim.id();
                    let ran =
                        self.with_java_world(|| horizon_jvm::events::block_entities(dim, &sets));
                    if let Err(e) = ran {
                        log::warn!("Java mods: block entities failed: {e}");
                    }
                }
                // The routed actions are asked once Fabric runs (precipitation's flag too).
                let _ = super::java_routed("precipitation");
                self.java_precipitation_ticks();
                let ticks = std::mem::take(
                    &mut *super::JAVA_RANDOM_TICKS
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()),
                );
                if ticks.is_empty() {
                    return;
                }
                let positions: Vec<[i32; 3]> = ticks.iter().map(|p| [p.x, p.y, p.z]).collect();
                let dim = self.dim.id();
                let ran =
                    self.with_java_world(|| horizon_jvm::events::random_ticks(dim, &positions));
                if let Err(e) = ran {
                    log::warn!("Java mods: random ticks failed: {e}");
                }
            }
        }

        /// A click in the window of a Java mod's menu (a cabinet's): the menu runs it, in Java;
        /// false for other windows.
        pub(in crate::game) fn java_menu_click(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            p: &play::ContainerClick,
        ) -> bool {
            {
                let Some(uuid) = java_window_of(host, conn, p.container) else {
                    return false;
                };
                let (slot, button, input) = (i32::from(p.slot), i32::from(p.button), p.input);
                let clicked = self.with_java_world(|| {
                    horizon_jvm::events::menu_click(&uuid, slot, button, input)
                });
                if let Err(e) = clicked {
                    log::warn!("Java mods: a click in a mod's menu failed: {e}");
                }
                self.apply_java_actions(Some(host));
                true
            }
        }

        /// The integrated world closes: the mods' client connection ends as a client's does
        /// when its server stops (recording mods save their recording then).
        pub(in crate::game) fn java_world_closing(&mut self) {
            if java_mods_on()
                && let Err(e) = horizon_jvm::events::client_closed()
            {
                log::warn!("Java mods: the client's connection didn't close: {e}");
            }
        }

        /// The world is saved (autosave, /save-all, closing): the Java mods' files beside it
        /// too (their entities, block entities, saved data), not only every 10 s.
        pub(in crate::game) fn save_java_mods(&mut self) {
            if java_mods_on()
                && let Err(e) = horizon_jvm::events::save_all()
            {
                log::warn!("Java mods: saving failed: {e}");
            }
        }

        /// A connection's player hit a Java mod's entity (`Attack` with one of the bridge's
        /// ids): the mods' server runs `Player.attack`. False for Horizon's entities.
        pub(in crate::game) fn java_entity_attack(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            id: i32,
        ) -> bool {
            {
                if !java_mods_on() {
                    return false;
                }
                if id < crate::game::java_entities::FIRST_ID {
                    return self.java_vanilla_entity(host, conn, id, None);
                }
                let Some(g) = host.hub.guest(conn) else {
                    return false;
                };
                if g.spectator() {
                    return true;
                }
                let uuid = uuid_string(g.uuid);
                match self.with_java_world(|| horizon_jvm::events::attack_entity(&uuid, id)) {
                    Ok(hit) => {
                        self.apply_java_actions(Some(host));
                        hit
                    }
                    Err(e) => {
                        log::warn!("Java mods: an attack failed: {e}");
                        true
                    }
                }
            }
        }

        /// A connection's player attacks (`interact` None) or uses one of Horizon's own mobs:
        /// Fabric's `AttackEntityCallback`/`UseEntityCallback` and the mods' heads of
        /// `Player.attack`/`interactOn`, on a copy made from the mob's NBT (Carry On's pickup,
        /// claims stopping a hit). True when a mod took it; a mob the mods removed (picked
        /// up) goes without drops.
        fn java_vanilla_entity(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            id: i32,
            interact: Option<&play::Interact>,
        ) -> bool {
            let Ok(id) = u32::try_from(id) else {
                return false;
            };
            let Some(i) = self
                .mobs
                .iter()
                .position(|m| m.id == id && m.alive() && !m.removed)
            else {
                return false;
            };
            let Some(g) = host.hub.guest(conn) else {
                return false;
            };
            if g.spectator() {
                return false;
            }
            let (uuid, selected) = (uuid_string(g.uuid), g.selected as i32);
            let snbt = horizon_client::snbt::to_snbt(&self.mob_tree_nbt(i));
            let dim = self.dim.id();
            let (using, hand, hit) = match interact {
                Some(p) => (true, i32::from(p.hand == 1), p.location),
                None => (false, 0, [0.0; 3]),
            };
            let hit = if using {
                let m = &self.mobs[i];
                [
                    m.position.x + hit[0],
                    m.position.y + hit[1],
                    m.position.z + hit[2],
                ]
            } else {
                hit
            };
            let result = self.with_java_world(|| {
                horizon_jvm::events::vanilla_entity(&uuid, dim, &snbt, using, hand, hit, selected)
            });
            self.apply_java_actions(Some(host));
            match result {
                Ok(r) => {
                    if r.removed
                        && let Some(m) = self.mobs.iter_mut().find(|m| m.id == id)
                    {
                        m.removed = true;
                    }
                    r.taken
                }
                Err(e) => {
                    log::warn!("Java mods: a mob's attack or use failed: {e}");
                    false
                }
            }
        }

        /// A connection's player used a Java mod's entity (`Interact`): the mods' server runs
        /// `Player.interactOn`. False for Horizon's entities.
        pub(in crate::game) fn java_entity_interact(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            p: &play::Interact,
        ) -> bool {
            {
                if !java_mods_on() {
                    return false;
                }
                if p.id < crate::game::java_entities::FIRST_ID {
                    return self.java_vanilla_entity(host, conn, p.id, Some(p));
                }
                let Some(g) = host.hub.guest(conn) else {
                    return false;
                };
                if g.spectator() {
                    return true;
                }
                let uuid = uuid_string(g.uuid);
                let selected = g.selected as i32;
                let used = self.with_java_world(|| {
                    horizon_jvm::events::interact_entity(
                        &uuid,
                        p.id,
                        p.hand == 1,
                        p.location,
                        selected,
                    )
                });
                if let Err(e) = &used {
                    log::warn!("Java mods: using an entity failed: {e}");
                }
                // The player's game swung its arm already.
                self.apply_java_actions(Some(host));
                true
            }
        }

        /// Input to the mod screen of a player's Java window (game/java_screen.rs's INPUT): the
        /// screen's own code runs it.
        pub(in crate::game) fn java_screen_input(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            data: &[u8],
        ) {
            {
                let Ok(input) = serde_json::from_slice::<Value>(data) else {
                    return;
                };
                let window = input.get("window").and_then(Value::as_i64).unwrap_or(-1) as i32;
                // A key of a Java mod's key binding, pressed or released while playing, and the
                // HUD's size: the connection's player's, whatever window is open.
                let bind = matches!(
                    input.get("kind").and_then(Value::as_str),
                    Some("bind" | "modconfig")
                );
                let uuid = if bind
                    || window == super::super::java_screen::HUD_WINDOW
                    || window == super::super::java_screen::OVERLAY_WINDOW
                {
                    host.hub
                        .guests
                        .iter()
                        .find(|g| g.conn == conn)
                        .map(|g| uuid_string(g.uuid))
                } else {
                    java_window_of(host, conn, window)
                };
                let Some(uuid) = uuid else {
                    return;
                };
                let input = input.to_string();
                let ran = self.with_java_world(|| horizon_jvm::events::screen_input(&uuid, &input));
                if let Err(e) = ran {
                    log::warn!("Java mods: input to a mod's screen failed: {e}");
                }
                self.apply_java_actions(Some(host));
            }
        }

        /// The player closed the window of a Java mod's menu; false for other windows.
        pub(in crate::game) fn java_menu_closed(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            window: i32,
        ) -> bool {
            {
                let Some(uuid) = java_window_of(host, conn, window) else {
                    return false;
                };
                JAVA_WINDOWS
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&uuid);
                let closed = self.with_java_world(|| horizon_jvm::events::menu_close(&uuid));
                if let Err(e) = closed {
                    log::warn!("Java mods: closing a mod's menu failed: {e}");
                }
                self.apply_java_actions(Some(host));
                true
            }
        }

        /// The player acting now is about to break `state` at `pos`: the mods' injections at
        /// `destroyBlock`'s head and Fabric's `PlayerBlockBreakEvents.BEFORE` (a mod may stop
        /// it), or vanilla's whole break on the Java side when a mod patches its inside (a vein
        /// miner). What the mods change (a tree feller's logs) is made with this tick's other
        /// actions.
        pub(in crate::game) fn java_break(
            &mut self,
            pos: [i32; 3],
            state: BlockStateId,
        ) -> super::JavaBreak {
            use super::JavaBreak;
            {
                if !java_mods_on() || !self.hears_players() {
                    return JavaBreak::Horizon { after: false };
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let state = state_string(self.pack.assets.registry, state);
                let (dim, selected) = (self.dim.id(), self.selected as i32);
                let result = self.with_java_world(|| {
                    horizon_jvm::events::break_block(&uuid, dim, pos, &state, selected)
                });
                self.apply_java_actor_slots(&uuid);
                match result {
                    Ok(horizon_jvm::events::BreakResult::Cancelled) => JavaBreak::Cancelled,
                    Ok(horizon_jvm::events::BreakResult::Horizon { after }) => {
                        JavaBreak::Horizon { after }
                    }
                    Ok(horizon_jvm::events::BreakResult::Java) => JavaBreak::Java,
                    Err(e) => {
                        log::warn!("Java mods: a block break failed: {e}");
                        JavaBreak::Horizon { after: false }
                    }
                }
            }
        }

        /// The player acting now starts digging `pos` from `face` (vanilla's `Direction` id):
        /// Fabric's `AttackBlockCallback`. True when a Java mod took it (no dig starts).
        pub(in crate::game) fn java_attack_block(&mut self, pos: [i32; 3], face: i32) -> bool {
            {
                if !java_mods_on() || !self.hears_players() {
                    return false;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let (dim, selected) = (self.dim.id(), self.selected as i32);
                let taken = self.with_java_world(|| {
                    horizon_jvm::events::attack_block(&uuid, dim, pos, face, selected)
                });
                self.apply_java_actor_slots(&uuid);
                taken.unwrap_or_else(|e| {
                    log::warn!("Java mods: attacking a block failed: {e}");
                    false
                })
            }
        }

        /// The player acting now took these crafting results: vanilla's `ResultSlot.onTake`
        /// on copies, for the Java mods' crafted hooks; what they give comes back as slots.
        pub(in crate::game) fn java_crafted(
            &mut self,
            takes: &[horizon_client::menu::CraftedTake],
        ) {
            {
                if takes.is_empty() || !java_mods_on() || !self.hears_players() {
                    return;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let snbt = |stack: &horizon_client::items::ItemStack| {
                    horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(stack))
                };
                for take in takes {
                    let grid: Vec<String> = take
                        .grid
                        .iter()
                        .map(|c| c.as_ref().map(snbt).unwrap_or_default())
                        .collect();
                    let result = snbt(&take.taken);
                    let ran = self.with_java_world(|| {
                        horizon_jvm::events::crafted(
                            &uuid,
                            &take.recipe,
                            &result,
                            take.taken.count as i32,
                            take.width as i32,
                            &grid,
                        )
                    });
                    if let Err(e) = ran {
                        log::warn!("Java mods: a crafting result's hooks failed: {e}");
                    }
                }
                self.apply_java_actor_slots(&uuid);
            }
        }

        /// The player acting now attacks the mob `id`, when Java mods patch the attack itself
        /// (inside `Player.attack`, or the client's attack: a mod's own swing): vanilla's
        /// `Player.attack` on copies of it and the mobs within 8 blocks, after the click's
        /// client half; each mob hit is hurt and pushed here with what its copy got. True when
        /// it was done that way (a mod's own swing taken: nothing hit here).
        /// A guest's attack on mob `id`, when Java mods patch the attack itself: their
        /// `Player.attack`, the guest facing the mob (its facing put back after). True when the
        /// mods took it.
        pub(in crate::game) fn java_mob_attack(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            gi: usize,
            id: i32,
        ) -> bool {
            let Ok(mob) = u32::try_from(id) else {
                return false;
            };
            let (yaw, pitch) = (host.hub.guests[gi].yaw, host.hub.guests[gi].pitch);
            let taken = self.look_at_entity(host, gi, id, None).is_some()
                && self
                    .as_guest(host, conn, |g| g.java_attack_mob(mob))
                    .unwrap_or(false);
            host.hub.guests[gi].yaw = yaw;
            host.hub.guests[gi].pitch = pitch;
            if taken {
                host.mob_players.insert(mob, conn);
            }
            taken
        }

        pub(in crate::game) fn java_attack_mob(&mut self, id: u32) -> bool {
            {
                if !java_mods_on() || !super::java_routed("player_attack") {
                    return false;
                }
                let Some(target) = self
                    .mobs
                    .iter()
                    .position(|m| m.id == id && m.alive() && !m.removed)
                else {
                    return false;
                };
                let Some(p) = self.player.as_ref() else {
                    return false;
                };
                let (feet, yaw, pitch) =
                    ([p.position.x, p.position.y, p.position.z], p.yaw, p.pitch);
                // The target first, then the mobs a sweep or a mod's swing can reach.
                let mut chosen = vec![target];
                for (i, m) in self.mobs.iter().enumerate() {
                    if i != target
                        && m.alive()
                        && !m.removed
                        && chosen.len() < 24
                        && m.distance_to(feet) < 8.0
                    {
                        chosen.push(i);
                    }
                }
                let ids: Vec<u32> = chosen.iter().map(|&i| self.mobs[i].id).collect();
                let nbts: Vec<String> = chosen
                    .iter()
                    .map(|&i| horizon_client::snbt::to_snbt(&self.mob_tree_nbt(i)))
                    .collect();
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let (dim, selected, ticks) = (
                    self.dim.id(),
                    self.selected as i32,
                    self.attack_ticks as i32,
                );
                let result = self.with_java_world(|| {
                    horizon_jvm::events::player_attack(
                        &uuid, dim, &nbts, &ids, ticks, yaw, pitch, selected,
                    )
                });
                self.apply_java_actor_slots(&uuid);
                let hits = match result {
                    Ok(Some(hits)) => hits,
                    Ok(None) => return false,
                    Err(e) => {
                        log::warn!("Java mods: an attack failed: {e}");
                        return false;
                    }
                };
                self.attack_ticks = 0;
                for hit in hits {
                    if let Some(i) = self.mobs.iter().position(|m| m.id == hit.id && m.alive()) {
                        self.routed_hit(
                            i,
                            hit.damage,
                            hit.knockback,
                            hit.knock_x,
                            hit.knock_z,
                            i == target,
                        );
                    }
                }
                self.swing_arm();
                true
            }
        }

        /// The precipitation ticks kept for the Java mods this tick: the biome's answers on the
        /// mirror (`Biome.shouldFreeze`, `shouldSnow`, `getPrecipitationAt` with Serene
        /// Seasons' patches), then Horizon's `tickPrecipitation` with them.
        fn java_precipitation_ticks(&mut self) {
            let tops = std::mem::take(
                &mut *super::JAVA_PRECIPITATION
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
            );
            if tops.is_empty() {
                return;
            }
            let flat: Vec<i32> = tops.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
            let dim = self.dim.id();
            let answers =
                match self.with_java_world(|| horizon_jvm::events::precipitation(dim, &flat)) {
                    Ok(a) => a,
                    Err(e) => {
                        log::warn!("Java mods: precipitation failed: {e}");
                        return;
                    }
                };
            use horizon_world::level::{Precipitation, PrecipitationKind};
            for (top, a) in tops.into_iter().zip(answers.chunks_exact(3)) {
                let info = Precipitation {
                    top,
                    freezes_below: a[0] != 0,
                    snows_at_top: a[1] != 0,
                    kind: match a[2] {
                        1 => PrecipitationKind::Rain,
                        2 => PrecipitationKind::Snow,
                        _ => PrecipitationKind::None,
                    },
                };
                self.with_level(|b, level, ticks| b.precipitation_tick_with(level, ticks, info));
            }
        }

        /// The player acting now picks hotbar slot `slot`: the Java mods' heads of
        /// `Inventory.setSelectedSlot` (Carry On keeps the slot while carrying). True when one
        /// cancelled.
        pub(in crate::game) fn java_select_slot(&mut self, slot: i32) -> bool {
            {
                if !java_mods_on() || !self.hears_players() {
                    return false;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let r = self.with_java_world(|| horizon_jvm::events::select_slot(&uuid, slot));
                self.apply_java_actor_slots(&uuid);
                r.unwrap_or(false)
            }
        }

        /// A vehicle of the level as the Java mods get a copy of it: its type, where it is,
        /// and a mob's NBT (empty for a boat or cart).
        #[cfg(feature = "java-mods")]
        fn java_vehicle(&mut self, vehicle: u32) -> Option<(String, [f64; 3], String)> {
            if let Some(i) = self.mobs.iter().position(|m| m.id == vehicle) {
                let m = &self.mobs[i];
                let at = [m.position.x, m.position.y, m.position.z];
                let ty = format!("minecraft:{}", m.kind.id.trim_start_matches("minecraft:"));
                return Some((ty, at, horizon_client::snbt::to_snbt(&self.mob_tree_nbt(i))));
            }
            if let Some(b) = self.boats.iter().find(|b| b.id == vehicle) {
                let raft = &*b.wood == "bamboo";
                let ty = match (raft, b.chest) {
                    (true, false) => "minecraft:bamboo_raft".to_string(),
                    (true, true) => "minecraft:bamboo_chest_raft".to_string(),
                    (false, false) => format!("minecraft:{}_boat", b.wood),
                    (false, true) => format!("minecraft:{}_chest_boat", b.wood),
                };
                return Some((
                    ty,
                    [b.position.x, b.position.y, b.position.z],
                    String::new(),
                ));
            }
            let c = self.carts.iter().find(|c| c.id == vehicle)?;
            let at = [c.position.x, c.position.y, c.position.z];
            Some(("minecraft:minecart".to_string(), at, String::new()))
        }

        /// The player acting now got onto vehicle `vehicle`: the Java mods' heads of
        /// `Entity.startRiding`, with a copy of it. True when one cancelled.
        pub(in crate::game) fn java_ride(&mut self, vehicle: u32) -> bool {
            {
                if !java_mods_on() || !self.hears_players() {
                    return false;
                }
                let Some((ty, at, snbt)) = self.java_vehicle(vehicle) else {
                    return false;
                };
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let dim = self.dim.id();
                let r =
                    self.with_java_world(|| horizon_jvm::events::ride(&uuid, dim, &ty, at, &snbt));
                self.apply_java_actor_slots(&uuid);
                r.unwrap_or(false)
            }
        }

        /// The player acting now gets off `vehicle` (sneaking): before the game takes it off
        /// (`after` false), the Java mods' heads of `stopRiding`/`removeVehicle` (true: one
        /// cancelled, it stays on); after (`after` true), their ends.
        pub(in crate::game) fn java_dismount(&mut self, vehicle: u32, after: bool) -> bool {
            #[cfg(feature = "java-mods")]
            {
                if !java_mods_on() || !self.hears_players() {
                    return false;
                }
                let Some((ty, at, snbt)) = self.java_vehicle(vehicle) else {
                    return false;
                };
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| uuid_string(self.player_uuid()));
                let dim = self.dim.id();
                let r = self.with_java_world(|| {
                    horizon_jvm::events::dismount(&uuid, dim, &ty, at, &snbt, after)
                });
                self.apply_java_actor_slots(&uuid);
                r.unwrap_or(false)
            }
            #[cfg(not(feature = "java-mods"))]
            {
                let _ = (vehicle, after);
                false
            }
        }

        /// The player acting now gets `effect`: the Java mods' heads of `LivingEntity.addEffect`.
        /// True when one cancelled (the effect isn't added).
        pub(in crate::game) fn java_add_effect(
            &mut self,
            effect: &horizon_client::effects::EffectInstance,
        ) -> bool {
            self.java_add_effect_to(None, effect)
        }

        /// The player acting now is about to jump (the server's `jumpFromGround`): the Java
        /// mods' heads there, when a mod hooks it (routed action `jumps`). True when one
        /// cancelled: the caller doesn't jump the player.
        pub(in crate::game) fn java_jump_refused(&mut self) -> bool {
            #[cfg(feature = "java-mods")]
            {
                if !java_mods_on() || !self.hears_players() || !super::java_routed("jumps") {
                    return false;
                }
                let uuid = self.acting_uuid_string();
                match self.with_java_world(|| horizon_jvm::events::jump_start(&uuid)) {
                    Ok(refused) => return refused,
                    Err(e) => log::warn!("Java mods: a jump failed: {e}"),
                }
            }
            false
        }

        /// The player acting now jumped: the ends of `jumpFromGround` for the Java mods.
        pub(in crate::game) fn java_jumped(&mut self) {
            #[cfg(feature = "java-mods")]
            {
                if !java_mods_on() || !self.hears_players() || !super::java_routed("jumps") {
                    return;
                }
                let uuid = self.acting_uuid_string();
                if let Err(e) = self.with_java_world(|| horizon_jvm::events::jump_end(&uuid)) {
                    log::warn!("Java mods: a jump failed: {e}");
                }
            }
        }

        #[cfg(feature = "java-mods")]
        fn acting_uuid_string(&self) -> String {
            self.lan_acting_uuid
                .map(uuid_string)
                .unwrap_or_else(|| uuid_string(self.player_uuid()))
        }

        /// `player` is about to be teleported to `at` in `dim` facing `look` (commands, a
        /// pearl, chorus fruit, a portal), or was (`end`): the Java mods' injections at
        /// `ServerPlayer.teleport`'s head, or its ends (Serene Seasons' GlitchCore, Balm's
        /// events), when a mod hooks it. True when a head cancelled: the caller leaves the
        /// player where it is.
        pub(in crate::game) fn java_player_teleport(
            &mut self,
            player: u128,
            dim: &str,
            at: [f64; 3],
            look: [f32; 2],
            end: bool,
        ) -> bool {
            #[cfg(feature = "java-mods")]
            {
                if !java_mods_on() || !super::java_routed("teleports") {
                    return false;
                }
                let uuid = uuid_string(player);
                let r = self
                    .with_java_world(|| horizon_jvm::events::teleport(&uuid, dim, at, look, end));
                match r {
                    Ok(refused) => return refused,
                    Err(e) => log::warn!("Java mods: a teleport failed: {e}"),
                }
            }
            #[cfg(not(feature = "java-mods"))]
            {
                let _ = (player, dim, at, look, end);
            }
            false
        }

        /// The player acting now is about to get `criterion` of advancement `id`, having
        /// `done`: vanilla's `PlayerAdvancements.award` on the mirror, when a mod patches it
        /// (routed action `advancements`: Balm's, Puzzles Lib's advancement events). True
        /// when the mods refused it.
        pub(in crate::game) fn java_award_refused(
            &mut self,
            id: &str,
            criterion: &str,
            done: &[&str],
        ) -> bool {
            #[cfg(feature = "java-mods")]
            {
                if !java_mods_on() || !self.hears_players() || !super::java_routed("advancements") {
                    return false;
                }
                let uuid = self.acting_uuid_string();
                let r =
                    self.with_java_world(|| horizon_jvm::events::award(&uuid, id, criterion, done));
                match r {
                    Ok(refused) => return refused,
                    Err(e) => log::warn!("Java mods: an advancement failed: {e}"),
                }
            }
            #[cfg(not(feature = "java-mods"))]
            {
                let _ = (id, criterion, done);
            }
            false
        }

        /// [`Game::java_add_effect`] for `player` (None: the player acting now).
        pub(in crate::game) fn java_add_effect_to(
            &mut self,
            player: Option<u128>,
            effect: &horizon_client::effects::EffectInstance,
        ) -> bool {
            #[cfg(feature = "java-mods")]
            {
                // Only when a mod hooks addEffect (beacons and conduits ask every few seconds,
                // the turtle helmet every tick).
                if !java_mods_on() || !self.hears_players() || !super::java_routed("effects") {
                    return false;
                }
                let uuid = player
                    .or(self.lan_acting_uuid)
                    .map(uuid_string)
                    .unwrap_or_else(|| uuid_string(self.player_uuid()));
                let id = format!("minecraft:{}", effect.id);
                let (duration, amplifier) = (effect.duration, effect.amplifier);
                let (ambient, visible) = (effect.ambient, effect.visible);
                self.with_java_world(|| {
                    horizon_jvm::events::add_effect(
                        &uuid, &id, duration, amplifier, ambient, visible,
                    )
                })
                .unwrap_or(false)
            }
            #[cfg(not(feature = "java-mods"))]
            {
                let _ = (player, effect);
                false
            }
        }

        /// Whether Java mods patch a player's fall damage (`PlayerMotion::route_fall`).
        pub(in crate::game) fn java_fall_routed(&self) -> bool {
            #[cfg(feature = "java-mods")]
            {
                java_mods_on() && self.hears_players() && super::java_routed("fall_damage")
            }
            #[cfg(not(feature = "java-mods"))]
            {
                false
            }
        }

        /// The player acting now landed (a `FallEvent::Routed`): vanilla's `causeFallDamage`
        /// with the mods' patches on the mirror, Horizon's `safe` distance and
        /// `damage_multiplier` answering `calculateFallDamage`. The damage to deal; `None`
        /// when the mirror couldn't say (Horizon's own rule then).
        pub(in crate::game) fn java_fall(
            &mut self,
            distance: f64,
            multiplier: f32,
            stalagmite: bool,
            safe: f64,
            damage_multiplier: f64,
        ) -> Option<f32> {
            #[cfg(feature = "java-mods")]
            {
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| uuid_string(self.player_uuid()));
                let dim = self.dim.id();
                let r = self.with_java_world(|| {
                    horizon_jvm::events::fall(
                        &uuid,
                        dim,
                        distance,
                        multiplier,
                        stalagmite,
                        safe,
                        damage_multiplier,
                        false,
                    )
                });
                match r {
                    Ok(damage) => damage,
                    Err(e) => {
                        log::warn!("Java mods: fall damage: {e}");
                        None
                    }
                }
            }
            #[cfg(not(feature = "java-mods"))]
            {
                let _ = (distance, multiplier, stalagmite, safe, damage_multiplier);
                None
            }
        }

        /// The game broke `state` at `pos` after [`Game::java_break`] said `after`: the mods'
        /// injections at `destroyBlock`'s end.
        pub(in crate::game) fn java_after_break(&mut self, pos: [i32; 3], state: BlockStateId) {
            {
                if !java_mods_on() || !self.hears_players() {
                    return;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let state = state_string(self.pack.assets.registry, state);
                let dim = self.dim.id();
                let ran = self
                    .with_java_world(|| horizon_jvm::events::after_break(&uuid, dim, pos, &state));
                self.apply_java_actor_slots(&uuid);
                if let Err(e) = ran {
                    log::warn!("Java mods: after a block break: {e}");
                }
            }
        }

        /// The player acting now touches `orb` (their `takeXpDelay` is 0), when a Java mod
        /// patches experience orbs' pickup (Clumps): vanilla's `ExperienceOrb.playerTouch` with
        /// the patches, on the player's experience, which it sets. Some((count left, delay));
        /// None: the game picks it up itself.
        pub(in crate::game) fn java_orb_touch(
            &mut self,
            orb: &horizon_client::entities::ExperienceOrb,
        ) -> Option<(u32, i32)> {
            {
                if !java_mods_on() || !super::java_routed("experience_orbs") {
                    return None;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let dim = self.dim.id();
                let (p, xp) = (orb.position, self.experience);
                let touched = self.with_java_world(|| {
                    horizon_jvm::events::orb_touch(
                        &uuid,
                        dim,
                        [p.x, p.y, p.z],
                        orb.value as i32,
                        orb.count as i32,
                        xp.level,
                        xp.progress,
                        xp.total,
                    )
                });
                self.apply_java_actor_slots(&uuid);
                match touched {
                    Ok(Some(t)) => {
                        let level_before = self.experience.level;
                        self.experience.level = t.level.max(0);
                        self.experience.progress = t.progress.clamp(0.0, 1.0);
                        self.experience.total = t.total.max(0);
                        // giveExperienceLevels' level-up sound every fifth level.
                        if self.experience.level > level_before
                            && self.experience.level % 5 == 0
                            && self.ticks > self.last_level_up + 100
                        {
                            self.last_level_up = self.ticks;
                            let level = self.experience.level;
                            let volume = if level > 30 { 1.0 } else { level as f32 / 30.0 };
                            let at = self.eye_position();
                            self.audio.play(
                                "minecraft:entity.player.levelup",
                                at,
                                volume * 0.75,
                                1.0,
                            );
                        }
                        Some((t.count.max(0) as u32, t.delay))
                    }
                    Ok(None) => None,
                    Err(e) => {
                        log::warn!("Java mods: an experience orb's pickup failed: {e}");
                        None
                    }
                }
            }
        }

        /// The player acting now touches the dropped `item` (it may pick it up), when a Java
        /// mod patches item pickup: vanilla's `ItemEntity.playerTouch` with the patches; what
        /// it takes comes back as the player's slots. Some(count left); None: the game does it.
        pub(in crate::game) fn java_item_touch(
            &mut self,
            item: &horizon_client::entities::ItemEntity,
        ) -> Option<u32> {
            {
                if !java_mods_on() || !super::java_routed("item_pickup") {
                    return None;
                }
                let uuid = self
                    .lan_acting_uuid
                    .map(uuid_string)
                    .unwrap_or_else(|| self.java_local_id());
                let dim = self.dim.id();
                let snbt = horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(
                    &item.stack,
                ));
                let p = item.position;
                let left = self.with_java_world(|| {
                    horizon_jvm::events::item_touch(&uuid, dim, [p.x, p.y, p.z], &snbt)
                });
                self.apply_java_actor_slots(&uuid);
                match left {
                    Ok(left) => left.map(|l| l.max(0) as u32),
                    Err(e) => {
                        log::warn!("Java mods: an item's pickup failed: {e}");
                        None
                    }
                }
            }
        }

        /// Makes the blocks Java mods set in the level in place, in order, as `/setblock` does,
        /// and their drops and level events. Those of other dimensions wait for their level.
        pub(in crate::game) fn apply_java_level_writes(&mut self) {
            self.apply_java_drops();
            if self.mods.java.block_writes.is_empty() {
                return;
            }
            let dim = self.dim.id();
            let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.mods.java.block_writes)
                .into_iter()
                .partition(|(d, _, _)| d == dim);
            self.mods.java.block_writes = later;
            let registry = self.pack.assets.registry;
            // Writes into chunks Horizon hasn't loaded yet wait for them, as vanilla's setBlock
            // loads the chunk (One Block's join sets its grass at 0 64 0 before the spawn's
            // chunks are in; dropped, its safety check found air and put a random block there).
            // At most ~20 s each.
            static WAITING: std::sync::Mutex<
                Option<std::collections::HashMap<(String, [i32; 3]), u32>>,
            > = std::sync::Mutex::new(None);
            let mut waiting = WAITING.lock().unwrap_or_else(|e| e.into_inner());
            let waiting = waiting.get_or_insert_with(Default::default);
            for (d, pos, state) in now {
                if !self.world.loaded(pos[0], pos[2]) {
                    let key = (d.clone(), pos);
                    let waited = waiting.entry(key).or_insert(0);
                    *waited += 1;
                    if *waited < 400 {
                        self.mods.java.block_writes.push((d, pos, state));
                    } else {
                        log::debug!("Java mods: a mod's block at {pos:?} never loaded");
                    }
                    continue;
                }
                waiting.remove(&(d.clone(), pos));
                let parsed = match crate::game::render_control::parse_state(registry, &state) {
                    Ok(parsed) => parsed,
                    Err(e) => {
                        log::warn!("Java mods: a mod set {state}: {e}");
                        continue;
                    }
                };
                if let Err(e) = self.place_state(pos, parsed, false) {
                    log::debug!("Java mods: a mod's block at {pos:?}: {e:?}");
                }
            }
            waiting.retain(|_, w| *w < 400);
        }

        /// The items Java mods dropped and the level events they made in the level in place: Horizon's item entities, with
        /// the motion the mod gave them and vanilla's pickup delay (`ItemEntity.setDefaultPickUpDelay`).
        fn apply_java_drops(&mut self) {
            let dim = self.dim.id();
            let now: Vec<horizon_jvm::events::Action> = {
                let mut drops = JAVA_DROPS.lock().unwrap_or_else(|e| e.into_inner());
                if drops.is_empty() {
                    return;
                }
                let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut *drops)
                    .into_iter()
                    .partition(|(d, _)| d == dim);
                *drops = later;
                now.into_iter().map(|(_, a)| a).collect()
            };
            for drop in now {
                // A vanilla mob a mod added (Carry On putting a carried cow down): Horizon's own.
                if let horizon_jvm::events::Action::SpawnEntity { snbt, .. } = &drop {
                    match horizon_world::nbt::parse_snbt(snbt) {
                        Ok((tag, _)) => {
                            if self.load_mob(tag, None).is_none() {
                                log::debug!("Java mods: a mod added an entity Horizon can't load");
                            }
                        }
                        Err(e) => log::warn!("Java mods: a mod's entity: {e}"),
                    }
                    continue;
                }
                if let horizon_jvm::events::Action::LevelEvent {
                    id,
                    position,
                    data,
                    state,
                    except,
                    ..
                } = &drop
                {
                    // The state a 2001 names, by name: Horizon's own id for it.
                    let data = match state {
                        Some(state) => {
                            match crate::game::render_control::parse_state(
                                self.pack.assets.registry,
                                state,
                            ) {
                                Ok(state) => state.0 as i32,
                                Err(_) => continue,
                            }
                        }
                        None => *data,
                    };
                    // Its source player's game made it already (a routed break's 2001):
                    // vanilla's broadcast leaves that player out. The events queued before
                    // are played first, with their own.
                    let except = except.as_ref().and_then(|uuid| {
                        let conns = super::JAVA_CONNS.lock().unwrap_or_else(|e| e.into_inner());
                        conns.iter().find(|(u, _)| u == uuid).map(|(_, c)| *c)
                    });
                    if except.is_some() {
                        let _ = self.process_level_events();
                    }
                    self.mods.java.event_except = except;
                    self.level_event_at(*id, *position, data);
                    self.mods.java.event_except = None;
                    continue;
                }
                let horizon_jvm::events::Action::DropItem {
                    position,
                    velocity,
                    item,
                    count,
                    ..
                } = drop
                else {
                    continue;
                };
                // An experience orb a mod's code spawned (a routed break's ore): its value.
                if item == "#experience_orb" {
                    if count > 0 {
                        self.spawn_orbs(position, count as u32);
                    }
                    continue;
                }
                if self.res.items().get(&item).is_none() || count <= 0 {
                    log::debug!("Java mods: a mod dropped {count} {item}, not an item here");
                    continue;
                }
                let stack = horizon_client::items::ItemStack::new(&item, count as u32);
                self.spawn_item(stack, position, velocity, 10);
            }
        }

        /// A player broke (or placed) `state` at `pos`: the player acting now.
        pub(in crate::game) fn java_block_event(
            &mut self,
            pos: [i32; 3],
            state: BlockStateId,
            placed: bool,
        ) {
            if !self.hears_players() {
                return;
            }
            let who = self.lan_acting.map_or(Who::Local, Who::Conn);
            let state = self.state_string(state);
            self.mods.java.events.push(ModEvent::Block {
                who,
                pos,
                state,
                placed,
            });
        }

        /// A chat message a player sent.
        pub(in crate::game) fn java_player_chat(&mut self, who: Who, text: &str) {
            if self.hears_players() {
                self.mods
                    .java
                    .events
                    .push(ModEvent::Chat(who, text.to_owned()));
            }
        }

        /// `/line` typed by the window's own player in-frame: true when a Java mod owns it.
        pub(in crate::game) fn local_java_command(&mut self, line: &str) -> bool {
            if !self.hears_players() {
                return false;
            }
            let Some(me) = self.java_players(None).first().map(|p| p.id.clone()) else {
                return false;
            };
            self.java_command(None, &me, line)
        }

        /// `/line` typed by a connection's player: true when a Java mod owns it.
        pub(in crate::game) fn guest_java_command(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            line: &str,
        ) -> bool {
            if !self.hears_players() {
                return false;
            }
            let Some(g) = host.hub.guest(conn).filter(|g| g.placed) else {
                return false;
            };
            let id = uuid_string(g.uuid);
            self.java_command(Some(host), &id, line)
        }

        /// A line for one player's action bar (`SystemChat` with `overlay`); the window's own
        /// player in-frame gets it in chat.
        fn overlay_server_player(&mut self, host: Option<&mut LanHost>, id: &str, line: Text) {
            let local = self.options.execution_role.is_client()
                && self.remote.is_none()
                && self.player.is_some();
            if local && self.java_local_id() == id {
                self.add_java_chat_line(line);
                return;
            }
            let Some(host) = host.or(self.lan.as_mut()) else {
                return;
            };
            let to = host
                .hub
                .guests
                .iter()
                .find(|g| g.placed && uuid_string(g.uuid) == id);
            if let Some(conn) = to.map(|g| g.conn) {
                host.hub.send_to(
                    conn,
                    &C::SystemChat(play::SystemChat {
                        content: component_nbt(&line.component()),
                        overlay: true,
                    }),
                );
            }
        }

        /// Sends a Java mod's line to the player with this id: the window's own chat for the
        /// local player, a system message for a connection's. `host` is the hub when it is out
        /// of `self.lan`.
        fn tell_server_player(&mut self, host: Option<&mut LanHost>, id: &str, line: Text) {
            let local = self.options.execution_role.is_client()
                && self.remote.is_none()
                && self.player.is_some();
            if local && self.java_local_id() == id {
                self.add_java_chat_line(line);
                return;
            }
            let Some(host) = host.or(self.lan.as_mut()) else {
                return;
            };
            let to = host
                .hub
                .guests
                .iter()
                .find(|g| g.placed && uuid_string(g.uuid) == id);
            if let Some(conn) = to.map(|g| g.conn) {
                host.hub.send_to(
                    conn,
                    &C::SystemChat(play::SystemChat {
                        content: component_nbt(&line.component()),
                        overlay: false,
                    }),
                );
            }
        }
    }
}
