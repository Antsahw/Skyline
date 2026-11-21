//! Multiplayer: the game driven by a server connection instead of the local
//! simulation. Mirrors vanilla's ClientPacketListener (chunks, entities,
//! inventory, health, time, weather, chat, respawn and dimension changes),
//! LocalPlayer.sendPosition (movement packets) and MultiPlayerGameMode
//! (block breaking with sequence numbers, use-item, attacks, container
//! clicks). The player still moves with the local physics, as the vanilla
//! client predicts its own movement; the server stays authoritative for
//! everything else.
mod selector_metadata;
use selector_metadata::hurt_sound_for;
mod attack_target;
mod break_prediction;
mod fall_sound;
mod interaction_actions;
mod interaction_prediction;
mod level_format;
mod raw_ingress;
mod recipe_properties;
mod use_item;

mod block_entity_data;
#[cfg(test)]
mod block_entity_data_tests;
use super::raw_backlog::Received;
use super::remote_chunks::{ChunkDecoder, ChunkFormat, Next};
use super::render_tick::{mob_tracking_body, player_tracking_body};
use super::{Dim, Game, Mining, Screen, level_sounds::LevelSound};
use crate::game::api_values::TryIntoApi;
use horizon_client::actions::Action;
use horizon_client::first_person::{Hand, SwingAnimation};
use horizon_client::input_queue::InputQueue;
use horizon_client::{
    Result,
    blocks::Fluid,
    chunk::Chunk,
    containers::{BlockEntity, Furnace, FurnaceKind},
    entities::{Experience, ExperienceOrb, ItemEntity},
    hud::HudBatch,
    items::ItemStack,
    lan::data::{BIOME_REGISTRY, client_biomes},
    menu::Input,
    mobs::{Mob, MobType, Who},
    motion::{MotionWorld, PlayerMotion},
    net::{Event, Session},
    particles::TrackingTarget,
    random_source,
    save::SaveDir,
    screen_effects,
    sprint::Keys,
    streaming::{ChunkGenerator, Proto, Streaming},
    tab_list::SPECTATOR,
    text::Text,
};
use horizon_core::{geometry::Vec3, random::RandomSource};
use horizon_protocol::{
    entity_data::{ITEM_ENTITY_ITEM, Value},
    item::{Hashed, Stack},
    packets::{
        Empty,
        play::{self, Clientbound as C, Serverbound as S},
    },
    registry::{Builtin, DimensionType, Dynamic, SyncedNames, ceil_log2},
    rsa::RsaPublicKey,
    secure_chat::{self, Signature},
    text::{Language, plain},
};
use mod_api::InteractionKey;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use winit::keyboard::KeyCode;
use {
    horizon_core::nbt::Tag,
    horizon_mod_runtime::attachments::{Holder, ResidentAttachments},
    horizon_world::{
        level::InteractionResult,
        registry::BlockStateId,
        stepped_interpolation::{Path, PosRot, Start, SteppedInterpolation},
    },
};

mod abilities;
mod chat_received;
use chat_received::QueuedChat;
pub(in crate::game) mod client_level_tick;
mod entity_section_steps;
mod item_use;
mod queued_keys;
mod remote_attributes;
mod tab_entries;
mod tables;

<<<<<<< ours
#[path = "remote_chat_queue.rs"]
mod chat_queue;

mod java;

type BuiltinCache = Vec<(std::path::PathBuf, Arc<Builtin>)>;
static BUILTIN: std::sync::Mutex<BuiltinCache> = std::sync::Mutex::new(Vec::new());
||||||| base
#[path = "remote_chat_queue.rs"]
mod chat_queue;

type BuiltinCache = Vec<(std::path::PathBuf, Arc<Builtin>)>;
static BUILTIN: std::sync::Mutex<BuiltinCache> = std::sync::Mutex::new(Vec::new());
=======
pub(super) use super::villager_client::builtin_registries;
>>>>>>> theirs

mod chat_queue;

/// `ChatType`'s two decorations: how a message shows in the chat, and what the narrator says of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Decoration {
    Chat,
    Narration,
}

/// `ChatTypeDecoration.decorate` of a chat type's `chat` or `narration`
/// entry: its translation, with the sender, the target or the message as its
/// parameters, in its style. A chat type the registry lacks decorates as
/// `minecraft:chat` (`chat.type.text`, narrated `chat.type.text.narrate`).
fn chat_type_decorate(
    chat_type: Option<&Tag>,
    decoration: Decoration,
    bound: &play::ChatTypeBound,
    content: &serde_json::Value,
) -> serde_json::Value {
    use horizon_client::scoreboard::tag_to_json;
    let (field, default_key) = match decoration {
        Decoration::Chat => ("chat", "chat.type.text"),
        Decoration::Narration => ("narration", "chat.type.text.narrate"),
    };
    let decoration = chat_type.and_then(|t| t.get(field));
    let key = match decoration.and_then(|d| d.get("translation_key")) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => default_key.into(),
    };
    let parameter = |name: &str| match name {
        "sender" => tag_to_json(&bound.name),
        "target" => bound
            .target
            .as_ref()
            .map_or_else(|| serde_json::Value::from(""), tag_to_json),
        _ => content.clone(),
    };
    let with: Vec<serde_json::Value> = match decoration.and_then(|d| d.get("parameters")) {
        Some(Tag::List(list)) => list
            .iter()
            .map(|p| match p {
                Tag::String(s) => parameter(&s.to_string_lossy()),
                _ => serde_json::Value::from(""),
            })
            .collect(),
        _ => vec![parameter("sender"), parameter("content")],
    };
    let mut component = match decoration.and_then(|d| d.get("style")) {
        Some(style @ Tag::Compound(_)) => tag_to_json(style),
        _ => serde_json::json!({}),
    };
    component["translate"] = key.into();
    component["with"] = with.into();
    component
}

/// A text component the server sent, for the HUD.
pub(super) fn remote_text(tag: &Tag) -> horizon_client::text::Text {
    use horizon_client::scoreboard::{comp_text, tag_to_json};
    comp_text(&tag_to_json(tag))
}

/// `Player.getVoicePitch`: a constant 1.0
/// (`javap -p -c -constants net.minecraft.world.entity.player.Player`:
/// `0: fconst_1  1: freturn`), so a player's own hurt and death sounds are
/// neither raised for a baby nor jittered - `LivingEntity.getVoicePitch`, which
/// the mobs port, does both. It takes no random, so it draws nothing from the
/// level random either.
pub(super) fn player_voice_pitch() -> f32 {
    1.0
}

/// The live entity a bound sound names; absence means that object was removed.
fn entity_sound_state(
    id: i32,
    player: Option<(i32, [f64; 3], bool)>,
    mob: Option<horizon_client::sound_engine::SoundEntityState>,
    tracked: &HashMap<i32, Tracked>,
    dragon: Option<(i32, [f64; 3])>,
) -> Option<horizon_client::sound_engine::SoundEntityState> {
    use horizon_client::sound_engine::SoundEntityState;
    if let Some((player_id, position, silent)) = player
        && player_id == id
    {
        return Some(SoundEntityState {
            position,
            silent,
            removed: false,
        });
    }
    let mut state = mob.or_else(|| {
        tracked.get(&id).map(|t| SoundEntityState {
            position: t.position,
            silent: t.silent,
            removed: false,
        })
    })?;
    // The dragon's Mob is metadata storage; only Dragon follows its movement.
    if let Some((dragon_id, position)) = dragon
        && dragon_id == id
    {
        state.position = position;
    }
    Some(state)
}

/// `ClientboundOpenScreenPacket.title` as a container screen draws it: the
/// component's styles, and the `§` codes a literal holds (`StringDecomposer`).
fn menu_title(tag: &Tag) -> horizon_client::text::Text {
    remote_text(tag).resolve_legacy()
}

/// `ChatTrustLevel`; a modified message keeps the signed content it was changed from.
pub(super) enum ChatTrust {
    Secure,
    Modified(String),
    NotSecure,
    /// `GuiMessageTag.chatError`: the line `ChatListener.handleChatMessageError` adds.
    Error,
}
impl ChatTrust {
    /// `ChatTrustLevel.createTag`.
    fn tag(&self) -> Option<horizon_client::chat::Tag> {
        match self {
            Self::Secure => None,
            Self::Modified(original) => Some(horizon_client::chat::Tag::modified(original)),
            Self::NotSecure => Some(horizon_client::chat::Tag::not_secure()),
            Self::Error => Some(horizon_client::chat::Tag::chat_error()),
        }
    }
}

/// Streaming needs a generator; a server world never generates.
struct NoTerrain;
impl ChunkGenerator for NoTerrain {
    type Worker = ();
    fn worker(&self) {}
    fn proto(&self, _: &mut (), _: i32, _: i32) -> Proto {
        Box::new(())
    }
    fn finish(&self, _: Proto) -> Chunk {
        Chunk::empty(0, 0)
    }
}

/// A server entity the client tracks (position as last sent).
pub(super) struct Tracked {
    kind: String,
    position: [f64; 3],
    /// `Entity.getPositionCodec`'s base: the position the server last
    /// reported (AddEntity, MoveEntity, EntityPositionSync), which a move
    /// packet's delta counts from. A TeleportEntity moves the entity but not
    /// this base, so the server's next delta still lands where it means.
    codec_base: [f64; 3],
    silent: bool,
    /// `Entity.vehicle`: the entity it rides (`SetPassengers`), whatever either one's type.
    pub(in crate::game) vehicle: Option<u32>,
}

/// `Entity.startRiding` / `removePassenger` for a `SetPassengers` packet: the
/// listed entities ride `vehicle`, the ones that rode it and are no longer
/// listed got off. Every tracked entity counts, whatever its type, so a
/// server's item display or marker on a player hides that player's name tag
/// as vanilla's `isVehicle` does (#5666).
fn seat_passengers(entities: &mut HashMap<i32, Tracked>, vehicle: u32, passengers: &[i32]) {
    for (id, t) in entities.iter_mut() {
        if passengers.contains(id) {
            t.vehicle = Some(vehicle);
        } else if t.vehicle == Some(vehicle) {
            t.vehicle = None;
        }
    }
}
/// Smooth an entity toward a new position over three ticks (InterpolationHandler).
#[derive(Clone, Copy)]
/// `LinearInterpolationHandler`'s target: a position and rotation (yRot,
/// xRot) reached in `steps` ticks.
pub(super) struct Lerp {
    target: [f64; 3],
    rotation: Option<(f32, f32)>,
    steps: u32,
}

/// `NewMinecartBehavior.MinecartStep`, including the server's rail pitch.
#[derive(Clone, Copy)]
struct CartStep {
    position: Vec3,
    movement: Vec3,
    yaw: f32,
    pitch: f32,
    weight: f32,
}

/// The client's state of a minecart on `NewMinecartBehavior`: the steps the
/// server sent (`lerpSteps`), the ones being played over 3 ticks
/// (`currentLerpSteps`) and where the cart was when they began (`oldLerp`).
pub(super) struct CartLerp {
    pending: Vec<CartStep>,
    current: Vec<CartStep>,
    total_weight: f64,
    delay: i32,
    old: CartStep,
}

impl CartLerp {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            current: Vec::new(),
            total_weight: 0.0,
            delay: 0,
            old: CartStep {
                position: Vec3::ZERO,
                movement: Vec3::ZERO,
                yaw: 0.0,
                pitch: 0.0,
                weight: 0.0,
            },
        }
    }

    /// `NewMinecartBehavior.getCartLerpPosition`, `getCartLerpMovements` and
    /// `getCartLerpYRot` / `getCartLerpXRot` at `partial`: the step the weights
    /// put this moment in, and how far along it from the one before.
    fn at(&self, partial: f32) -> (Vec3, Vec3, f32, f32) {
        let alpha = ((3 - self.delay) as f32 + partial) / 3.0;
        let mut count_up = 0.0f32;
        let mut in_step = 1.0f32;
        let mut found = None;
        for (i, s) in self.current.iter().enumerate() {
            if s.weight <= 0.0 {
                continue;
            }
            count_up += s.weight;
            if f64::from(count_up) >= self.total_weight * f64::from(alpha) {
                let before = count_up - s.weight;
                in_step = ((f64::from(alpha) * self.total_weight - f64::from(before))
                    / f64::from(s.weight)) as f32;
                found = Some(i);
                break;
            }
        }
        let index = found.unwrap_or(self.current.len() - 1);
        let current = self.current[index];
        let previous = if index > 0 {
            self.current[index - 1]
        } else {
            self.old
        };
        let t = f64::from(in_step);
        let lerp = |a: Vec3, b: Vec3| {
            Vec3::new(
                a.x + t * (b.x - a.x),
                a.y + t * (b.y - a.y),
                a.z + t * (b.z - a.z),
            )
        };
        (
            lerp(previous.position, current.position),
            lerp(previous.movement, current.movement),
            previous.yaw + in_step * wrap_degrees(current.yaw - previous.yaw),
            previous.pitch + in_step * wrap_degrees(current.pitch - previous.pitch),
        )
    }

    /// `NewMinecartBehavior.lerpClientPositionAndRotation`: once the steps
    /// being played are used up, the queued ones become the current ones;
    /// the cart then sits where the steps put it at the end of this tick.
    /// False when it has no steps to play.
    fn tick(&mut self, c: &mut super::minecarts::Minecart) -> bool {
        self.delay -= 1;
        if self.delay <= 0 {
            self.old = CartStep {
                position: c.position,
                movement: c.velocity,
                yaw: c.yaw,
                pitch: c.pitch,
                weight: 0.0,
            };
            self.current.clear();
            if !self.pending.is_empty() {
                self.current.append(&mut self.pending);
                self.total_weight = self.current.iter().map(|s| f64::from(s.weight)).sum();
                self.delay = if self.total_weight == 0.0 { 0 } else { 3 };
            }
        }
        if self.current.is_empty() {
            return false;
        }
        let (position, movement, yaw, pitch) = self.at(1.0);
        c.position = position;
        c.velocity = movement;
        c.yaw = yaw;
        c.pitch = pitch;
        true
    }
}

/// `SteppedInterpolationHandler.doInterpolate`'s `setPos` and `setRot` on a
/// mob: its yaw kept continuous (the same turn as vanilla's, without the
/// wrap that would spin it in the frame between ticks), its pitch clamped.
fn place_interpolated(m: &mut Mob, at: PosRot) {
    m.position = Vec3::new(at.position[0], at.position[1], at.position[2]);
    m.yaw += wrap_degrees(at.yaw - m.yaw);
    m.head_pitch = (at.pitch % 360.0).clamp(-90.0, 90.0);
}

/// The packet a living entity's move came in.
#[derive(Clone, Copy, PartialEq)]
enum MovePacket {
    /// `handleMoveEntity`: always interpolated.
    Move,
    /// `handleEntityPositionSync`.
    PositionSync,
    /// `handleTeleportEntity` (`setValuesFromPositionPacket`).
    Teleport,
}

impl MovePacket {
    /// `tooBigToInterpolate`: a move to `to` of more than 64 blocks snaps.
    /// A position sync measures it from where the entity is, a teleport from
    /// where it is headed (`PositionMoveRotation.of`'s
    /// `getClientPositionAndRotation`).
    fn snaps(self, handler: &SteppedInterpolation, entity: PosRot, to: [f64; 3]) -> bool {
        let from = match self {
            MovePacket::Move => return false,
            MovePacket::PositionSync => entity.position,
            MovePacket::Teleport => handler.heading(entity).position,
        };
        let d = [0, 1, 2].map(|i| to[i] - from[i]);
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > 4096.0
    }
}

/// A move `packet` to `to` for a living entity at `entity`, through its
/// handler (`interpolateTo`): where it is put at once, if anywhere. A move
/// too big to interpolate is a `snapTo`; vanilla leaves the handler running
/// and its next step shifts what is left by the jump
/// (`adjustInterpolationTargetFromDeltas`), which this handler doesn't port,
/// so the steps are dropped instead. That is the same for an entity that
/// wasn't moving, as a teleported one usually isn't.
fn stepped_move(
    handler: &mut SteppedInterpolation,
    packet: MovePacket,
    entity: PosRot,
    to: [f64; 3],
    path: Option<Path>,
    rotation: Option<(f32, f32)>,
) -> Option<PosRot> {
    if packet.snaps(handler, entity, to) {
        handler.cancel();
        let (yaw, pitch) = rotation.unwrap_or((entity.yaw, entity.pitch));
        return Some(PosRot {
            position: to,
            yaw,
            pitch,
        });
    }
    match handler.interpolate_to(entity, path, rotation) {
        Start::Snap(at) => Some(at),
        Start::Interpolating | Start::Unchanged => None,
    }
}

/// `Minecraft.services().profileKeySignatureValidator()`: the services' profile-key keys, which a
/// player's chat key must be signed by. Asked for once per connection to a server, off the game
/// thread.
pub(super) enum ServicesKeys {
    Pending,
    Ready(Vec<RsaPublicKey>),
    /// `canValidateProfileKeys()` is false: none could be had.
    Missing,
}
/// The signed-in account's `/minecraft/profile` fetch after joining.
enum OwnProfile {
    Idle,
    Fetching(std::sync::mpsc::Receiver<Option<horizon_client::skins::Textures>>),
    Done(Option<horizon_client::skins::Textures>),
}

pub(super) struct Remote {
    pub(super) professions: Arc<horizon_client::mobs::villager::ResolvedProfessions>,
    pub session: Session,
    pub brand: String,
    pub(super) recipe_viewer: crate::game::recipe_book::ViewerChannels,
    /// This connection's configured feature flags, carried through reconfiguration.
    pub(super) features: std::sync::Arc<[String]>,
    /// The server announced it runs mods' quiet commands.
    pub quiet_commands: bool,
    /// The server advertised private custom recipe-book synchronization.
    pub custom_books: bool,
    /// The synced attachments the server sent, by holder.
    pub attachments: HashMap<Holder, ResidentAttachments>,
    /// The server samples far terrain for us, this far at most (`lod_net`).
    pub far_terrain: Option<horizon_client::lod_net::Offer>,
    pub(super) builtin: Arc<Builtin>,
    lang: Arc<Language>,
    pub(in crate::game) registries: Arc<Dynamic>,
    /// The block, item and fluid tags of `registries` by name, as native mods read them
    /// (`install_remote_tags`).
    pub(super) tags: Option<horizon_client::items::Items>,
    /// The game's enchantments and the server's that it lacks (a data
    /// pack's), from their synced data; `None` when the server has none.
    pub(super) enchantments: Option<Arc<horizon_client::enchant::Enchantments>>,
    pub(super) joined: bool,
    /// `ServerReconfigScreen` is up since this tick.
    pub(super) reconfiguring: Option<u64>,
    /// `handleLogin` ran: vanilla's `Minecraft.player` exists from then on,
    /// while here the player is made at the first teleport.
    pub(super) logged_in: bool,
    status: String,
    pub(super) entity_id: i32,
    local_entity_silent: bool,
    /// `Minecraft.cameraEntity` when the server put the view on another
    /// entity (ClientboundSetCameraPacket); None while it is the player.
    pub(super) camera: Option<i32>,
    /// The item the player's use began with (`LivingEntity.useItem`).
    pub(super) use_item: Option<Arc<str>>,
    pub(super) dimension: DimensionType,
    /// The level's key (`minecraft:overworld`): where its far terrain is kept.
    pub(in crate::game) level: String,
    /// `level`'s key, as mods' handles name the level.
    pub(super) level_key: mod_api::DimensionKey,
    /// The level's facts from the dimension type this connection's server sent (`None` when it
    /// sent none the game takes): owned by the connection, so entering a level touches no other
    /// level's facts, the integrated server's included.
    pub(in crate::game) level_facts: Option<Arc<horizon_world::dimension_type::DimensionFacts>>,
    /// The sky, fog and cloud attributes (and timelines) of the level from the server's type, for a
    /// level of a mod or a pack (`None` for a built-in level, or when the type is refused).
    pub(in crate::game) level_environment: Option<horizon_client::environment::Environment>,
    /// Connection-owned facts, cached by level and the full named type identity.
    facts_cache: HashMap<
        String,
        (
            String,
            serde_json::Value,
            Arc<horizon_world::dimension_type::DimensionFacts>,
        ),
    >,
    /// The levels this connection added to the process's table of levels: the only ones it drops.
    added_levels: std::collections::HashSet<String>,
    /// `ClientPlayerNetworkEvent.LoggingOut` was fired for this connection: every way out
    /// of a world reports it once.
    logged_out: bool,
    /// The other levels' worlds the server opened on `horizon:levels` (`viewed_worlds`).
    pub(in crate::game) viewed: super::viewed_worlds::ViewedWorlds,
    /// The level's hashed seed (login, respawn): the folder of its kept chunks
    /// (`server_terrain::level_dir`).
    pub(super) hashed_seed: i64,
    /// What chunk packets decode with (the dimension's height and sky, the
    /// biome registry); replaced whenever one of them changes.
    pub(super) chunk_format: Arc<ChunkFormat>,
    /// Decodes chunk packets as they arrive, off the frame.
    decoder: ChunkDecoder,
    /// Options.serverRenderDistance: the server's view distance from the
    /// login or the last chunk-cache-radius packet.
    pub(super) view_distance: i32,
    /// `ClientLevel.serverSimulationDistance`: from the login or the last
    /// simulation-distance packet.
    pub(super) simulation_distance: i32,
    /// `BlockStatePredictionHandler.currentSequenceNr`: the level's, so a new level (a login,
    /// another dimension) counts from 0 again.
    sequence: i32,
    /// `BlockStatePredictionHandler.lastTeleportSequence`: the sequence of the last
    /// teleport, so a prediction made before it keeps its block but not its snap.
    last_teleport_sequence: i32,
    pub(super) position_sender: horizon_client::player_sync::PositionSender,
    last_input: u8,
    was_sprinting: bool,
    /// The slot the server last heard (`MultiPlayerGameMode.carriedIndex`): a cell, so `send` can
    /// send a new one first (`ensureHasSentCarriedItem`).
    carried: std::cell::Cell<Option<usize>>,
    was_shared_flags: u8,
    was_living_flags: u8,
    last_recorded_pose: Option<crate::pose::Pose>,
    /// The last ClientboundPlayerAbilitiesPacket. Vanilla's LocalPlayer
    /// holds its abilities from the login on; here the player is made at
    /// the first teleport, which takes them then.
    abilities: Option<play::PlayerAbilities>,
    loaded_sent: bool,
    /// The dimension of the client level the mods heard made
    /// (`native_client_level_load`), to be told dropped once.
    pub(super) heard_level: Option<String>,
    /// `ClientPacketListener.levelLoadTracker`: the loading screen after a login
    /// or respawn, until the player's section is shown.
    pub(super) level_load: horizon_client::level_loading::LevelLoadTracker,
    /// The ClientInformation last sent (`Options.broadcastOptions` sends it
    /// again when a view distance, Chat, skin or Main Hand option changes).
    sent_info: horizon_protocol::packets::configuration::ClientInformation,
    /// BlockStatePredictionHandler: blocks changed ahead of the server (a predicted break),
    /// with what it remembers of each of them. Until the server acknowledges the
    /// sequence its block updates only replace the remembered state; then the block
    /// takes it.
    predictions: HashMap<[i32; 3], Prediction>,
    /// The entities the level held when its `LocalPlayer` was added
    /// (`ClientLevel.addEntity` on login or respawn): `ClientLevel.tickEntities`
    /// ticks in the order entities were added, so these tick before the
    /// player and every later one after it.
    ticked_before_player: std::collections::HashSet<u32>,
    /// Open container: (window id, menu name).
    pub(super) menu: ClientMenu,
    /// The crafting result slot of the open menu as the server last set it
    /// (`CraftingMenu.slotChangedCraftingGrid`): the client never computes it.
    pub(super) craft_result: Option<ItemStack>,
    /// The open menu's title as the server named it, with its styles.
    menu_title: Option<horizon_client::text::Text>,
    menu_title_snapshot: Option<Arc<mod_api::Text>>,
    pub(in crate::game) state_id: i32,
    use_target: Option<[i32; 3]>,
    /// A mod menu's extra data, kept for the `OpenScreen` of its window that
    /// follows (`horizon:menu_extra`).
    pub(in crate::game) menu_extra: Option<horizon_protocol::menu_extra::Message>,
    pub(super) entities: HashMap<i32, Tracked>,
    /// Per-tick remote entity scratch retained between frames.
    tick_fluids: Vec<Option<horizon_client::mobs::living::Fluids>>,
    tick_rain: Vec<bool>,
    tick_standings: Vec<Option<String>>,
    tick_water_fx: Vec<horizon_client::mobs::MobEvent>,
    /// Complete network riding graph, including invisible plugin seat entities.
    pub(super) passenger_vehicles: HashMap<u32, u32>,
    pub(super) lerps: HashMap<i32, Lerp>,
    /// The minecarts the server moves with `ClientboundMoveMinecartPacket`
    /// (`NewMinecartBehavior`), which have no interpolation handler.
    pub(super) cart_lerps: HashMap<i32, CartLerp>,
    /// Living entities' `SteppedInterpolationHandler`s.
    pub(super) stepped: HashMap<i32, SteppedInterpolation>,
    /// `LivingEntity.lerpHeadTo`: the head yaw and the ticks left to reach it.
    pub(super) head_lerps: HashMap<i32, (f32, u32)>,
    /// The happy ghast the player just got off, until the server's teleport
    /// puts the player on top of it (`place_ghast_under`).
    pub(super) ghast_dismount: Option<u32>,
    /// Whether the player is the first passenger of its vehicle, the one
    /// `getControllingPassenger` names.
    pub(super) first_passenger: bool,
    /// The player's place in its vehicle's `getPassengers`.
    pub(super) passenger_index: usize,
    /// Item entities waiting for their item data: where they are and their motion.
    pub(super) pending_items: HashMap<i32, ([f64; 3], Vec3)>,
    health: (f32, i32, f32),
    /// `LocalPlayer.flashOnSetHealth`: set by the first health the server
    /// sends; from then on a drop starts the hurt animation.
    flash_on_set_health: bool,
    game_mode: i32,
    pub chat: Vec<(String, Instant)>,
    /// Lines received since joining, for the control socket's events.
    pub chat_received: u64,
    /// `ChatListener.delayedMessageQueue`: player chat held back by Chat
    /// Delay, with the signature a deletion names it by and what the
    /// narrator says once it shows.
    chat_queue: VecDeque<QueuedChat>,
    /// `ClientPacketListener.messageSignatureCache`: the signatures packed
    /// ones index, as the server's copy for this connection holds them.
    pub(super) signatures: horizon_protocol::secure_chat::SignatureCache,
    /// `ChatListener.previousMessageTime`: when a message last showed, if one has.
    chat_previous: Option<Instant>,
    /// `ClientPacketListener.nextChatIndex`: the global index the next chat packet must carry.
    next_chat_index: i32,
    /// `serverEnforcesSecureChat`, from the login packet.
    enforces_secure_chat: bool,
    services_keys: Arc<std::sync::Mutex<ServicesKeys>>,
    /// The `RemoteChatSession.Data` each player's list entry last carried (INITIALIZE_CHAT).
    chat_sessions: HashMap<u128, Option<(u128, i64, Vec<u8>, Vec<u8>)>>,
    /// `PlayerInfo.messageValidator` of the players whose chat has been read.
    chat_validators: HashMap<u128, secure_chat::MessageValidator>,
    /// `ConnectScreen.lastNarration`: when "Joining the world" was last said.
    joining_narrated: Option<Instant>,
    /// The tree the chat colours, completes and parses with: the server's
    /// (ClientboundCommandsPacket) once it has sent one, else the game's own,
    /// plus the commands native mods own (`chat::chat_tree_for`). Rebuilt where
    /// that base changes, not at each keystroke.
    pub chat_tree: horizon_client::command::Dispatcher,
    selector_metadata_supported: bool,
    /// Whether a `ClientboundCommandsPacket` has supplied the server's tree.
    /// The local fallback is for UI completion only; vanilla signs command
    /// arguments against an empty tree until the server sends one.
    pub(super) chat_tree_received: bool,
    /// The chat session, the last-seen window and the signature cache.
    pub(super) secure_chat: super::remote_chat::SecureChat,
    /// `ClientSuggestionProvider.customCompletionSuggestions`: the extra chat
    /// completions this server sent (ClientboundCustomChatCompletionsPacket).
    custom_completions: std::collections::BTreeSet<String>,
    players: HashMap<u128, String>,
    /// `ClientPacketListener.playerInfoMap` as player-list entries, and which of them are listed.
    tab_players: HashMap<u128, horizon_client::tab_list::TabPlayer>,
    /// Each listed profile's unpacked `textures` property.
    skin_textures: HashMap<u128, horizon_client::skins::Textures>,
    /// Active skin and cape from the services profile when the server sent none.
    own_profile: OwnProfile,
    /// The skin texture each profile's players show, once its download is ready.
    skins: HashMap<u128, String>,
    skins_revision: u64,
    published_skins: Option<(usize, u128, u64, u64)>,
    listed: std::collections::HashSet<u128>,
    /// `ClientboundLoginPacket.onlineMode`: the player list shows heads.
    pub(super) online_mode: bool,
    /// `ClientboundLoginPacket.levels`: the server's level keys, which
    /// `ClientPacketListener.levels()` hands to command suggestions.
    pub(super) levels: Vec<String>,
    /// Player entities in the level: entity id to profile UUID.
    pub(super) level_players: HashMap<i32, u128>,
    /// The profile name each player entity was created with (`RemotePlayer.getGameProfile`): it outlives the
    /// player-list entry, which servers remove to hide their NPCs from the list.
    level_player_names: HashMap<i32, String>,
    /// The `textures` each player entity found in the player list first
    /// (`AbstractClientPlayer.getPlayerInfo` keeps that `PlayerInfo`): its skin
    /// outlives the entry, which NPC plugins remove a moment after spawning.
    level_player_textures: HashMap<i32, horizon_client::skins::Textures>,
    /// `ClientPacketListener.seenPlayers`: the player-info entries of the
    /// players whose entity this connection has added. It outlives an entity's
    /// removal and a dimension change, as vanilla's does.
    seen_players: std::collections::HashSet<u128>,
    /// Entities with shared flag 6 (`Entity.isCurrentlyGlowing` on a client); mobs keep
    /// theirs in `Mob::glowing`.
    pub(super) glowing: std::collections::HashSet<i32>,
    /// The player's own shared flag 5 (`Entity.isInvisible`) as the server last set it.
    pub(super) own_invisible: bool,
    /// The player's own shared flag 0 (`Entity.isOnFire`) as the server last
    /// set it: its fire (blaze fireballs, fire aspect, lightning) never
    /// reaches the window's own `fire_ticks` otherwise.
    own_on_fire: bool,
    /// `LivingEntity.DATA_ARROW_COUNT_ID` and `DATA_STINGER_COUNT_ID` of the
    /// own player, as the server synced them.
    pub(super) own_arrows: i32,
    pub(super) own_stingers: i32,
    /// `Player.DATA_SHOULDER_PARROT_LEFT` and `_RIGHT` of the own player.
    pub(super) own_shoulders: [Option<i32>; 2],
    /// The local player's profile UUID (always in the level).
    pub(super) profile: u128,
    /// The local player's profile name: its score holder and team member.
    pub(super) profile_name: String,
    /// `MultiPlayerGameMode.destroyBlockPos`, `.destroyProgress` and the face last sent: the block
    /// the break targets, which outlives the break itself.
    destroying: Option<([i32; 3], u8, f32)>,
    /// `MultiPlayerGameMode.destroyingItem`: the main-hand stack the break began with.
    destroying_item: Option<ItemStack>,
    /// `MultiPlayerGameMode.isDestroying`: a break is running. `destroying` keeps the target after
    /// one ends (`destroyBlockPos` is written only when a break starts), so a dig resumed on the
    /// same block and item sends no new START and the server keeps its clock (arismoko/horizon#4124).
    is_destroying: bool,
    /// Per-hand swing policy, restored when the current input attempt ends.
    pub(in crate::game) interaction_swing: [bool; 2],
    destroy_delay: u32,
    pub(super) input: horizon_client::interaction_input::InteractionInput,
    /// Keys whose presses send a packet (swap hands, drop, a mount's
    /// inventory): like the click queue, they wait for the tick's
    /// `handleKeybinds` phase rather than going out between window events.
    pub(super) pending_keys: InputQueue<KeyCode>,
    /// Whether the main and off hand still wait for the server's answer to
    /// a SWAP_ITEM_WITH_OFFHAND (`remote_hand_slot_set`).
    swap_answer: [bool; 2],
    stop_mining: bool,
    placing: Option<([i32; 3], u64)>,
    /// `CommonPlayerSpawnInfo.isFlat`: the level is a superflat one.
    pub(super) flat: bool,
    death_message: Option<horizon_client::text::Text>,
    /// `LocalPlayer.showDeathScreen`: off with `immediate_respawn`.
    show_death_screen: bool,
    /// `Player.isReducedDebugInfo`: the server's `reduced_debug_info` rule.
    pub(super) reduced_debug_info: bool,
    /// `LocalPlayer.doLimitedCrafting`: whether limited crafting is enabled.
    pub(super) do_limited_crafting: bool,
    player_type: Option<Arc<MobType>>,
    /// Inventory, off hand and armour as last synchronised, for creative-mode
    /// slot updates (`creative_sync_stacks`).
    synced: Vec<Option<ItemStack>>,
    decode_errors: u32,
    /// Connection-owned `ClientClockManager` state, retained across dimensions.
    clocks: HashMap<i32, play::ClockState>,
    default_clock: Option<i32>,
    overworld_clock: Option<i32>,
    clock_game_time: i64,
    /// `ClientChunkCache.Storage`'s view centre (ClientboundSetChunkCacheCenterPacket).
    chunk_center: (i32, i32),
    /// Received events a frame left for the next one, in order, and when
    /// the oldest of them first waited (`poll_remote`).
    backlog: VecDeque<Received>,
    backlog_since: Option<Instant>,
    /// `ClientboundTransferPacket`'s target: when the connection ends the
    /// game connects here, the stored cookies carried over (`TransferState`).
    transfer: Option<String>,
    /// `ClientCommonPacketListenerImpl.serverLinks`: the links "Server
    /// Links..." in the pause menu lists (only `http`/`https` URIs kept).
    pub(super) server_links: Vec<horizon_protocol::packets::configuration::ServerLink>,
    /// `ClientPacketListener.getDebugQueryHandler`: the F3+I query waiting for the server's data.
    pub(super) debug_query: super::debug_keys::DebugQuery,
}

/// The time a frame spends on chunk packets before it leaves the rest of
/// the received events for the next frame. A batch of 25 to 60 columns
/// (0.1 ms each on an M4 Pro) all in one frame was a 3 to 8 ms frame every
/// time the player crossed a chunk border at 32 chunks; they decode on the
/// chunk decoder's thread now, and the frame only inserts them.
const CHUNK_PACKETS_PER_FRAME: Duration = Duration::from_millis(1);
/// Events that have waited this long are all handled in the next frame,
/// whatever inserting their columns costs, up to a column still decoding:
/// the packets after a slow machine's chunks don't wait on its frames.
const BACKLOG_LIMIT: Duration = Duration::from_millis(50);
/// The open menu name of a server's horse inventory, then its chest's
/// columns (the packet names no `MenuType`).
const MOUNT_MENU: &str = "horizon:mount";
/// ServerboundPlayerInputPacket's shift bit.
const SHIFT_INPUT: u8 = 1 << 5;

fn face_direction(face: [i32; 3]) -> u8 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        [1, 0, 0] => 5,
        _ => 1,
    }
}

/// A protocol number format as the scoreboard's.
fn number_format(f: &play::NumberFormat) -> horizon_client::scoreboard::NumberFormat {
    use horizon_client::scoreboard::{NumberFormat, tag_to_json};
    match f {
        play::NumberFormat::Blank => NumberFormat::Blank,
        play::NumberFormat::Styled(t) => NumberFormat::Styled(tag_to_json(t)),
        play::NumberFormat::Fixed(t) => NumberFormat::Fixed(tag_to_json(t)),
    }
}

mod apply_packet;
mod blocks;
mod chat;
mod connection;
mod entities;
mod entity_updates;
mod input;
mod level_state;
mod menu_slots;
mod menus;
mod mining;
mod player_state;
mod polling;
mod scoreboard;
mod screens;
mod skins;
mod tick;
mod tick_entities;
mod use_prediction;
mod wire;

// The `Game` methods moved into the files above stay here as the mod points they were, by name, module path and
// signature (docs/layers.md#mod-points-stay-put); each forwards to its body in its file.
impl Game<'_> {
    // connection.rs
    pub(super) fn remote_sound_name(
        &self,
        sound: &(i32, Option<(String, Option<f32>)>),
    ) -> Option<String> {
        self.remote_sound_name_body(sound)
    }
    pub(super) fn remote_sound_entity_state(
        &mut self,
        id: i32,
    ) -> Option<horizon_client::sound_engine::SoundEntityState> {
        self.remote_sound_entity_state_body(id)
    }
    pub(super) fn remote_hurt_sound(&self, source_type: i32) -> &'static str {
        self.remote_hurt_sound_body(source_type)
    }
    pub(super) fn connect_server(&mut self, address: &str, name: &str) -> Result<()> {
        self.connect_server_body(address, name)
    }
    pub(super) fn connect_server_with_cookies(
        &mut self,
        address: &str,
        name: &str,
        cookies: Option<horizon_client::net::Cookies>,
    ) -> Result<()> {
        self.connect_server_with_cookies_body(address, name, cookies)
    }
    pub(super) fn connect_server_with_options(
        &mut self,
        address: &str,
        name: &str,
        cookies: Option<horizon_client::net::Cookies>,
        lan: bool,
        realm: bool,
    ) -> Result<()> {
        self.connect_server_with_options_body(address, name, cookies, lan, realm)
    }
    pub(super) fn connect_remote_session(
        &mut self,
        address: &str,
        name: &str,
        local: Option<horizon_protocol::connection::Pipe>,
        cookies: Option<horizon_client::net::Cookies>,
        lan: bool,
        realm: bool,
    ) -> Result<()> {
<<<<<<< ours
        self.realm_session = realm;
        self.reset_tutorial();
        self.perf.world_opening();
        // A server's own biomes come over the connection; the last local
        // world's packed table must not name its ids. The window's own
        // server thread keeps the one it just set.
        if local.is_none() {
            self.world_biomes = None;
            self.world_biomes_base = None;
        }
        let root = self.options.resources.clone();
        let builtin = builtin_registries(&root).ok_or("Resources are incomplete: registries.json is missing or invalid. Click \"Prepare again\" in the launcher (or run horizon-prepare), then try again.")?;
        let lang = Arc::new(horizon_client::net::load_language(&root));
        // Options.createInformation: what the server learns about this player,
        // sent again when one of those options changes.
        let info = self
            .settings
            .client_information(client_view_distance(self.settings.render_distance));
        if local.is_none() && !self.options.split_guest {
            horizon_client::crash::set_detail(
                "Server",
                horizon_client::net::shown_address(address),
            );
        }
        // A signed-in account always plays under its own name; a splitscreen player signs in
        // with its own account when it picked one.
        let auth = self.auth();
        let name = auth.as_ref().map_or(name, |a| a.name.as_str());
        let user = auth.as_ref().map_or_else(
            || {
                (
                    name.to_owned(),
                    horizon_protocol::crypto::offline_uuid(name),
                )
            },
            |a| (a.name.clone(), a.uuid),
        );
        self.server_packs_connect(local.is_none().then_some(address), user);
        let session = match local {
            Some(pipe) => Session::connect_local(
                pipe,
                name,
                auth.clone(),
                info.clone(),
                lang.clone(),
                root.clone(),
            ),
            None => {
                self.extras_connecting(address, name, lan);
                match cookies {
                    Some(cookies) => Session::connect_transferred(
                        address,
                        name,
                        auth.clone(),
                        info.clone(),
                        lang.clone(),
                        cookies,
                        root.clone(),
                    ),
                    None => {
                        let connect = if lan {
                            Session::connect_lan
                        } else {
                            Session::connect
                        };
                        connect(
                            address,
                            name,
                            auth.clone(),
                            info.clone(),
                            lang.clone(),
                            root.clone(),
                        )
                    }
                }
            }
        };
        let player_type = super::perspective::player_type(&root);
        let mut streaming = Streaming::new(
            Arc::new(NoTerrain),
            self.worker_context(),
            self.res.threads(),
        );
        streaming.remote = true;
        streaming.radius = self.settings.render_distance;
        self.streaming = streaming;
        self.world = World::new(self.res.facts().air, 0);
        self.save = Arc::new(SaveDir::new(
            std::env::temp_dir().join("horizon-remote-unused"),
            horizon_client::save::LevelShape::OVERWORLD,
        ));
        self.inventory = vec![None; 36];
        self.offhand = None;
        self.armor = [None, None, None, None];
        self.craft_grid = vec![None; 9];
        self.craft_size = 2;
        self.player = None;
        self.spawn = None;
        self.mobs.clear();
        self.clear_remote_objects();
        self.clear_remote_projectiles();
        self.creatures.projectiles.clear();
        self.dropped.clear();
        self.particles.clear();
        self.pickups.clear();
        self.falling.clear();
        self.mining = None;
        self.container = None;
        self.carried = None;
        self.using = None;
        self.end_mod_use();
        self.clicks.miss_time = 0;
        // A new LocalPlayer has no effects: the server sends what it has
        // (ClientboundUpdateMobEffectPacket on placement).
        self.potions = Default::default();
        self.set_mode_id(0);
        self.experience = Experience {
            level: 0,
            progress: 0.0,
            total: 0,
            score: 0,
        };
        // The connected client never opens a server world (single player also
        // joins here), so load the registries its argument parsers and local
        // suggestions need before receiving the server's command tree.
        self.ui.registries = Default::default();
        self.build_registries();
        self.ui.registries.permission = 0;
        self.ui.chat.clear();
        self.screen = Screen::Playing;
        self.title = None;
        self.dim = Dim::Overworld;
        self.clear_lod();
        self.pending_uploads.clear();
        if let Some(r) = self.renderer.as_mut() {
            r.clear_sections(self.world.layout());
        }
        self.apply_settings();
        self.clear_waypoints();
        self.boss_overlay.reset();
        self.java_new_server();
        // The chat's tree until the server sends its own, or forever when it
        // sends none: the game's own, plus the commands native mods own.
        let chat_tree = self.chat_tree_for(None);
        // Minecraft.services() has the keys wherever the game is played, single player's own
        // server included: its guests' chat keys are checked against them.
        let services_keys = Arc::new(std::sync::Mutex::new(ServicesKeys::Pending));
        let slot = services_keys.clone();
        let _ = std::thread::Builder::new()
            .name("Services keys".into())
            .spawn(move || {
                use horizon_server_files::profiles;
                let keys = profiles::fetch_profile_key_keys(&profiles::discovery_url());
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = match keys {
                    Ok(keys) if !keys.is_empty() => ServicesKeys::Ready(keys),
                    Ok(_) => ServicesKeys::Missing,
                    Err(why) => {
                        log::error!("Horizon: {why}");
                        ServicesKeys::Missing
                    }
                };
            });
        self.remote = Some(Remote {
            session,
            save_folder: None,
            brand: "unknown".into(),
            recipe_transfer: false,
            features: vec!["minecraft:vanilla".into()],
            quiet_commands: false,
            mod_net: false,
            attachments: HashMap::new(),
            far_terrain: None,
            builtin,
            status: lang
                .get("connect.connecting")
                .cloned()
                .unwrap_or_else(|| "Connecting to the server...".into()),
            lang,
            registries: Arc::new(Dynamic::default()),
            tables: Default::default(),
            joined: false,
            reconfiguring: None,
            logged_in: false,
            entity_id: 0,
            local_entity_silent: false,
            camera: None,
            use_item: None,
            dimension: DimensionType::vanilla("minecraft:overworld"),
            level: "minecraft:overworld".into(),
            hashed_seed: 0,
            chunk_format: Arc::new(ChunkFormat::new(
                &DimensionType::vanilla("minecraft:overworld"),
                Vec::new(),
                7,
            )),
            decoder: ChunkDecoder::new(),
            view_distance: self.settings.render_distance,
            simulation_distance: self.settings.simulation_distance,
            sequence: 0,
            last_teleport_sequence: -1,
            position_sender: Default::default(),
            last_input: 0,
            was_sprinting: false,
            carried: std::cell::Cell::new(None),
            was_shared_flags: 0,
            was_living_flags: 0,
            last_recorded_pose: None,
            abilities: None,
            loaded_sent: false,
            heard_level: None,
            native_epoch: None,
            level_load: horizon_client::level_loading::LevelLoadTracker::new(0),
            sent_info: info,
            predictions: HashMap::new(),
            ticked_before_player: std::collections::HashSet::new(),
            menu: None,
            craft_result: None,
            menu_title: None,
            state_id: 0,
            use_target: None,
            menu_extra: None,
            entities: HashMap::new(),
            tick_fluids: Vec::new(),
            tick_rain: Vec::new(),
            tick_standings: Vec::new(),
            tick_water_fx: Vec::new(),
            passenger_vehicles: HashMap::new(),
            lerps: HashMap::new(),
            cart_lerps: HashMap::new(),
            stepped: HashMap::new(),
            head_lerps: HashMap::new(),
            ghast_dismount: None,
            first_passenger: false,
            passenger_index: 0,
            pending_items: HashMap::new(),
            health: (20.0, 20, 5.0),
            flash_on_set_health: false,
            game_mode: 0,
            chat: Vec::new(),
            chat_received: 0,
            chat_queue: VecDeque::new(),
            signatures: Default::default(),
            chat_previous: None,
            next_chat_index: 0,
            enforces_secure_chat: false,
            services_keys,
            chat_sessions: HashMap::new(),
            chat_validators: HashMap::new(),
            joining_narrated: None,
            chat_tree,
            chat_tree_received: false,
            secure_chat: Default::default(),
            custom_completions: Default::default(),
            players: HashMap::new(),
            tab_players: HashMap::new(),
            skin_textures: HashMap::new(),
            own_profile: OwnProfile::Idle,
            skins: HashMap::new(),
            skins_revision: 0,
            published_skins: None,
            listed: Default::default(),
            online_mode: false,
            level_players: HashMap::new(),
            level_player_names: HashMap::new(),
            level_player_textures: HashMap::new(),
            glowing: Default::default(),
            own_invisible: false,
            own_on_fire: false,
            own_arrows: 0,
            own_stingers: 0,
            own_shoulders: [None; 2],
            profile: 0,
            profile_name: String::new(),
            destroying: None,
            destroying_item: String::new(),
            is_destroying: false,
            destroy_delay: 0,
            input: Default::default(),
            pending_keys: Vec::new(),
            swap_answer: [false; 2],
            stop_mining: false,
            placing: None,
            flat: false,
            death_message: None,
            show_death_screen: true,
            reduced_debug_info: false,
            do_limited_crafting: false,
            player_type,
            synced: vec![None; CREATIVE_SYNCED],
            decode_errors: 0,
            clock: None,
            clock_rate: 1.0,
            clock_partial: 0.0,
            clock_game_time: 0,
            chunk_center: (0, 0),
            backlog: VecDeque::new(),
            backlog_since: None,
            server_links: Vec::new(),
            debug_query: Default::default(),
            transfer: None,
        });
        // The level's TickRateManager follows the server's ticking packets.
        self.clock = horizon_client::clock::FixedStep::client();
        // The player's advancements and statistics are the server's.
        self.adv = Default::default();
        self.stats = Default::default();
        Ok(())
    }
    /// Prepare a client session and remote state to receive packets from a studio replay.
||||||| base
        self.realm_session = realm;
        self.reset_tutorial();
        self.perf.world_opening();
        // A server's own biomes come over the connection; the last local
        // world's packed table must not name its ids. The window's own
        // server thread keeps the one it just set.
        if local.is_none() {
            self.world_biomes = None;
            self.world_biomes_base = None;
        }
        let root = self.options.resources.clone();
        let builtin = builtin_registries(&root).ok_or("Resources are incomplete: registries.json is missing or invalid. Click \"Prepare again\" in the launcher (or run horizon-prepare), then try again.")?;
        let lang = Arc::new(horizon_client::net::load_language(&root));
        // Options.createInformation: what the server learns about this player,
        // sent again when one of those options changes.
        let info = self
            .settings
            .client_information(client_view_distance(self.settings.render_distance));
        if local.is_none() && !self.options.split_guest {
            horizon_client::crash::set_detail(
                "Server",
                horizon_client::net::shown_address(address),
            );
        }
        // A signed-in account always plays under its own name; a splitscreen player signs in
        // with its own account when it picked one.
        let auth = self.auth();
        let name = auth.as_ref().map_or(name, |a| a.name.as_str());
        let user = auth.as_ref().map_or_else(
            || {
                (
                    name.to_owned(),
                    horizon_protocol::crypto::offline_uuid(name),
                )
            },
            |a| (a.name.clone(), a.uuid),
        );
        self.server_packs_connect(local.is_none().then_some(address), user);
        let session = match local {
            Some(pipe) => Session::connect_local(
                pipe,
                name,
                auth.clone(),
                info.clone(),
                lang.clone(),
                root.clone(),
            ),
            None => {
                self.extras_connecting(address, name, lan);
                match cookies {
                    Some(cookies) => Session::connect_transferred(
                        address,
                        name,
                        auth.clone(),
                        info.clone(),
                        lang.clone(),
                        cookies,
                        root.clone(),
                    ),
                    None => {
                        let connect = if lan {
                            Session::connect_lan
                        } else {
                            Session::connect
                        };
                        connect(
                            address,
                            name,
                            auth.clone(),
                            info.clone(),
                            lang.clone(),
                            root.clone(),
                        )
                    }
                }
            }
        };
        let player_type = super::perspective::player_type(&root);
        let mut streaming = Streaming::new(
            Arc::new(NoTerrain),
            self.worker_context(),
            self.res.threads(),
        );
        streaming.remote = true;
        streaming.radius = self.settings.render_distance;
        self.streaming = streaming;
        self.world = World::new(self.res.facts().air, 0);
        self.save = Arc::new(SaveDir::new(
            std::env::temp_dir().join("horizon-remote-unused"),
            horizon_client::save::LevelShape::OVERWORLD,
        ));
        self.inventory = vec![None; 36];
        self.offhand = None;
        self.armor = [None, None, None, None];
        self.craft_grid = vec![None; 9];
        self.craft_size = 2;
        self.player = None;
        self.spawn = None;
        self.mobs.clear();
        self.clear_remote_objects();
        self.clear_remote_projectiles();
        self.creatures.projectiles.clear();
        self.dropped.clear();
        self.particles.clear();
        self.pickups.clear();
        self.falling.clear();
        self.mining = None;
        self.container = None;
        self.carried = None;
        self.using = None;
        self.end_mod_use();
        self.clicks.miss_time = 0;
        // A new LocalPlayer has no effects: the server sends what it has
        // (ClientboundUpdateMobEffectPacket on placement).
        self.potions = Default::default();
        self.set_mode_id(0);
        self.experience = Experience {
            level: 0,
            progress: 0.0,
            total: 0,
            score: 0,
        };
        // The connected client never opens a server world (single player also
        // joins here), so load the registries its argument parsers and local
        // suggestions need before receiving the server's command tree.
        self.ui.registries = Default::default();
        self.build_registries();
        self.ui.registries.permission = 0;
        self.ui.chat.clear();
        self.screen = Screen::Playing;
        self.title = None;
        self.dim = Dim::Overworld;
        self.clear_lod();
        self.pending_uploads.clear();
        if let Some(r) = self.renderer.as_mut() {
            r.clear_sections(self.world.layout());
        }
        self.apply_settings();
        self.clear_waypoints();
        self.boss_overlay.reset();
        // The chat's tree until the server sends its own, or forever when it
        // sends none: the game's own, plus the commands native mods own.
        let chat_tree = self.chat_tree_for(None);
        // Minecraft.services() has the keys wherever the game is played, single player's own
        // server included: its guests' chat keys are checked against them.
        let services_keys = Arc::new(std::sync::Mutex::new(ServicesKeys::Pending));
        let slot = services_keys.clone();
        let _ = std::thread::Builder::new()
            .name("Services keys".into())
            .spawn(move || {
                use horizon_server_files::profiles;
                let keys = profiles::fetch_profile_key_keys(&profiles::discovery_url());
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = match keys {
                    Ok(keys) if !keys.is_empty() => ServicesKeys::Ready(keys),
                    Ok(_) => ServicesKeys::Missing,
                    Err(why) => {
                        log::error!("Horizon: {why}");
                        ServicesKeys::Missing
                    }
                };
            });
        self.remote = Some(Remote {
            session,
            save_folder: None,
            brand: "unknown".into(),
            recipe_transfer: false,
            features: vec!["minecraft:vanilla".into()],
            quiet_commands: false,
            mod_net: false,
            attachments: HashMap::new(),
            far_terrain: None,
            builtin,
            status: lang
                .get("connect.connecting")
                .cloned()
                .unwrap_or_else(|| "Connecting to the server...".into()),
            lang,
            registries: Arc::new(Dynamic::default()),
            tables: Default::default(),
            joined: false,
            reconfiguring: None,
            logged_in: false,
            entity_id: 0,
            local_entity_silent: false,
            camera: None,
            use_item: None,
            dimension: DimensionType::vanilla("minecraft:overworld"),
            level: "minecraft:overworld".into(),
            hashed_seed: 0,
            chunk_format: Arc::new(ChunkFormat::new(
                &DimensionType::vanilla("minecraft:overworld"),
                Vec::new(),
                7,
            )),
            decoder: ChunkDecoder::new(),
            view_distance: self.settings.render_distance,
            simulation_distance: self.settings.simulation_distance,
            sequence: 0,
            last_teleport_sequence: -1,
            position_sender: Default::default(),
            last_input: 0,
            was_sprinting: false,
            carried: std::cell::Cell::new(None),
            was_shared_flags: 0,
            was_living_flags: 0,
            last_recorded_pose: None,
            abilities: None,
            loaded_sent: false,
            heard_level: None,
            native_epoch: None,
            level_load: horizon_client::level_loading::LevelLoadTracker::new(0),
            sent_info: info,
            predictions: HashMap::new(),
            ticked_before_player: std::collections::HashSet::new(),
            menu: None,
            craft_result: None,
            menu_title: None,
            state_id: 0,
            use_target: None,
            menu_extra: None,
            entities: HashMap::new(),
            tick_fluids: Vec::new(),
            tick_rain: Vec::new(),
            tick_standings: Vec::new(),
            tick_water_fx: Vec::new(),
            passenger_vehicles: HashMap::new(),
            lerps: HashMap::new(),
            cart_lerps: HashMap::new(),
            stepped: HashMap::new(),
            head_lerps: HashMap::new(),
            ghast_dismount: None,
            first_passenger: false,
            passenger_index: 0,
            pending_items: HashMap::new(),
            health: (20.0, 20, 5.0),
            flash_on_set_health: false,
            game_mode: 0,
            chat: Vec::new(),
            chat_received: 0,
            chat_queue: VecDeque::new(),
            signatures: Default::default(),
            chat_previous: None,
            next_chat_index: 0,
            enforces_secure_chat: false,
            services_keys,
            chat_sessions: HashMap::new(),
            chat_validators: HashMap::new(),
            joining_narrated: None,
            chat_tree,
            chat_tree_received: false,
            secure_chat: Default::default(),
            custom_completions: Default::default(),
            players: HashMap::new(),
            tab_players: HashMap::new(),
            skin_textures: HashMap::new(),
            own_profile: OwnProfile::Idle,
            skins: HashMap::new(),
            skins_revision: 0,
            published_skins: None,
            listed: Default::default(),
            online_mode: false,
            level_players: HashMap::new(),
            level_player_names: HashMap::new(),
            level_player_textures: HashMap::new(),
            glowing: Default::default(),
            own_invisible: false,
            own_on_fire: false,
            own_arrows: 0,
            own_stingers: 0,
            own_shoulders: [None; 2],
            profile: 0,
            profile_name: String::new(),
            destroying: None,
            destroying_item: String::new(),
            is_destroying: false,
            destroy_delay: 0,
            input: Default::default(),
            pending_keys: Vec::new(),
            swap_answer: [false; 2],
            stop_mining: false,
            placing: None,
            flat: false,
            death_message: None,
            show_death_screen: true,
            reduced_debug_info: false,
            do_limited_crafting: false,
            player_type,
            synced: vec![None; CREATIVE_SYNCED],
            decode_errors: 0,
            clock: None,
            clock_rate: 1.0,
            clock_partial: 0.0,
            clock_game_time: 0,
            chunk_center: (0, 0),
            backlog: VecDeque::new(),
            backlog_since: None,
            server_links: Vec::new(),
            debug_query: Default::default(),
            transfer: None,
        });
        // The level's TickRateManager follows the server's ticking packets.
        self.clock = horizon_client::clock::FixedStep::client();
        // The player's advancements and statistics are the server's.
        self.adv = Default::default();
        self.stats = Default::default();
        Ok(())
    }
    /// Prepare a client session and remote state to receive packets from a studio replay.
=======
        self.connect_remote_session_body(address, name, local, cookies, lan, realm)
    }
>>>>>>> theirs
    pub(super) fn init_remote_for_replay(&mut self) -> Result<()> {
        self.init_remote_for_replay_body()
    }
<<<<<<< ours

    /// Disconnect and return to the multiplayer screen (DisconnectedScreen).
    pub(super) fn leave_server(&mut self, reason: Option<String>) -> Result<()> {
        if reason.is_none() && self.integrated.as_ref().is_some_and(|s| !s.stopped()) {
            return self.begin_closing_save(false);
        }
        self.finish_perf();
        if reason.is_none() {
            // Leaving on purpose: nothing for auto-reconnect to retry.
            self.extras_left();
        }
        self.clear_waypoints();
        self.hud_disconnected();
        // Vanilla's Hud also ticks on the title and connect screens, so the
        // action bar has run out by the next world; Horizon's ticks in one.
        self.titles.overlay_time = 0;
        // GameRenderer.resetData and narrator.clear (Minecraft.disconnect).
        self.item_activation.reset();
        // Kept mod block entity geometry belongs to the world leaving.
        self.kept_block_entities.get_mut().clear();
        self.clear_narrator();
        // updateLevelInEngines(null): every sound stops and the music manager
        // lets go of its track. Leaving a server comes straight here, not
        // through finish_close_world, so the biome's ambient loop would
        // otherwise play on over the title screen until the next world.
        self.ambience.clear(&mut self.audio);
        self.music.stop_playing(None, &mut self.audio);
        self.audio.stop_all();
        // A world is leaving either way: its icon attempt goes with it.
        self.world_icon = None;
        // Minecraft.disconnect: the client level goes.
        if let Some(old) = self.remote.as_mut().and_then(|r| r.heard_level.take()) {
            self.native_client_level_unload(&old);
        }
        Game::java_device_left();
        if self.remote.is_some() {
            self.native_client_disconnecting();
        }
        if let Some(mut r) = self.remote.take() {
            r.session.disconnect();
        }
        super::lan::forget_sync_data();
        self.gui.atlas.clear_object_skins();
        self.server_packs_disconnect();
        self.code_of_conduct = None;
        self.reset_tutorial();
        self.close_dialogs();
        // Gui.setScreen -> KeyMapping.releaseAll: the title or disconnect
        // screen releases every held key. Its release goes to that screen,
        // so a key kept here would still be down in the next world (#3246).
        self.keys.clear();
        self.left = false;
        self.right = false;
        // The shared world is closing: its invite code ends with it.
        self.friends_lan_closed();
        // Single player's server thread saves the world before the title shows.
        let single_player = match self.integrated.take() {
            Some(server) => {
                server.stop()?;
                true
            }
            None => false,
        };
        // A splitscreen player leaving says nothing of where P1 plays.
        if !self.options.split_guest {
            horizon_client::crash::clear_detail("Server");
        }
        self.clock = Default::default();
        self.adv = Default::default();
        self.stats = Default::default();
        self.score.board = Default::default();
        self.streaming = Streaming::new(Arc::new(NoTerrain), self.worker_context(), 1);
        self.streaming.paused = true;
        self.world = World::new(self.res.facts().air, 0);
        self.player = None;
        // The vehicle went with the level: an id kept here would seat the
        // next login on an entity that is not there.
        self.riding = None;
        // Out of the server's bed: InBedChatScreen goes with the level.
        self.sleeping = None;
        self.sleep_timer = 0;
        self.clear_entities();
        self.score.board = Default::default();
        self.tab_list = Default::default();
        self.border = Default::default();
        self.screen = Screen::Playing;
        self.container = None;
        self.clear_lod();
        self.pending_uploads.clear();
        if let Some(r) = self.renderer.as_mut() {
            r.clear_sections(self.world.layout());
        }
        let saves = std::env::current_dir()?.join("saves");
        let mut title = horizon_client::title::Title::new(saves.clone(), &self.options.resources)
            .with_user(|| self.options.player_name());
        title.last_server = self.settings.file.string("lastServer");
        title.skip_multiplayer_warning = self.settings.file.bool("skipMultiplayerWarning");
        if single_player && reason.is_none() {
            self.title = Some(title);
            self.capture(false);
            return Ok(());
        }
        title.mode = horizon_client::title::Mode::Multiplayer(match reason {
            Some(reason) => horizon_client::multiplayer::Screen::Disconnected {
                reason,
                retry: None,
                lines: std::cell::Cell::new(1),
            },
            None => horizon_client::multiplayer::Screen::list(&saves),
        });
        self.title = Some(title);
        self.capture(false);
        Ok(())
||||||| base

    /// Disconnect and return to the multiplayer screen (DisconnectedScreen).
    pub(super) fn leave_server(&mut self, reason: Option<String>) -> Result<()> {
        if reason.is_none() && self.integrated.as_ref().is_some_and(|s| !s.stopped()) {
            return self.begin_closing_save(false);
        }
        self.finish_perf();
        if reason.is_none() {
            // Leaving on purpose: nothing for auto-reconnect to retry.
            self.extras_left();
        }
        self.clear_waypoints();
        self.hud_disconnected();
        // Vanilla's Hud also ticks on the title and connect screens, so the
        // action bar has run out by the next world; Horizon's ticks in one.
        self.titles.overlay_time = 0;
        // GameRenderer.resetData and narrator.clear (Minecraft.disconnect).
        self.item_activation.reset();
        // Kept mod block entity geometry belongs to the world leaving.
        self.kept_block_entities.get_mut().clear();
        self.clear_narrator();
        // updateLevelInEngines(null): every sound stops and the music manager
        // lets go of its track. Leaving a server comes straight here, not
        // through finish_close_world, so the biome's ambient loop would
        // otherwise play on over the title screen until the next world.
        self.ambience.clear(&mut self.audio);
        self.music.stop_playing(None, &mut self.audio);
        self.audio.stop_all();
        // A world is leaving either way: its icon attempt goes with it.
        self.world_icon = None;
        // Minecraft.disconnect: the client level goes.
        if let Some(old) = self.remote.as_mut().and_then(|r| r.heard_level.take()) {
            self.native_client_level_unload(&old);
        }
        if self.remote.is_some() {
            self.native_client_disconnecting();
        }
        if let Some(mut r) = self.remote.take() {
            r.session.disconnect();
        }
        super::lan::forget_sync_data();
        self.gui.atlas.clear_object_skins();
        self.server_packs_disconnect();
        self.code_of_conduct = None;
        self.reset_tutorial();
        self.close_dialogs();
        // Gui.setScreen -> KeyMapping.releaseAll: the title or disconnect
        // screen releases every held key. Its release goes to that screen,
        // so a key kept here would still be down in the next world (#3246).
        self.keys.clear();
        self.left = false;
        self.right = false;
        // The shared world is closing: its invite code ends with it.
        self.friends_lan_closed();
        // Single player's server thread saves the world before the title shows.
        let single_player = match self.integrated.take() {
            Some(server) => {
                server.stop()?;
                true
            }
            None => false,
        };
        // A splitscreen player leaving says nothing of where P1 plays.
        if !self.options.split_guest {
            horizon_client::crash::clear_detail("Server");
        }
        self.clock = Default::default();
        self.adv = Default::default();
        self.stats = Default::default();
        self.score.board = Default::default();
        self.streaming = Streaming::new(Arc::new(NoTerrain), self.worker_context(), 1);
        self.streaming.paused = true;
        self.world = World::new(self.res.facts().air, 0);
        self.player = None;
        // The vehicle went with the level: an id kept here would seat the
        // next login on an entity that is not there.
        self.riding = None;
        // Out of the server's bed: InBedChatScreen goes with the level.
        self.sleeping = None;
        self.sleep_timer = 0;
        self.clear_entities();
        self.score.board = Default::default();
        self.tab_list = Default::default();
        self.border = Default::default();
        self.screen = Screen::Playing;
        self.container = None;
        self.clear_lod();
        self.pending_uploads.clear();
        if let Some(r) = self.renderer.as_mut() {
            r.clear_sections(self.world.layout());
        }
        let saves = std::env::current_dir()?.join("saves");
        let mut title = horizon_client::title::Title::new(saves.clone(), &self.options.resources)
            .with_user(|| self.options.player_name());
        title.last_server = self.settings.file.string("lastServer");
        title.skip_multiplayer_warning = self.settings.file.bool("skipMultiplayerWarning");
        if single_player && reason.is_none() {
            self.title = Some(title);
            self.capture(false);
            return Ok(());
        }
        title.mode = horizon_client::title::Mode::Multiplayer(match reason {
            Some(reason) => horizon_client::multiplayer::Screen::Disconnected {
                reason,
                retry: None,
                lines: std::cell::Cell::new(1),
            },
            None => horizon_client::multiplayer::Screen::list(&saves),
        });
        self.title = Some(title);
        self.capture(false);
        Ok(())
=======
    pub(super) fn leave_server(&mut self, reason: Option<Text>) -> Result<()> {
        self.leave_server_body(reason)
>>>>>>> theirs
    }

    // wire.rs
    fn hud_disconnected(&mut self) {
        self.hud_disconnected_body()
    }
    pub(super) fn send(&self, packet: S) {
        self.send_body(packet)
    }
    pub(super) fn send_play(&self, packet: S) -> bool {
        self.send_play_body(packet)
    }
    fn send_action(&self, action: i32) {
        self.send_action_body(action)
    }
    fn remote_shift_down(&self) -> bool {
        self.remote_shift_down_body()
    }
    fn next_sequence(&mut self) -> i32 {
        self.next_sequence_body()
    }
    fn item_name(&self, id: i32) -> Arc<str> {
        self.item_name_body(id)
    }
    pub(super) fn remote_effect_network_id(&self, effect: &str) -> Option<i32> {
        self.remote_effect_network_id_body(effect)
    }
    pub(super) fn remote_effect_id(&self, effect: i32) -> Option<&'static str> {
        self.remote_effect_id_body(effect)
    }
    fn remote_effect_def(
        &self,
        effect: i32,
    ) -> Option<&'static horizon_client::brewing::EffectDef> {
        self.remote_effect_def_body(effect)
    }
    fn remote_effect(&mut self, p: &play::UpdateMobEffect) {
        self.remote_effect_body(p)
    }
    fn item_id(&self, name: &str) -> i32 {
        self.item_id_body(name)
    }
    pub(super) fn to_stack(&self, s: &Option<Stack>) -> Option<ItemStack> {
        self.to_stack_body(s)
    }
    fn to_network(&self, s: &Option<ItemStack>) -> Option<Stack> {
        self.to_network_body(s)
    }
    fn hashed_stack(&self, s: &Option<ItemStack>) -> Option<Hashed> {
        self.hashed_stack_body(s)
    }

    // chat.rs
    pub(super) fn add_chat_line(
        &mut self,
        content: horizon_client::text::Text,
        source: horizon_client::chat::ChatSource,
    ) -> bool {
        self.add_chat_line_body(content, source)
    }
    fn signed_chat_line(
        &mut self,
        content: horizon_client::text::Text,
        source: horizon_client::chat::ChatSource,
        signature: Option<Signature>,
        trust: ChatTrust,
    ) -> bool {
        self.signed_chat_line_body(content, source, signature, trust)
    }
    fn handle_player_chat(&mut self, chat: QueuedChat) {
        self.handle_player_chat_body(chat)
    }
    fn handle_chat_message_error(
        &mut self,
        chat_type: &play::ChatTypeBound,
        signature: Option<&Signature>,
    ) {
        self.handle_chat_message_error_body(chat_type, signature)
    }
    fn chat_trust(
        &self,
        sender: u128,
        signed: bool,
        timestamp: i64,
        content: &str,
        unsigned: Option<&Tag>,
        decorated: &horizon_client::text::Text,
    ) -> ChatTrust {
        self.chat_trust_body(sender, signed, timestamp, content, unsigned, decorated)
    }
    fn handle_delete_chat(&mut self, packed: &(i32, Option<Vec<u8>>)) -> Result<()> {
        self.handle_delete_chat_body(packed)
    }
    fn accept_chat(&mut self, chat: QueuedChat) -> bool {
        self.accept_chat_body(chat)
    }
    fn player_chat_line(&mut self, chat: QueuedChat) -> bool {
        self.player_chat_line_body(chat)
    }

    // polling.rs
    pub(super) fn remote_carts(&self) -> Option<Vec<serde_json::Value>> {
        self.remote_carts_body()
    }
    pub(super) fn auth(&self) -> Option<std::sync::Arc<horizon_client::auth::AuthSession>> {
        self.auth_body()
    }
    pub(super) fn online_server(&self) -> bool {
        self.online_server_body()
    }
    pub(super) fn remote_custom_completions(&self) -> std::collections::BTreeSet<String> {
        self.remote_custom_completions_body()
    }
    pub(super) fn remote_online_names(&self) -> Option<Vec<String>> {
        self.remote_online_names_body()
    }
    pub(super) fn remote_menu_id(&self) -> i32 {
        self.remote_menu_id_body()
    }
    pub(super) fn remote_chat(&self, n: usize) -> Option<Vec<String>> {
        self.remote_chat_body(n)
    }
    pub(super) fn remote_backlog(&self) -> usize {
        self.remote_backlog_body()
    }
    pub(super) fn poll_stationary_client(&mut self) -> Result<()> {
        self.poll_stationary_client_body()
    }
    pub(super) fn poll_remote(&mut self) -> Result<()> {
        self.poll_remote_body()
    }
    fn send_suggestion_request(&mut self) {
        self.send_suggestion_request_body()
    }
    fn handle_command_suggestions(&mut self, data: &[u8]) {
        self.handle_command_suggestions_body(data)
    }

    // level_state.rs
    fn set_registries(&mut self, registries: Arc<Dynamic>) {
        self.set_registries_body(registries)
    }
    pub(in crate::game) fn enter_remote_dimension(
        &mut self,
        spawn: &play::SpawnInfo,
        keep_world: bool,
        reason: horizon_client::level_loading::Reason,
    ) {
<<<<<<< ours
        // The replacement player starts with its inventory menu, even if the
        // server switched levels without closing the previous container.
        self.remote_close_local();
        self.screen = Screen::Playing;
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        // A Horizon server's bulk edit sends whole columns with the light
        // from before its relight; any other server's packet light is final.
        let relight_resent = r.quiet_commands;
        let dimension = r.registries.dimension(spawn.dimension_type);
        // The server's dimension type gives the level's shape, and its
        // chunks the shape's column.
        let shape = horizon_client::chunk::LevelShape::new(
            dimension.min_y,
            dimension.height,
            dimension.logical_height,
            dimension.has_skylight,
            dimension.has_ceiling,
        )
        .unwrap_or_else(|_| super::Dim::from_id(&dimension.name).shape());
        let level_dim = client_level(&spawn.dimension, &dimension, shape);
        // handleRespawn keeps the ClientLevel only for the same level key:
        // two levels sharing a dimension type are still two levels.
        let same = keep_world && r.heard_level.as_deref() == Some(spawn.dimension.as_str());
        r.chunk_format = Arc::new(
            r.chunk_format
                .with_dimension(&dimension)
                .in_column(shape.column()),
        );
        r.dimension = dimension;
        r.level = spawn.dimension.clone();
        r.hashed_seed = spawn.seed;
        r.game_mode = spawn.game_type;
        r.flat = spawn.flat;
        r.loaded_sent = false;
        // A replacement LevelLoadingScreen starts its bar from nothing (the
        // first one carries on from the server's start).
        if self.player.is_some() {
            self.loading_progress = 0.0;
        }
        // startWaitingForNewLevel: `LevelLoadTracker.startClientLoad` and the
        // `LevelLoadingScreen`.
        r.level_load.start_client_load(client_millis(), reason);
        // A replacement LocalPlayer has a fresh sendPosition baseline.
        r.position_sender = Default::default();
        r.predictions.clear();
        self.set_mode_id(spawn.game_type);
        self.dim = level_dim;
        self.audio.set_world(&spawn.dimension);
        // ClientPacketListener.handleLogin / handleRespawn: the server's
        // Player.getLastDeathLocation, for the recovery compass.
        self.level.last_death = spawn
            .last_death
            .as_ref()
            .map(|d| (d.dimension.clone(), d.pos));
        if !same {
            // A new ClientLevel: Minecraft.setLevel stops every sound.
            self.stop_sounds_for_new_level();
            let mut streaming = Streaming::new(
                Arc::new(NoTerrain),
                self.worker_context(),
                self.res.threads(),
            );
            streaming.remote = true;
            streaming.radius = self.streaming.radius;
            self.streaming = streaming;
            let (time, day, weather) = (
                self.world.game_time,
                self.world.day_time,
                self.world.weather,
            );
            let old = std::mem::take(&mut self.world.chunks);
            let count = old.len();
            horizon_client::chunk::free_columns(old, count);
            self.world = World::new(self.res.facts().air, spawn.seed);
            self.world.shape = shape;
            self.world.game_time = time;
            self.world.day_time = day;
            self.world.weather = weather;
            self.mobs.clear();
            self.clear_remote_objects();
            self.clear_remote_projectiles();
            self.dropped.clear();
            self.particles.clear();
            self.pickups.clear();
            if let Some(r) = self.remote.as_mut() {
                r.entities.clear();
                r.passenger_vehicles.clear();
                r.lerps.clear();
                r.cart_lerps.clear();
                r.stepped.clear();
                r.head_lerps.clear();
                r.pending_items.clear();
                // The entity ids are the old world's; the new dimension's
                // player entities re-map to their profiles on AddEntity.
                r.level_players.clear();
                r.level_player_names.clear();
                r.level_player_textures.clear();
                // A new ClientLevel: a new chunk storage, centred on 0, 0.
                r.chunk_center = (0, 0);
            }
            self.clear_lod();
            self.pending_uploads.clear();
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.clear_sections(self.world.layout());
            }
            // The new ClientLevel is told before setLevel drops the old one.
            let old = self.remote.as_mut().and_then(|r| {
                r.native_epoch = Some(super::native_lifecycle::fresh_epoch());
                r.heard_level.replace(spawn.dimension.clone())
            });
            self.native_client_level_load(&spawn.dimension);
            if let Some(old) = old {
                self.native_client_level_unload(&old);
            }
        }
        self.streaming.relight_resent = relight_resent;
        let has_sky = self
            .remote
            .as_ref()
            .is_none_or(|r| r.dimension.has_skylight);
        self.streaming.sky_light = has_sky;
        self.streaming.nether_shading = self.nether_lighting();
        // The seed in SpawnInfo is already the hashed one the client's BiomeManager holds.
        self.streaming.biome_zoom = horizon_core::biome::BiomeZoom::new(spawn.seed);
        let player = self.player.as_ref();
        self.particles
            .detach_tracking_target(TrackingTarget::LocalPlayer, || {
                player.map(player_tracking_body)
            });
        self.drop_remote_player();
        // handleLogin / handleRespawn: LocalPlayer.setPortalCooldown, so a
        // player who arrives standing in a portal gets no overlay from it.
        self.portals.player.cooldown = spawn.portal_cooldown;
        self.apply_settings();
||||||| base
        // The replacement player starts with its inventory menu, even if the
        // server switched levels without closing the previous container.
        self.remote_close_local();
        self.screen = Screen::Playing;
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        // A Horizon server's bulk edit sends whole columns with the light
        // from before its relight; any other server's packet light is final.
        let relight_resent = r.quiet_commands;
        let dimension = r.registries.dimension(spawn.dimension_type);
        // The server's dimension type gives the level's shape, and its
        // chunks the shape's column.
        let shape = horizon_client::chunk::LevelShape::new(
            dimension.min_y,
            dimension.height,
            dimension.logical_height,
            dimension.has_skylight,
            dimension.has_ceiling,
        )
        .unwrap_or_else(|_| super::Dim::from_id(&dimension.name).shape());
        // handleRespawn keeps the ClientLevel only for the same level key:
        // two levels sharing a dimension type are still two levels.
        let same = keep_world && r.heard_level.as_deref() == Some(spawn.dimension.as_str());
        r.chunk_format = Arc::new(
            r.chunk_format
                .with_dimension(&dimension)
                .in_column(shape.column()),
        );
        r.dimension = dimension;
        r.level = spawn.dimension.clone();
        r.hashed_seed = spawn.seed;
        r.game_mode = spawn.game_type;
        r.flat = spawn.flat;
        r.loaded_sent = false;
        // A replacement LevelLoadingScreen starts its bar from nothing (the
        // first one carries on from the server's start).
        if self.player.is_some() {
            self.loading_progress = 0.0;
        }
        // startWaitingForNewLevel: `LevelLoadTracker.startClientLoad` and the
        // `LevelLoadingScreen`.
        r.level_load.start_client_load(client_millis(), reason);
        // A replacement LocalPlayer has a fresh sendPosition baseline.
        r.position_sender = Default::default();
        r.predictions.clear();
        self.set_mode_id(spawn.game_type);
        self.dim = Dim::from_id(&spawn.dimension);
        self.audio.set_world(&spawn.dimension);
        // ClientPacketListener.handleLogin / handleRespawn: the server's
        // Player.getLastDeathLocation, for the recovery compass.
        self.level.last_death = spawn
            .last_death
            .as_ref()
            .map(|d| (d.dimension.clone(), d.pos));
        if !same {
            // A new ClientLevel: Minecraft.setLevel stops every sound.
            self.stop_sounds_for_new_level();
            let mut streaming = Streaming::new(
                Arc::new(NoTerrain),
                self.worker_context(),
                self.res.threads(),
            );
            streaming.remote = true;
            streaming.radius = self.streaming.radius;
            self.streaming = streaming;
            let (time, day, weather) = (
                self.world.game_time,
                self.world.day_time,
                self.world.weather,
            );
            let old = std::mem::take(&mut self.world.chunks);
            let count = old.len();
            horizon_client::chunk::free_columns(old, count);
            self.world = World::new(self.res.facts().air, spawn.seed);
            self.world.shape = shape;
            self.world.game_time = time;
            self.world.day_time = day;
            self.world.weather = weather;
            self.mobs.clear();
            self.clear_remote_objects();
            self.clear_remote_projectiles();
            self.dropped.clear();
            self.particles.clear();
            self.pickups.clear();
            if let Some(r) = self.remote.as_mut() {
                r.entities.clear();
                r.passenger_vehicles.clear();
                r.lerps.clear();
                r.cart_lerps.clear();
                r.stepped.clear();
                r.head_lerps.clear();
                r.pending_items.clear();
                // The entity ids are the old world's; the new dimension's
                // player entities re-map to their profiles on AddEntity.
                r.level_players.clear();
                r.level_player_names.clear();
                r.level_player_textures.clear();
                // A new ClientLevel: a new chunk storage, centred on 0, 0.
                r.chunk_center = (0, 0);
            }
            self.clear_lod();
            self.pending_uploads.clear();
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.clear_sections(self.world.layout());
            }
            // The new ClientLevel is told before setLevel drops the old one.
            let old = self.remote.as_mut().and_then(|r| {
                r.native_epoch = Some(super::native_lifecycle::fresh_epoch());
                r.heard_level.replace(spawn.dimension.clone())
            });
            self.native_client_level_load(&spawn.dimension);
            if let Some(old) = old {
                self.native_client_level_unload(&old);
            }
        }
        self.streaming.relight_resent = relight_resent;
        let has_sky = self
            .remote
            .as_ref()
            .is_none_or(|r| r.dimension.has_skylight);
        self.streaming.sky_light = has_sky;
        self.streaming.nether_shading = self.nether_lighting();
        // The seed in SpawnInfo is already the hashed one the client's BiomeManager holds.
        self.streaming.biome_zoom = horizon_core::biome::BiomeZoom::new(spawn.seed);
        let player = self.player.as_ref();
        self.particles
            .detach_tracking_target(TrackingTarget::LocalPlayer, || {
                player.map(player_tracking_body)
            });
        self.drop_remote_player();
        // handleLogin / handleRespawn: LocalPlayer.setPortalCooldown, so a
        // player who arrives standing in a portal gets no overlay from it.
        self.portals.player.cooldown = spawn.portal_cooldown;
        self.apply_settings();
=======
        self.enter_remote_dimension_body(spawn, keep_world, reason)
>>>>>>> theirs
    }
    pub(super) fn remote_chunk_in_range(&self, x: i32, z: i32) -> bool {
        self.remote_chunk_in_range_body(x, z)
    }
    fn chunk_inside_remote_range(center: (i32, i32), distance: i32, x: i32, z: i32) -> bool {
        Self::chunk_inside_remote_range_body(center, distance, x, z)
    }
    pub(in crate::game) fn drop_remote_chunks_out_of_range(
        &mut self,
        center: (i32, i32),
        distance: i32,
    ) -> Vec<(i32, i32)> {
        self.drop_remote_chunks_out_of_range_body(center, distance)
    }
    pub(in crate::game) fn forget_all_remote_columns(&mut self) -> Vec<(i32, i32)> {
        self.forget_all_remote_columns_body()
    }
    pub(in crate::game) fn forget_remote_columns(
        &mut self,
        gone: &mut horizon_client::fast_hash::FxHashSet<(i32, i32)>,
        keep_terrain: bool,
    ) -> Vec<(i32, i32)> {
        self.forget_remote_columns_body(gone, keep_terrain)
    }
    fn remote_animate(&mut self, id: i32, action: u8) {
        self.remote_animate_body(id, action)
    }
    fn remote_server_data(&mut self, icon: Option<&[u8]>) {
        self.remote_server_data_body(icon)
    }

    // blocks.rs
    pub(super) fn sync_block_entity(
        &mut self,
        pos: (i32, i32, i32),
        previous: Option<BlockStateId>,
        state: BlockStateId,
    ) {
<<<<<<< ours
        let name = self.res.facts().get(state).name.clone();
        // LevelChunk.setBlockState: another block's block entity goes, even
        // one of the same type (a mod's blocks may share one).
        let other_block = previous.is_some_and(|p| self.res.facts().get(p).name != name);
        self.ensure_stateful_entity_table();
        // State-aware factories decide creation only. A valid same-block
        // instance survives even when the new state would create none.
        if !other_block
            && self.world.block_entities.stateful_entity(state.0)
            && matches!(
                self.world.block_entities.get(&pos),
                Some(BlockEntity::Mod(_))
            )
        {
            return;
        }
        // A mod block's is the mod's own, as on the server.
        match self.new_block_entity_for_state(state) {
            Some(entity) => {
                let same = self.world.block_entities.get(&pos).is_some_and(|e| {
                    e.id() == entity.id() && !(other_block && matches!(e, BlockEntity::Mod(_)))
                });
                if !same {
                    // Attachments go with the block entity they were on. When there was none
                    // here, what is kept for the position is the new one's: the server can sync
                    // a block entity's attachments in the same tick it places the block, and
                    // they may arrive first (a Wired Redstone wire's shape, without which the
                    // crosshair went through it and it could not be mined).
                    if previous.is_some() && self.world.block_entities.contains_key(&pos) {
                        self.forget_block_entity_attachments(pos);
                    }
                    self.world.block_entities.insert(pos, entity);
                }
            }
            None => {
                if previous.is_some() {
                    self.forget_block_entity_attachments(pos);
                }
                self.world.block_entities.remove(&pos);
            }
        }
||||||| base
        let name = self.res.facts().get(state).name.clone();
        // LevelChunk.setBlockState: another block's block entity goes, even
        // one of the same type (a mod's blocks may share one).
        let other_block = previous.is_some_and(|p| self.res.facts().get(p).name != name);
        self.ensure_stateful_entity_table();
        // State-aware factories decide creation only. A valid same-block
        // instance survives even when the new state would create none.
        if !other_block
            && self.world.block_entities.stateful_entity(state.0)
            && matches!(
                self.world.block_entities.get(&pos),
                Some(BlockEntity::Mod(_))
            )
        {
            return;
        }
        // A mod block's is the mod's own, as on the server.
        match self.new_block_entity_for_state(state) {
            Some(entity) => {
                let same = self.world.block_entities.get(&pos).is_some_and(|e| {
                    e.id() == entity.id() && !(other_block && matches!(e, BlockEntity::Mod(_)))
                });
                if !same {
                    if previous.is_some() {
                        self.forget_block_entity_attachments(pos);
                    }
                    self.world.block_entities.insert(pos, entity);
                }
            }
            None => {
                if previous.is_some() {
                    self.forget_block_entity_attachments(pos);
                }
                self.world.block_entities.remove(&pos);
            }
        }
=======
        self.sync_block_entity_body(pos, previous, state)
>>>>>>> theirs
    }
    fn forget_block_entity_attachments(&mut self, (x, y, z): (i32, i32, i32)) {
        self.forget_block_entity_attachments_body((x, y, z))
    }
    fn server_block(&mut self, pos: [i32; 3], state: i32) {
        self.server_block_body(pos, state)
    }
    fn end_predictions(&mut self, sequence: i32) {
        self.end_predictions_body(sequence)
    }
    fn player_position(&self) -> [f64; 3] {
        self.player_position_body()
    }
    fn block_meets_player(&self, pos: [i32; 3], state: i32) -> bool {
        self.block_meets_player_body(pos, state)
    }
    fn snap_player_to(&mut self, at: [f64; 3]) {
        self.snap_player_to_body(at)
    }
    fn set_block(&mut self, pos: [i32; 3], state: i32, predicted: bool) {
        self.set_block_body(pos, state, predicted)
    }

    // entities.rs
    pub(super) fn remote_vehicle_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.remote_vehicle_ids_body()
    }
    pub(super) fn remote_seat_passengers(&mut self, vehicle: u32, passengers: &[i32]) {
        self.remote_seat_passengers_body(vehicle, passengers)
    }
    pub(super) fn client_attachment_entity_exists(&self, holder: Holder) -> bool {
        self.client_attachment_entity_exists_body(holder)
    }
    pub(super) fn spawn_entity(
        &mut self,
        id: i32,
        kind: &str,
        position: [f64; 3],
        rotation: [f32; 3],
    ) {
        self.spawn_entity_body(id, kind, position, rotation)
    }
    fn move_entity(
        &mut self,
        id: i32,
        path: Option<Path>,
        rotation: Option<(f32, f32)>,
        on_ground: bool,
        packet: MovePacket,
    ) {
        self.move_entity_body(id, path, rotation, on_ground, packet)
    }
    pub(super) fn detach_remote_tracking_entity(&mut self, id: i32) {
        self.detach_remote_tracking_entity_body(id)
    }

    // menu_slots.rs
    pub(super) fn inventory_screen_slot(menu: &str, slot: i32) -> Option<usize> {
        Self::inventory_screen_slot_body(menu, slot)
    }
    pub(super) fn menu_slot_index(menu: &str, internal: usize) -> Option<usize> {
        Self::menu_slot_index_body(menu, internal)
    }
    pub(super) fn remote_station_kind(menu: &str) -> Option<super::stations::Kind> {
        Self::remote_station_kind_body(menu)
    }
    fn open_menu_name(&self, container: i32) -> String {
        self.open_menu_name_body(container)
    }
    fn set_menu_slot(&mut self, container: i32, slot: i32, item: Option<ItemStack>) {
        self.set_menu_slot_body(container, slot, item)
    }

<<<<<<< ours
    pub(super) fn handle_packet(&mut self, packet: C) -> Result<()> {
        self.native_client_packet(&packet);
        match packet {
            // ClientCommonPacketListenerImpl.handlePing runs on the game
            // thread, after all preceding packets (especially teleports).
            C::Ping(p) => self.send(S::Pong(p)),
            // A native mod's server half to its client half.
            C::CustomPayload(p) if p.channel == horizon_protocol::mod_net::CHANNEL => {
                self.remote_native_net(&p.data);
            }
            // The synced attachments of a holder the server shows the window.
            C::CustomPayload(p) if p.channel == horizon_protocol::attachment_sync::CHANNEL => {
                self.remote_attachment_sync(&p.data);
            }
            // A mod menu type's block and its mod's bytes, just before the
            // menu's `OpenScreen`.
            C::CustomPayload(p) if p.channel == horizon_protocol::menu_extra::CHANNEL => {
                match horizon_protocol::menu_extra::decode(&p.data) {
                    Ok(extra) => {
                        if let Some(r) = self.remote.as_mut() {
                            r.menu_extra = Some(extra);
                        }
                    }
                    Err(e) => log::debug!("Horizon: a bad menu extra payload: {e}"),
                }
            }
            C::CustomPayload(p) if p.channel == horizon_client::lan::recipes::TRANSFER_CHANNEL => {
                self.remote_recipe_transfer(&p.data);
            }
            // The server's recipes its native mods asked to send.
            C::CustomPayload(p) if p.channel == horizon_client::lan::recipes::CONTENT_CHANNEL => {
                self.remote_recipe_content(&p.data);
            }
            // PingDebugMonitor.onPongReceived.
            C::PongResponse(p) => {
                let delay = self.debug_millis() - p.time;
                self.debug_charts.ping.log(delay);
            }
            C::Login(p) => {
                self.test_highlights.clear();
                self.forget_synced_recipes();
                self.level.hardcore = p.hardcore;
                // handleLogin: startWaitingForNewLevel with Reason.OTHER, which
                // replaces ServerReconfigScreen.
                if let Some(r) = self.remote.as_mut() {
                    r.reconfiguring = None;
                    r.attachments.clear();
                }
                // handleLogin makes a new MultiPlayerGameMode (carried index 0) and a
                // new LocalPlayer (slot 0, no effects, riding nothing: the server
                // pairs a vehicle again with ClientboundSetPassengersPacket).
                self.selected = 0;
                self.potions.effects.clear();
                self.riding = None;
                if let Some(r) = self.remote.as_mut() {
                    r.carried.set(Some(0));
                    // handleLogin makes a new ClientLevel: its predictions count from 0.
                    r.sequence = 0;
                    r.own_invisible = false;
                    r.own_on_fire = false;
                    r.online_mode = p.online_mode;
                    r.enforces_secure_chat = p.enforces_secure_chat;
                    r.entity_id = p.player_id;
                    r.local_entity_silent = false;
                    // handleLogin: the new LocalPlayer is the camera.
                    r.camera = None;
                    r.logged_in = true;
                    r.view_distance = p.chunk_radius;
                    r.simulation_distance = p.simulation_distance;
                    r.show_death_screen = p.show_death_screen;
                    r.reduced_debug_info = p.reduced_debug_info;
                    r.do_limited_crafting = p.limited_crafting;
                }
                self.streaming.radius = self
                    .settings
                    .render_distance
                    .min(p.chunk_radius.saturating_add(1))
                    .max(2);
                // handleLogin: startWaitingForNewLevel(.., Reason.OTHER).
                self.enter_remote_dimension(
                    &p.spawn,
                    false,
                    horizon_client::level_loading::Reason::Other,
                );
                // handleLogin: ClientPlayerNetworkEvent.LoggingIn, the new level made.
                self.native_client_player_network_logging_in();
                // No ClientInformation here: the configuration phase sent it,
                // and ClientPacketListener.broadcastClientInformation sends it
                // again only when an option changes it.
                self.remote_chat_login();
            }
            C::Respawn(p) => {
                if let Some(r) = self.remote.as_ref() {
                    self.audio.remove_sound_entity(r.entity_id as u32);
                }
                // A new player, without effects (the server sends them
                // again) and with no item in use.
                self.potions.effects.clear();
                // The server sends the level, chunks and entities again.
                if let Some(r) = self.remote.as_mut() {
                    r.attachments
                        .retain(|holder, _| matches!(holder, Holder::Player(_)));
                }
                self.using = None;
                self.end_mod_use();
                // After a death (no KEEP_ENTITY_DATA) the new LocalPlayer
                // rides nothing: the server pairs a vehicle again with its
                // passengers. A dimension change keeps the vehicle it travels in.
                if p.keep & 2 == 0 {
                    self.riding = None;
                    // KEEP_ENTITY_DATA copies the old player's synched data
                    // (its shared flags, its score) to the new one; without it they reset.
                    self.experience.score = 0;
                    if let Some(r) = self.remote.as_mut() {
                        r.own_invisible = false;
                        r.own_on_fire = false;
                        r.local_entity_silent = false;
                    }
                }
                let dimension_changed = self.dim.id() != p.spawn.dimension;
                let died = self.screen == Screen::Dead
                    || self
                        .player
                        .as_ref()
                        .is_some_and(|m| m.health.health() <= 0.0);
                let reason = horizon_client::level_loading::reason_for_respawn(
                    died,
                    &p.spawn.dimension,
                    self.dim.id(),
                );
                if dimension_changed {
                    horizon_client::spans::startup::arm("dimension.change");
                }
                // handleRespawn makes a new ClientLevel for another dimension
                // key (a proxy's server switch too): its predictions count from 0.
                if let Some(r) = self.remote.as_mut()
                    && r.level != p.spawn.dimension
                {
                    r.sequence = 0;
                }
                self.portals.respawn_attributes.respawn(p.keep & 1 != 0);
                self.enter_remote_dimension(&p.spawn, true, reason);
                // handleRespawn adds the new LocalPlayer after what the kept
                // level holds; a new level holds nothing yet.
                let before: std::collections::HashSet<u32> =
                    self.mobs.iter().map(|m| m.id).collect();
                if let Some(r) = self.remote.as_mut() {
                    r.ticked_before_player = before;
                }
                if dimension_changed {
                    // LevelExtractor.setLevel: a new level drops the highlights.
                    self.test_highlights.clear();
                    let (music, _) = self.situational_music();
                    self.music.stop_playing(music.as_ref(), &mut self.audio);
                }
                self.screen = Screen::Playing;
                if let Some(r) = self.remote.as_mut() {
                    r.death_message = None;
                    r.health = (20.0, 20, 5.0);
                    // handleRespawn: the new LocalPlayer is the camera.
                    r.camera = None;
                }
                self.capture(true);
            }
            C::PlayerPosition(p) => self.remote_teleport(&p)?,
            C::PlayerRotation(p) => self.remote_rotation(&p),
            C::PlayerLookAt(p) => self.remote_look_at(&p),
            // ClientCommonPacketListenerImpl.handlePostEffects.
            C::PostEffects(p) => self.post_effects = p.effects,
            C::SetCamera(p) => self.remote_set_camera(p.camera_id),
            C::LevelChunkWithLight(p) => self.remote_chunk(&p, None),
            C::LightUpdate(p) => self.remote_light_update(&p),
            C::ChunksBiomes(p) => self.remote_chunks_biomes(&p),
            // `poll_remote` forgets a run of them at once.
            C::ForgetLevelChunk(p) => {
                self.forget_remote_columns(&mut [(p.pos[0], p.pos[1])].into_iter().collect());
            }
            C::ChunkBatchFinished(_) => {
                // ChunkBatchSizeCalculator's ceiling; meshing runs on workers.
                self.send(S::ChunkBatchReceived(play::ChunkBatchReceived {
                    chunks_per_tick: 64.0,
                }));
            }
            C::BlockUpdate(p) => self.server_block(p.pos, p.state),
            C::AddTransientBlock(p) => self.add_transient_block(p.pos, p.state),
            // ClientPacketListener.handleBlockDestruction: another player's crack.
            C::BlockDestruction(p) => {
                self.destroy_block_progress(p.id, p.pos, i32::from(p.progress));
            }
            C::SectionBlocksUpdate(p) => {
                let [sx, sy, sz] = p.section;
                for change in p.changes {
                    let local = (change & 0xFFF) as i32;
                    let pos = [
                        sx * 16 + (local >> 8 & 15),
                        sy * 16 + (local & 15),
                        sz * 16 + (local >> 4 & 15),
                    ];
                    self.server_block(pos, (change >> 12) as i32);
                }
            }
            C::BlockChangedAck(p) => self.end_predictions(p.sequence),
            // Level.blockEvent on the client level: the block's triggerEvent
            // runs on the client's copy of whatever is at pos (a piston's
            // moveBlocks builds the moving block entities it animates, a note
            // block plays); what it doesn't handle reaches the client copy of
            // the block entity (chest lids, shulker boxes).
            C::BlockEvent(p) => {
                let pos = horizon_core::position::BlockPos::new(p.pos[0], p.pos[1], p.pos[2]);
                let triggered = self.with_level(|b, level, ticks| {
                    b.client_block_event(level, ticks, pos, i32::from(p.b0), i32::from(p.b1))
                });
                self.moving_pistons_stale = true;
                self.process_level_events()?;
                if !triggered {
                    let state = self.world.block(p.pos[0], p.pos[1], p.pos[2]);
                    let block = &self.res.facts().get(state).name;
                    let named = self.remote.as_ref().and_then(|r| {
                        r.builtin
                            .get("minecraft:block")?
                            .name(p.block)
                            .map(str::to_owned)
                    });
                    if named.as_deref() == Some(&**block) {
                        self.block_entity_event(p.pos, i32::from(p.b0), i32::from(p.b1));
                    }
                }
            }
            // ClientLevel.levelEvent: LevelEventHandler, as this client's own
            // level plays them (particles and sounds: a spawner's burst, bone
            // meal, waxing, a block broken by someone else).
            // ClientPacketListener.handleLevelEvent: a global one is heard
            // from anywhere (ClientLevel.globalLevelEvent).
            C::LevelEvent(p) if p.global => self.global_level_event_sound(p.kind, p.pos),
            C::LevelEvent(p) => self.level_event_at(p.kind, p.pos, p.data),
            C::LevelParticles(p) => self.remote_level_particles(&p),
            C::Explode(p) => self.remote_explode(&p),
            C::Sound(p) => {
                if let Some(name) = self.remote_sound_name(&p.sound)
                    && !self.pvp_mutes_sound(&name)
                {
                    let at = [
                        f64::from(p.x) / 8.0,
                        f64::from(p.y) / 8.0,
                        f64::from(p.z) / 8.0,
                    ];
                    // ClientPacketListener.handleSoundEvent plays with packet.getSource().
                    match usize::try_from(p.source)
                        .ok()
                        .filter(|s| *s < horizon_client::settings::SOUND_SOURCES.len())
                    {
                        Some(source) => {
                            self.audio
                                .play_on(source, &name, Some(at), p.volume, p.pitch)
                        }
                        None => self.audio.play(&name, Some(at), p.volume, p.pitch),
                    }
                }
            }
            C::SoundEntity(p) => {
                if let Some(name) = self.remote_sound_name(&p.sound)
                    && !self.pvp_mutes_sound(&name)
                    && let Some(state) = self.remote_sound_entity_state(p.id)
                {
                    let source = usize::try_from(p.source)
                        .ok()
                        .filter(|s| *s < horizon_client::settings::SOUND_SOURCES.len())
                        .unwrap_or_else(|| horizon_client::settings::sound_source(&name));
                    let entity = horizon_client::sound_engine::SoundEntity::new(
                        p.id as u32,
                        state.position,
                        state.silent,
                    );
                    entity.update(state);
                    self.audio
                        .play_entity_seeded(&name, source, p.volume, p.pitch, p.seed, entity);
                }
            }
            C::StopSound(p) => self
                .audio
                .stop_sounds(p.sound.as_deref(), p.source.map(|s| s as usize)),
            C::AddEntity(p) => {
                self.audio.remove_sound_entity(p.id as u32);
                let kind = self
                    .remote
                    .as_ref()
                    .and_then(|r| {
                        r.builtin
                            .get("minecraft:entity_type")?
                            .name(p.kind)
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                let rotation =
                    [p.yaw, p.pitch, p.head_yaw].map(horizon_protocol::buf::unpack_degrees);
                if let Some(r) = self.remote.as_mut() {
                    // ClientLevel.addEntity: an id it holds already is removed
                    // and added again, after the player.
                    r.ticked_before_player.remove(&(p.id as u32));
                    // A new entity looks its PlayerInfo up afresh.
                    r.level_player_textures.remove(&p.id);
                    if kind == "minecraft:player" {
                        r.level_players.insert(p.id, p.uuid);
                        // createEntityFromPacket: the player takes the profile of its list entry.
                        match r.players.get(&p.uuid) {
                            Some(name) => r.level_player_names.insert(p.id, name.clone()),
                            None => r.level_player_names.remove(&p.id),
                        };
                    } else {
                        r.level_players.remove(&p.id);
                        r.level_player_names.remove(&p.id);
                    }
                }
                // Entity.getTeam: a mob is on a team by its UUID, a player by
                // its profile name (`Game::scoreboard_name`).
                self.persist.uuids.insert(
                    p.id as u32,
                    horizon_client::lan::playerdata::uuid_ints(p.uuid),
                );
                let replace = self.tracks_remote_entity(p.id);
                self.spawn_entity(p.id, &kind, p.position, rotation);
                if let Some(r) = self.remote.as_mut() {
                    r.cart_lerps.remove(&p.id);
                    r.lerps.remove(&p.id);
                }
                if !self.spawn_remote_object(
                    p.id,
                    &kind,
                    p.position,
                    (rotation[0], rotation[1]),
                    p.data,
                ) {
                    self.spawn_added_projectile(&p, &kind, (rotation[0], rotation[1]), replace);
                }
                // Entity.recreateFromPacket: objects must exist before applying motion.
                self.remote_motion(p.id, p.velocity);
                if self.minecart_improvements()
                    && self.carts.iter().any(|c| c.id == p.id as u32)
                    && let Some(r) = self.remote.as_mut()
                {
                    r.cart_lerps.insert(p.id, CartLerp::new());
                }
                self.move_remote_text_display(p.id, p.position, Some((rotation[0], rotation[1])));
                self.move_remote_interaction(p.id, p.position);
            }
            C::RemoveEntities(p) => {
                for &id in &p.ids {
                    if let Some(r) = self.remote.as_mut() {
                        if let Some(uuid) = r.level_players.remove(&id) {
                            r.attachments.remove(&Holder::Player(uuid));
                        }
                        r.level_player_names.remove(&id);
                        r.level_player_textures.remove(&id);
                        r.attachments.remove(&Holder::Entity(id as u32));
                    }
                }
                self.remove_entities(&p.ids);
            }
            C::MoveEntityPos(play::MoveEntityPos(m))
            | C::MoveEntityPosRot(play::MoveEntityPosRot(m))
            | C::MoveEntityRot(play::MoveEntityRot(m)) => {
                // handleMoveEntity: the delta counts from the position codec's
                // base, which then moves to the delta's end.
                let base = self
                    .remote
                    .as_ref()
                    .and_then(|r| r.entities.get(&m.id))
                    .map(|t| t.codec_base);
                let Some(base) = base else { return Ok(()) };
                // VecDeltaCodec.decode: each step's position from the last.
                let path = m.delta.as_ref().map(|d| match d {
                    play::VecDelta::Linear(_) => Path::Linear(d.end(base)),
                    play::VecDelta::Stepped(steps) => {
                        let mut at = base;
                        Path::Stepped(
                            steps
                                .iter()
                                .map(|(delta, ticks)| {
                                    at = play::VecDelta::apply(at, *delta);
                                    (at, *ticks)
                                })
                                .collect(),
                        )
                    }
                });
                let rotation = m.rotation.map(|(y, x)| {
                    (
                        horizon_protocol::buf::unpack_degrees(y),
                        horizon_protocol::buf::unpack_degrees(x),
                    )
                });
                if let (Some(path), Some(t)) = (
                    &path,
                    self.remote.as_mut().and_then(|r| r.entities.get_mut(&m.id)),
                ) {
                    t.codec_base = path.end();
                }
                self.move_entity(m.id, path, rotation, m.on_ground, MovePacket::Move);
            }
            C::EntityPositionSync(p) => {
                let path = match p.path {
                    play::PositionPath::Linear(at) => Path::Linear(at),
                    play::PositionPath::Stepped(steps) => Path::Stepped(steps),
                };
                // handleEntityPositionSync: the position codec's base is set.
                if let Some(t) = self.remote.as_mut().and_then(|r| r.entities.get_mut(&p.id)) {
                    t.codec_base = path.end();
                }
                self.move_entity(
                    p.id,
                    Some(path),
                    Some((p.yaw, p.pitch)),
                    p.on_ground,
                    MovePacket::PositionSync,
                )
            }
            C::TeleportEntity(p) => {
                let base = self
                    .remote
                    .as_ref()
                    .and_then(|r| r.entities.get(&p.id))
                    .map_or([0.0; 3], |t| t.position);
                let rel = |bit: i32, i: usize| {
                    if p.relatives & bit != 0 {
                        base[i] + p.position[i]
                    } else {
                        p.position[i]
                    }
                };
                let position = [
                    rel(play::relative::X, 0),
                    rel(play::relative::Y, 1),
                    rel(play::relative::Z, 2),
                ];
                // setValuesFromPositionPacket: interpolated, unless too far.
                self.move_entity(
                    p.id,
                    Some(Path::Linear(position)),
                    Some((p.yaw, p.pitch)),
                    p.on_ground,
                    MovePacket::Teleport,
                );
                // handleTeleportEntity: the mount this client moves is put
                // there at once.
                if self.driven_mount().is_some_and(|m| m.id as i32 == p.id) {
                    self.remote_move_vehicle(position, p.yaw);
                }
            }
            C::SetPassengers(p) => self.remote_set_passengers(p.vehicle, &p.passengers),
            // ClientPacketListener.handleEntityLinkPacket: who holds a mob's
            // lead (this player, another player, a mob or a fence knot), for
            // the rope.
            C::SetEntityLink(p) => {
                let me = self.remote.as_ref().map(|r| r.entity_id);
                use horizon_client::mobs::leash::Holder;
                let holder = match p.dest {
                    0 => None,
                    d if Some(d) == me => Some(Holder::Player),
                    d => match self.remote_knots.get(&(d as u32)) {
                        Some(&block) => Some(Holder::Knot(block)),
                        // Not a known knot yet: kept by id until the knot
                        // arrives (game/remote_objects.rs).
                        None if self.boats.iter().any(|b| b.id == d as u32) => {
                            Some(Holder::Boat(d as u32))
                        }
                        None => Some(Holder::Mob(d as u32)),
                    },
                };
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.source as u32) {
                    m.extra.leash = holder;
                } else if let Some(b) = self.boats.iter_mut().find(|b| b.id == p.source as u32) {
                    // A leashed boat or raft (AbstractBoat is Leashable).
                    b.leash = holder;
                }
            }
            C::MoveVehicle(p) => self.remote_move_vehicle(p.position, p.yaw),
            C::MoveMinecartAlongTrack(p) => self.remote_move_minecart(p),
            // ClientPacketListener.handleRotateMob: lerpHeadTo(yHeadRot, 3).
            C::RotateHead(p) => {
                if let Some(r) = self.remote.as_mut() {
                    let yaw = horizon_protocol::buf::unpack_degrees(p.head_yaw);
                    r.head_lerps.insert(p.id, (yaw, 3));
                }
            }
            C::SetEntityData(p) => {
                if let Some(r) = self.remote.as_mut() {
                    for (index, _, value) in &p.values {
                        if let (4, Value::Bool(silent)) = (index, value) {
                            if p.id == r.entity_id {
                                r.local_entity_silent = *silent;
                            }
                            if let Some(t) = r.entities.get_mut(&p.id) {
                                t.silent = *silent;
                            }
                        }
                    }
                }
                // Entity.DATA_SHARED_FLAGS_ID, DATA_CUSTOM_NAME and
                // DATA_CUSTOM_NAME_VISIBLE: server NPCs and holograms rely on
                // these instead of saved NBT.
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            (0, Value::Byte(flags)) => {
                                m.invisible = flags & 0x20 != 0;
                                m.glowing = flags & 0x40 != 0;
                            }
                            (2, Value::OptText(name)) => {
                                let text = name.as_ref().map(horizon_client::entity_nbt::name_text);
                                m.custom_name =
                                    text.as_ref().map(horizon_client::text::Text::plain);
                                m.custom_name_text = text;
                            }
                            (2, Value::Text(name)) => {
                                let text = horizon_client::entity_nbt::name_text(name);
                                m.custom_name = Some(text.plain());
                                m.custom_name_text = Some(text);
                            }
                            (3, Value::Bool(visible)) => m.name_visible = *visible,
                            _ => {}
                        }
                    }
                }
                if let Some((_, _, Value::Byte(flags))) = p.values.iter().find(|v| v.0 == 0)
                    && let Some(r) = self.remote.as_mut()
                {
                    if flags & 0x40 != 0 {
                        r.glowing.insert(p.id);
                    } else {
                        r.glowing.remove(&p.id);
                    }
                }
                self.remote_mob_data(p.id, &p.values);
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                    && m.species() == horizon_client::mobs::Species::Creeper
                {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            // Creeper.DATA_SWELL_DIR, DATA_IS_POWERED, DATA_IS_IGNITED.
                            (16, Value::Int(dir)) => m.extra.counter = *dir,
                            (17, Value::Bool(powered)) => m.extra.charged = *powered,
                            (18, Value::Bool(ignited)) => m.extra.flag = *ignited,
                            _ => {}
                        }
                    }
                }
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            // Entity.DATA_TICKS_FROZEN: isFullyFrozen shakes it.
                            (7, Value::Int(ticks)) => m.ticks_frozen = *ticks,
                            // Strider.DATA_SUFFOCATING (after AgeableMob's two
                            // and DATA_BOOST_TIME): a cold strider shakes.
                            (19, Value::Bool(cold))
                                if m.species() == horizon_client::mobs::Species::Strider =>
                            {
                                m.extra.flag = *cold;
                            }
                            _ => {}
                        }
                    }
                }
                // LivingEntity.DATA_LIVING_ENTITY_FLAGS: for another player
                // too, remote_mob_data's apply reads index 8 above.
                let mannequin = self
                    .mobs
                    .iter()
                    .any(|m| m.id == p.id as u32 && &*m.kind.id == "minecraft:mannequin");
                let other = mannequin
                    || self.remote.as_ref().is_some_and(|r| {
                        r.level_players.contains_key(&p.id) && r.entity_id != p.id
                    });
                // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): another
                // player lies down in their bed (drawn lying in it), or gets up.
                if other {
                    for (index, _, value) in &p.values {
                        if let (14, Value::OptPos(bed)) = (index, value) {
                            let code = bed.map_or(0, |b| self.bed_facing(b));
                            if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                                m.extra.ints[1] = code;
                                if let Some(head) = *bed {
                                    // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): setPosToBed.
                                    let at = Vec3::new(
                                        f64::from(head[0]) + 0.5,
                                        f64::from(head[1]) + 0.5625 + 0.125,
                                        f64::from(head[2]) + 0.5,
                                    );
                                    m.position = at;
                                    m.previous = at;
                                    m.head_pitch = 0.0;
                                    m.previous_head_pitch = 0.0;
                                }
                            }
                        }
                        // Avatar.DATA_PLAYER_MAIN_HAND (HumanoidArm: 0 left, 1
                        // right): the arm a swing of the main hand moves.
                        if let (15, Value::Id(arm)) = (index, value)
                            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                        {
                            m.extra.main_hand_left = *arm == 0;
                        }
                        // Avatar.DATA_PLAYER_MODE_CUSTOMISATION: the skin layers
                        // the player shows (PlayerModelPart bits).
                        if let (16, Value::Byte(bits)) = (index, value)
                            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                        {
                            m.extra.hidden_model_parts = !(*bits as u8) & 0x7F;
                        }
                    }
                }
                // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): this
                // player lies down in the bed, or gets up.
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            (14, Value::OptPos(bed)) => {
                                if bed.is_some() && self.sleeping.is_none() {
                                    self.sleep_timer = 0;
                                }
                                self.sleeping = *bed;
                                if let Some(head) = *bed {
                                    // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): setPosToBed.
                                    let at = Vec3::new(
                                        f64::from(head[0]) + 0.5,
                                        f64::from(head[1]) + 0.5625 + 0.125,
                                        f64::from(head[2]) + 0.5,
                                    );
                                    if let Some(pl) = self.player.as_mut() {
                                        pl.position = at;
                                        pl.velocity = Vec3::ZERO;
                                        pl.pitch = 0.0;
                                    }
                                    self.previous_position = at;
                                    self.pitch = 0.0;
                                    self.snap_player_model_pitch();
                                }
                            }
                            // Entity.DATA_SHARED_FLAGS_ID's sprinting bit: the
                            // server stops the sprint after a hit's extra
                            // knockback (the flag only, as assignValues sets
                            // it: the speed modifier and wasSprinting stay).
                            // Its invisible bit (Entity.isInvisible): the
                            // Invisibility effect's, or a plugin's that hides
                            // the real body under a pose (#3788).
                            (0, Value::Byte(flags)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.sprinting = flags & 0x08 != 0;
                                    // Entity.DATA_SHARED_FLAGS_ID's fall-flying
                                    // bit (0x80, `Entity.setSharedFlag(7, ...)`):
                                    // `LivingEntity.updateFallFlying` clears the
                                    // glide on the server only and `LocalPlayer`
                                    // never ends one itself, so this packet is
                                    // the glide's own end. Applied when it is
                                    // handled — the frame's poll, before its
                                    // ticks — as ClientPacketListener does, so a
                                    // clear that arrives mid-tick lets that tick
                                    // keep its glide travel (#7616).
                                    pl.fall_flying = flags & -128 != 0;
                                }
                                let on_fire = flags & 0x01 != 0;
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_invisible = flags & 0x20 != 0;
                                    let was_on_fire =
                                        std::mem::replace(&mut r.own_on_fire, on_fire);
                                    // The server put the fire out: the
                                    // window's countdown ends with it.
                                    if was_on_fire
                                        && !on_fire
                                        && let Some(pl) = self.player.as_mut()
                                    {
                                        pl.fire_ticks = pl.fire_ticks.min(0);
                                    }
                                }
                            }
                            // Entity.DATA_AIR_SUPPLY_ID and
                            // Player.DATA_PLAYER_ABSORPTION_ID: the bubbles
                            // and golden hearts the server's player has.
                            (1, Value::Int(air)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.air = *air;
                                }
                            }
                            // Entity.DATA_TICKS_FROZEN: the server's count of the
                            // powder snow freeze, which the client's own copy reads
                            // for the frost overlay, the frozen hearts and the
                            // shiver of its third-person model (#11217).
                            (7, Value::Int(ticks)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.ticks_frozen = *ticks;
                                }
                            }
                            // LivingEntity.DATA_ARROW_COUNT_ID and DATA_STINGER_COUNT_ID.
                            (12, Value::Int(count)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_arrows = *count;
                                }
                            }
                            (13, Value::Int(count)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_stingers = *count;
                                }
                            }
                            (19 | 20, Value::Id(v)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_shoulders[usize::from(*index == 20)] =
                                        (*v > 0).then(|| (*v - 1).clamp(0, 4));
                                }
                            }
                            (17, Value::Float(amount)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    let _ = pl.internal_set_absorption(*amount);
                                }
                            }
                            (18, Value::Int(score)) => self.experience.score = *score,
                            // LivingEntity.DATA_LIVING_ENTITY_FLAGS: the server
                            // starts or stops this player's use, as an axe that
                            // disables the raised shield does (Player.disableShield's
                            // stopUsingItem). LocalPlayer.onSyncedDataUpdated.
                            (8, Value::Byte(flags)) => {
                                let selected = self.selected;
                                let using = self.using;
                                self.using = synced_local_use(using, *flags, selected, |slot| {
                                    self.stack_in(slot).is_some()
                                });
                                // onSyncedDataUpdated's useItemRemaining: getUseDuration,
                                // without startUsingItem's event.
                                if using.is_none()
                                    && let Some((slot, _)) = self.using
                                    && let Some(stack) = self.stack_in(slot).cloned()
                                {
                                    self.use_duration = self.use_duration(&stack);
                                    self.use_start = super::using_item::UseStart::of(stack);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Entity.DATA_SHARED_FLAGS_ID's crouching and sprinting bits:
                // another player's or a mannequin's crouch (their model and
                // Entity.isDiscrete's name tag), and the dust a runner raises.
                if (mannequin
                    || self
                        .remote
                        .as_ref()
                        .is_some_and(|r| r.level_players.contains_key(&p.id)))
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    for (index, _, value) in &p.values {
                        if let (0, Value::Byte(flags)) = (index, value) {
                            m.extra.crouching = flags & 2 != 0;
                            m.extra.sprinting = flags & 8 != 0;
                        }
                    }
                }
                for (index, _, value) in &p.values {
                    if *index == ITEM_ENTITY_ITEM
                        && let Value::Item(stack) = value
                    {
                        self.remote_item_data(p.id, stack);
                    }
                    // ExperienceOrb.DATA_VALUE (the icon's size).
                    if *index == 8
                        && let Value::Int(v) = value
                        && let Some(o) = self.orbs.iter_mut().find(|o| o.id == p.id as u32)
                    {
                        o.value = *v;
                    }
                }
                self.remote_object_data(p.id, &p.values);
                self.remote_projectile_data(p.id, &p.values);
                self.remote_dragon_data(p.id, &p.values);
            }
            // ClientPacketListener.handleSetEquipment: what a mob or another
            // player holds and wears (EquipmentSlot ordinals).
            C::SetEquipment(p) => {
                self.remote_object_equipment(p.id, &p.slots);
                let items: Vec<(u8, Option<Arc<str>>, Option<ItemStack>)> = p
                    .slots
                    .iter()
                    .map(|(slot, stack)| {
                        (
                            *slot,
                            stack.as_ref().map(|s| self.item_name(s.item)),
                            self.to_stack(stack),
                        )
                    })
                    .collect();
                // CrossbowItem.isCharged: the main hand's `charged_projectiles`
                // (AvatarRenderer.getArmPose holds a loaded crossbow up).
                let charged = p.slots.iter().find(|(slot, _)| *slot == 0).map(|(_, s)| {
                    s.as_ref().is_some_and(|s| {
                        matches!(
                            s.component(horizon_protocol::item::component::CHARGED_PROJECTILES),
                            Some(horizon_protocol::item::Component::Stacks(list))
                                if !list.is_empty()
                        )
                    })
                });
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    if let Some(charged) = charged
                        && &*m.kind.id == "minecraft:player"
                    {
                        m.extra.charged = charged;
                    }
                    for (slot, item, stack) in items {
                        let e = &mut m.extra;
                        match slot {
                            0 => match stack {
                                // The components the server sent (a loaded
                                // crossbow's `charged_projectiles`, dyed
                                // leather, an enchanted book's id, ...) are
                                // what the item model draws.
                                Some(stack) => e.set_main_hand_stack(Some(stack)),
                                None => e.set_main_hand(item),
                            },
                            1 => e.off_hand = item,
                            2 => {
                                e.feet = item;
                                e.armor[3] = stack;
                            }
                            3 => {
                                e.legs = item;
                                e.armor[2] = stack;
                            }
                            4 => {
                                e.chest = item;
                                e.armor[1] = stack;
                            }
                            5 => {
                                e.head = item;
                                e.armor[0] = stack;
                            }
                            6 => {
                                // A wolf's armor shows its cracks.
                                e.body_damage = stack.as_ref().map_or(0, |s| s.damage as i32);
                                e.body = item;
                            }
                            7 => m.set_saddle(stack),
                            _ => {}
                        }
                    }
                }
            }
            C::TakeItemEntity(p) => {
                // ClientPacketListener.handleTakeItemEntity: the pickup sound at
                // the orb, item or arrow, whoever collects it (the server sends
                // none); an entity this client doesn't track sounds at this player.
                let orb_at = self
                    .orbs
                    .iter()
                    .find(|o| o.id == p.item as u32)
                    .map(|o| o.position);
                let orb = orb_at.is_some();
                let item = self.dropped.iter().find(|d| d.id == p.item as u32);
                let shot = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|s| s.id == p.item as u32);
                let shot = shot.map(|s| s.position).or_else(|| {
                    self.tridents
                        .iter()
                        .find(|t| t.id == p.item as u32)
                        .map(|t| t.position)
                });
                let at = match (item, shot) {
                    (Some(d), _) => Some([d.position.x, d.position.y, d.position.z]),
                    (None, Some(s)) => Some([s.x, s.y, s.z]),
                    (None, None)
                        if self
                            .remote
                            .as_ref()
                            .is_some_and(|r| r.entity_id == p.player) =>
                    {
                        self.eye_position()
                    }
                    (None, None) => None,
                };
                // `ClientPacketListener.random`, the listener's own stream: a
                // pickup pitch is never a draw of the level's.
                if let Some(o) = orb_at {
                    let mut r = horizon_client::random_source::create();
                    let pitch = (r.next_float() - r.next_float()) * 0.35 + 0.9;
                    self.audio.play(
                        "minecraft:entity.experience_orb.pickup",
                        Some([o.x, o.y, o.z]),
                        0.1,
                        pitch,
                    );
                } else if let Some(at) = at {
                    let mut r = horizon_client::random_source::create();
                    let pitch = ((r.next_float() - r.next_float()) * 0.7 + 1.0) * 2.0;
                    self.audio
                        .play("minecraft:entity.item.pickup", Some(at), 0.2, pitch);
                }
                // ItemPickupParticle toward the collector (this player, another one or a mob).
                let collector = if self
                    .remote
                    .as_ref()
                    .is_some_and(|r| r.entity_id == p.player)
                {
                    super::pickups::Collector::Player
                } else {
                    super::pickups::Collector::Mob(p.player as u32)
                };
                if let Some(item) = self.dropped.iter().find(|d| d.id == p.item as u32).cloned() {
                    self.add_pickup(super::pickups::Taken::Item(Box::new(item)), collector);
                } else if let Some(orb) = self.orbs.iter().find(|o| o.id == p.item as u32).cloned()
                {
                    self.add_pickup(super::pickups::Taken::Orb(Box::new(orb)), collector);
                } else if let Some(shot) = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|s| s.id == p.item as u32)
                    .cloned()
                {
                    // An arrow flies to its collector too, then goes.
                    self.add_pickup(super::pickups::Taken::Arrow(Box::new(shot)), collector);
                } else if let Some(i) = self.tridents.iter().position(|t| t.id == p.item as u32) {
                    // So does a trident: a server's client keeps them apart.
                    let trident = self.tridents.remove(i);
                    self.add_pickup(super::pickups::Taken::Trident(Box::new(trident)), collector);
                }
                // An orb stays: the server may still hold merged ones in it.
                if !orb {
                    self.remove_entity(p.item);
                }
            }
            // ClientPacketListener.handleSetEntityMotion: knockback on this player.
            C::SetEntityMotion(p) if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) => {
                if let Some(pl) = self.player.as_mut() {
                    pl.velocity = Vec3::new(p.velocity[0], p.velocity[1], p.velocity[2]);
                }
            }
            // The same packet for another entity: Entity.lerpMotion.
            C::SetEntityMotion(p) => self.remote_motion(p.id, p.velocity),
            // ClientPacketListener.handleProjectilePowerPacket: the power a
            // hurting projectile accelerates by, sent bundled with its motion.
            C::ProjectilePower(p) => {
                super::remote_projectiles::remote_projectile_power(
                    &mut self.creatures.projectiles,
                    p.id,
                    p.power,
                );
            }
            // ClientPacketListener.handleDamageEvent: LivingEntity.handleDamageEvent
            // jolts the walk animation and restarts the hurt flash, and the
            // hurt player's own client plays its hurt sound (`Player.getHurtSound`:
            // the damage type's effects) at `getSoundVolume` and
            // `Player.getVoicePitch`, which is 1.0 for every player. The
            // server's sound packet reaches every other player around, never
            // this one (#4149), and that copy is played at 1.0.
            // This player's own hurt tilt and flash come from the server too
            // (sent to the player itself): its own moves never hurt it.
            C::DamageEvent(p) => {
                let me = self.remote.as_ref().map(|r| r.entity_id);
                let own = me == Some(p.id);
                self.pvp_hurt(p.id, own, p.direct.map(|d| Some(d) == me));
                if own {
                    let (attacker, player) = own_hurt_attacker(p.cause, |id| {
                        self.mob_index
                            .get(&self.mobs, id)
                            .map(|m| &*m.kind.id == "minecraft:player")
                    });
                    self.native_client_hurt(attacker, player);
                    // For `events`: this client has no combat tracker of its own here.
                    let cause = self.remote.as_ref().and_then(|r| {
                        r.registries
                            .name("minecraft:damage_type", p.source_type)
                            .map(str::to_owned)
                    });
                    let by = attacker.and_then(|id| {
                        self.mob_index
                            .get(&self.mobs, id)
                            .map(|m| m.kind.id.to_string())
                    });
                    self.events
                        .note_hit(cause.unwrap_or_else(|| "unknown".into()), by);
                    let at = self
                        .player
                        .as_ref()
                        .map(|pl| [pl.position.x, pl.position.y, pl.position.z]);
                    if let Some(pl) = self.player.as_mut() {
                        pl.health.animate_hurt();
                    }
                    if let Some(at) = at {
                        let sound = self.remote_hurt_sound(p.source_type);
                        let pitch = player_voice_pitch();
                        self.audio.play_on(
                            horizon_client::settings::sound_source(sound),
                            sound,
                            Some(at),
                            1.0,
                            pitch,
                        );
                    }
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.limb_speed = 1.5;
                    m.hurt_time = 10;
                } else {
                    self.remote_dragon_hurt(p.id);
                }
            }
            C::HurtAnimation(p) => {
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    // Player.animateHurt: the tilt turns around the yaw.
                    if let Some(pl) = self.player.as_mut() {
                        pl.health.animate_hurt_dir(p.yaw);
                    }
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.hurt_time = 10;
                } else {
                    self.remote_dragon_hurt(p.id);
                }
            }
            C::EntityEvent(p) => {
                if p.event == super::fireworks::FIREWORK_EXPLODE_EVENT {
                    self.remote_firework_explosion(p.id);
                }
                self.remote_fangs_event(p.id as u32, p.event);
                self.remote_boat_event(p.id, p.event);
                // MinecartTNT.handleEntityEvent 70: prime the fuse the window draws.
                self.remote_tnt_cart_event(p.id, p.event);
                // Arrow.handleEntityEvent 0: a tipped arrow's burst of particles.
                if p.event == 0 {
                    self.remote_arrow_puff(p.id as u32);
                }
                // LivingEntity.handleEntityEvent 2: a spear's hit felt.
                if p.event == 2 {
                    self.remote_kinetic_hit(p.id);
                }
                // LocalPlayer.setPermissions also refreshes an open World Options screen.
                if (24..=28).contains(&p.event)
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                {
                    self.ui.registries.permission = (p.event - 24) as u8;
                    self.remote_world_options_permission(p.event >= 26);
                }
                // Player.handleEntityEvent 22 / 23: reduced debug info on / off.
                if (22..=23).contains(&p.event)
                    && let Some(r) = self.remote.as_mut().filter(|r| r.entity_id == p.id)
                {
                    r.reduced_debug_info = p.event == 22;
                }
                // Ravager.handleEntityEvent 69: the roar's puff and push.
                if p.event == 69 {
                    self.remote_ravager_roar(p.id);
                }
                // ClientPacketListener.handleEntityEvent 63: a sniffer's digging sound.
                if p.event == 63 {
                    self.start_sniffer_digging_sound(p.id as u32);
                }
                // ClientPacketListener.handleEntityEvent 21: a guardian's hum.
                if p.event == 21 {
                    self.start_guardian_sound(p.id as u32);
                }
                // ZombieVillager.handleEntityEvent 16: the cure starts.
                if p.event == 16 {
                    self.zombie_villager_cure_sound(p.id as u32);
                }
                // Witch.handleEntityEvent 15: the magic puffs over its hat.
                if p.event == 15 {
                    self.witch_magic_particles(p.id as u32);
                }
                // Fox.handleEntityEvent 45: the crumbs of the food in its mouth.
                if p.event == 45 {
                    self.fox_eat_particles(p.id as u32);
                }
                // Rabbit.handleEntityEvent(1): the hop clocks, and
                // Entity.spawnSprintParticle (one block-dust crumb).
                if p.event == 1 {
                    let hop = {
                        let m = self.mob_index.get_mut(&mut self.mobs, p.id as u32);
                        if let Some(m) = m
                            && m.species() == horizon_client::mobs::Species::Rabbit
                        {
                            let sample =
                                (m.position, m.velocity, f64::from(m.width()), m.supporting);
                            horizon_client::mobs::rabbit::hop_event(m);
                            Some(sample)
                        } else {
                            None
                        }
                    };
                    if let Some((position, movement, width, support)) = hop {
                        let id = p.id as u32;
                        self.sprint_particle(position, movement, support, width, |game| {
                            // this.random: the rabbit's own, x then z.
                            game.mob_index
                                .get_mut(&mut game.mobs, id)
                                .map(|m| (m.random.next_double(), m.random.next_double()))
                        });
                    }
                }
                if p.event == 19
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    horizon_client::mobs::aquatic::squid_cycle_event(m);
                }
                // LivingEntity.handleEntityEvent 55: another player swapped
                // its main and off hand (`swapHandItems`).
                if p.event == 55
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    std::mem::swap(&mut m.extra.main_hand, &mut m.extra.off_hand);
                }
                // LivingEntity.handleEntityEvent 47 to 52, 65 and 68: an
                // equipped item broke (onEquippedItemBroken).
                if let Some(slot) = horizon_client::mobs::item_break::equipment_break_slot(p.event)
                {
                    self.remote_equipment_break(p.id, slot);
                }
                // EntityEvent.DEATH: start the death animation.
                if p.event == 3
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    m.health = 0.0;
                }
                // LivingEntity.handleEntityEvent 3 on this player: its death
                // sound, which Player.playSound plays for this client alone
                // (the server's goes to everyone else).
                if p.event == 3
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                    && let Some(pl) = self.player.as_ref()
                {
                    let at = [pl.position.x, pl.position.y, pl.position.z];
                    let sound = "minecraft:entity.player.death";
                    let pitch = player_voice_pitch();
                    self.audio.play_on(
                        horizon_client::settings::sound_source(sound),
                        sound,
                        Some(at),
                        1.0,
                        pitch,
                    );
                }
                // The events that start a mob's animation (the warden's).
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    horizon_client::mobs::sync::entity_event(m, p.event as u8);
                }
                // LivingEntity.handleEntityEvent 46: portal particles from the
                // old position to the new one (chorus fruit, an enderman).
                if p.event == 46 {
                    self.entity_teleport_particles(p.id);
                }
                // LivingEntity.handleEntityEvent 60: the poof as the dead body goes.
                if p.event == 60
                    && let Some(m) = self.mob_index.get(&self.mobs, p.id as u32)
                {
                    let (at, w, h) = (m.position, f64::from(m.width()), f64::from(m.height()));
                    self.death_poof([at.x, at.y, at.z], w, h);
                }
                if p.event == super::totem::TOTEM_EVENT {
                    self.remote_totem(p.id);
                }
                // Mob.handleEntityEvent 20: the spawn poof.
                if p.event == 20 && self.mobs.iter().any(|m| m.id == p.id as u32) {
                    self.poof_particles(p.id as u32);
                }
                // The particle bursts of Villager 12 to 14 and 42, TamableAnimal
                // and AbstractHorse 6 and 7, Animal and Allay 18, Ocelot 40 and
                // 41, Dolphin 38: the table in `entity_event_burst` is the one
                // list of them; any other event draws nothing here.
                self.entity_event_particles(p.id as u32, p.event);
                // Snowball and ThrownEgg.handleEntityEvent 3: the crumbs where
                // the projectile is (the server removes it right after).
                if p.event == 3 {
                    let crumbs = self
                        .creatures
                        .projectiles
                        .iter()
                        .find(|pr| pr.id == p.id as u32)
                        .and_then(|pr| {
                            let item = match pr.kind {
                                horizon_client::projectiles::ProjectileKind::Snowball => {
                                    "minecraft:snowball"
                                }
                                horizon_client::projectiles::ProjectileKind::Egg => "minecraft:egg",
                                horizon_client::projectiles::ProjectileKind::Thrown(t) => t.item,
                                _ => return None,
                            };
                            Some(([pr.position.x, pr.position.y, pr.position.z], item))
                        });
                    if let Some((at, item)) = crumbs {
                        self.item_burst(at, item);
                    }
                }
                if p.event == super::combat::DROWN_EVENT {
                    self.remote_drown(p.id);
                }
                // Entity events 53 and 54: honey crumbs while sliding, and the
                // ten from a landing (`HoneyBlock.showSlideParticles` /
                // `showJumpParticles`).
                if horizon_client::landing::honey_particle_count(p.event).is_some() {
                    let at = if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                        self.player
                            .as_ref()
                            .map(|q| [q.position.x, q.position.y, q.position.z])
                    } else {
                        self.mobs
                            .iter()
                            .find(|m| m.id == p.id as u32)
                            .map(|m| [m.position.x, m.position.y, m.position.z])
                    };
                    if let Some(at) = at {
                        self.honey_entity_particles(at, p.event);
                    }
                }
                if p.event == super::consume::USE_ITEM_COMPLETE_EVENT
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                {
                    self.remote_complete_using();
                }
            }
            C::Animate(p) => self.remote_animate(p.id, p.action),
            // ClientPacketListener.handleSwingAnimation: the entity swings
            // (LivingEntity.swing, which a swing in its first half ignores).
            C::SwingAnimation(p) => {
                let hand = if p.hand == 1 { Hand::Off } else { Hand::Main };
                let animation = horizon_client::first_person::SwingAnimation {
                    kind: p.kind.clamp(0, 255) as u8,
                    duration: p.duration,
                };
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    self.swing_hand(hand, animation);
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.swing_hand_modified(hand == Hand::Off, animation);
                }
            }
            // The player's own instances: its maximum health (the hearts the
            // HUD draws and the health it may hold), reach, speed and so on.
            // Then the received health again, which the old maximum clamped.
            C::UpdateAttributes(p) if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) => {
                let nameplate = self.remote_nameplate_values(&p.values);
                self.apply_own_attributes(p.values);
                if let Some(m) = self.player_model.as_mut() {
                    if let Some(value) = nameplate.name_tag {
                        m.set_name_tag_distance(value);
                    }
                    if let Some(value) = nameplate.below_name {
                        m.set_below_name_distance(value);
                    }
                }
                self.apply_remote_health();
            }
            C::UpdateAttributes(p) => self.remote_mob_attributes(p.id, &p.values),
            C::UpdateMobEffect(p) => self.remote_effect(&p),
            C::RemoveMobEffect(p) => {
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                    && let Some(id) = self.remote_effect_id(p.effect)
                {
                    self.potions.effects.list.retain(|e| e.id != id);
                }
            }
            C::ChangeDifficulty(p) => {
                self.ui.difficulty = p.difficulty.clamp(0, 3) as u8;
                self.level.difficulty_locked = p.locked;
                self.remote_world_options_difficulty(self.ui.difficulty, p.locked);
            }
            C::GameRuleValues(p) => self.remote_world_options_rules(p.entries),
            // ClientPacketListener.handleSetHealth: LocalPlayer.hurtTo.
            C::SetHealth(p) => {
                let flash = self.remote.as_mut().is_some_and(|r| {
                    r.health = (p.health, p.food, p.saturation);
                    std::mem::replace(&mut r.flash_on_set_health, true)
                });
                if flash && let Some(pl) = self.player.as_mut() {
                    pl.health.flash_health_change(p.health);
                }
                self.apply_remote_health();
                // The death screen opens (or the respawn is asked for) with the
                // death packet when the rule hides it.
                let shown = self.remote.as_ref().is_none_or(|r| r.show_death_screen);
                if p.health <= 0.0 && shown && self.screen != Screen::Dead && self.player.is_some()
                {
                    self.close_dialogs();
                    self.screen = Screen::Dead;
                    self.ui.death_time = 0;
                    self.ui.death_confirm = None;
                    self.capture(false);
                }
            }
            C::PlayerCombatKill(p) => {
                // The component resolves in the client's current language, as
                // the chat line does, not in the language the session joined
                // with (`Remote.lang` is the vanilla server-language table).
                let message = remote_text(&p.message);
                let me = self.remote.as_ref().is_some_and(|r| r.entity_id == p.id);
                if let Some(r) = self.remote.as_mut() {
                    r.death_message = Some(message);
                }
                // ClientPacketListener.handlePlayerCombatKill: the death
                // screen, whatever health was last sent, or `respawn()` at
                // once when it is hidden.
                if me && self.screen != Screen::Dead && self.player.is_some() {
                    if self.remote.as_ref().is_none_or(|r| r.show_death_screen) {
                        self.close_dialogs();
                        self.screen = Screen::Dead;
                        self.ui.death_time = 0;
                        self.ui.death_confirm = None;
                        self.capture(false);
                    } else {
                        self.respawn_player()?;
                    }
                }
            }
            C::TrackedWaypoint(p) => self.handle_waypoint(&p),
            C::SetExperience(p) => {
                self.experience = Experience {
                    level: p.level,
                    progress: p.progress,
                    total: p.total,
                    score: self.experience.score,
                }
            }
            // ClientPacketListener.handleSetTime: the game time, then
            // ClientClockManager.handleUpdates: the clock catches up with the
            // game time and takes the server's state. Only this level's
            // default world clock is applied — a packet that only updates
            // another clock (the End's) must not overwrite day_time, or the
            // sky follows game_time and looks inverted vs the server's day.
            C::SetTime(p) => {
                self.world.game_time = p.game_time;
                self.remote_clock_tick();
                let clock = {
                    let r = self.remote.as_ref();
                    let names =
                        r.and_then(|r| r.registries.registries.get("minecraft:world_clock"));
                    let tracked = r.and_then(|r| r.clock);
                    let wanted = r
                        .and_then(|r| r.dimension.default_clock.clone())
                        .or_else(|| r.map(|r| r.dimension.name.clone()))
                        .unwrap_or_else(|| "minecraft:overworld".into());
                    let name = |id: i32| {
                        names
                            .and_then(|n| n.get(id as usize))
                            .map(|(n, _)| n.as_str())
                    };
                    p.clocks
                        .iter()
                        .find(|c| name(c.clock) == Some(wanted.as_str()))
                        .or_else(|| tracked.and_then(|id| p.clocks.iter().find(|c| c.clock == id)))
                        .cloned()
                };
                if let Some(c) = clock {
                    self.world.day_time = c.total_ticks;
                    if let Some(r) = self.remote.as_mut() {
                        r.clock = Some(c.clock);
                        r.clock_rate = c.rate;
                        r.clock_partial = c.partial_tick;
                    }
                }
            }
            C::GameEvent(p) => match p.event {
                // NO_RESPAWN_BLOCK_AVAILABLE: ClientPacketListener.handleGameEvent
                // sends the player this system message.
                0 => self.system_message(horizon_client::text::Text::translate(
                    "block.minecraft.spawn.not_valid",
                    &[],
                )),
                // LEVEL_CHUNKS_LOAD_START: the server has begun sending the level.
                13 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.level_load.loading_packets_received();
                    }
                }
                // GUARDIAN_ELDER_EFFECT: the apparition, and the curse sound
                // unless the elder guardian is silent.
                10 => {
                    self.creatures.elder_apparition = super::mob_blocks::APPARITION_TICKS;
                    if p.param == 1.0
                        && let Some(at) = self.player.as_ref().map(|m| m.position)
                    {
                        self.audio.play(
                            "minecraft:entity.elder_guardian.curse",
                            Some([at.x, at.y, at.z]),
                            1.0,
                            1.0,
                        );
                    }
                }
                // WIN_GAME: the poem and credits, then PERFORM_RESPAWN.
                4 if self.screen != Screen::Win => {
                    self.open_win_screen(true, super::win::Finish::RemoteRespawn);
                }
                1 => {
                    self.world.weather.raining = true;
                    self.world.weather.rain_level = (0.0, 0.0);
                }
                2 => {
                    self.world.weather.raining = false;
                    self.world.weather.rain_level = (1.0, 1.0);
                }
                3 => {
                    let mode = p.param as i32;
                    self.set_mode_id(mode);
                    if let Some(r) = self.remote.as_mut() {
                        r.game_mode = mode;
                    }
                }
                // IMMEDIATE_RESPAWN: LocalPlayer.setShowDeathScreen.
                11 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.show_death_screen = p.param == 0.0;
                    }
                }
                // PLAY_ARROW_HIT_SOUND: ClientPacketListener.handleGameEvent
                // plays ARROW_HIT_PLAYER at player eye height.
                6 => {
                    if let Some(at) = self.eye_position() {
                        self.audio
                            .play("minecraft:entity.arrow.hit_player", Some(at), 0.18, 0.45);
                    }
                }
                // RAIN_LEVEL_CHANGE: ClientPacketListener.handleGameEvent.
                7 => {
                    self.world.weather.rain_level = (p.param, p.param);
                    self.world.weather.raining = p.param > 0.0;
                }
                // THUNDER_LEVEL_CHANGE: ClientPacketListener.handleGameEvent.
                8 => {
                    self.world.weather.thunder_level = (p.param, p.param);
                    self.world.weather.thundering = p.param > 0.0;
                }
                // PUFFER_FISH_STING: ClientPacketListener.handleGameEvent
                // plays PUFFER_FISH_STING at player position.
                9 => {
                    if let Some(at) = self.player.as_ref().map(|m| m.position) {
                        self.audio.play(
                            "minecraft:entity.puffer_fish.sting",
                            Some([at.x, at.y, at.z]),
                            1.0,
                            1.0,
                        );
                    }
                }
                // LIMITED_CRAFTING: LocalPlayer.setDoLimitedCrafting.
                12 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.do_limited_crafting = p.param == 1.0;
                    }
                }
                _ => {}
            },
            C::PlayerAbilities(p) => {
                self.creative = p.flags & 8 != 0;
                if let Some(r) = self.remote.as_mut() {
                    r.abilities = Some(p);
                }
                self.apply_remote_abilities();
            }
            // ClientPacketListener.handleSetHeldSlot: a hotbar slot only, and the
            // carried index stays, so the next tick answers with SetCarriedItem.
            C::SetHeldSlot(p) => {
                if (0..9).contains(&p.slot) {
                    self.selected = p.slot as usize;
                }
            }
            C::ContainerSetContent(p) => {
                self.remote_menu_state(p.container, p.state);
                for (slot, item) in p.items.iter().enumerate() {
                    let stack = self.to_stack(item);
                    self.set_menu_slot(p.container, slot as i32, stack);
                }
                self.carried = self.to_stack(&p.carried);
                self.remote_synced();
            }
            C::ContainerSetSlot(p) => {
                self.remote_menu_state(p.container, p.state);
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                // ClientPacketListener.handleContainerSetSlot: a hotbar slot that
                // gains items squeezes for 5 ticks.
                if p.container == 0
                    && (36..=45).contains(&p.slot)
                    && let Some(new) = &stack
                {
                    let (pop, old) = match p.slot {
                        45 => (9, self.offhand.as_ref()),
                        s => (s as usize - 36, self.inventory[s as usize - 36].as_ref()),
                    };
                    if old.is_none_or(|old| old.count < new.count) {
                        self.hotbar_pop[pop] = 5;
                    }
                }
                let before = self.creative_stacks();
                self.set_menu_slot(p.container, i32::from(p.slot), stack);
                self.remote_synced_since(&before);
            }
            C::SetCursorItem(p) => {
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                self.carried = stack;
            }
            C::SetPlayerInventory(p) => {
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                let before = self.creative_stacks();
                let held = match p.slot {
                    0..=35 => self.inventory[p.slot as usize].as_ref(),
                    40 => self.offhand.as_ref(),
                    _ => None,
                };
                let changed = !super::hand::matches_for_swap(held, stack.as_ref());
                if let Ok(slot) = usize::try_from(p.slot) {
                    self.remote_hand_slot_set(slot, changed);
                }
                match p.slot {
                    0..=35 => self.inventory[p.slot as usize] = stack,
                    36..=39 => self.armor[3 - (p.slot - 36) as usize] = stack,
                    // Inventory.SLOT_OFFHAND.
                    40 => self.offhand = stack,
                    _ => {}
                }
                self.remote_synced_since(&before);
            }
            C::ContainerSetData(p) if self.open_menu_name(p.container) == "minecraft:lectern" => {
                self.remote_lectern_page(i32::from(p.value));
            }
            C::ContainerSetData(p) if self.remote_station_open(p.container) => {
                self.remote_station_data(usize::try_from(p.id).unwrap_or(usize::MAX), p.value);
            }
            C::ContainerSetData(p) if self.native_screen_data(p.container, p.id, p.value) => {}
            C::ContainerSetData(p) => {
                let v = i32::from(p.value);
                match self
                    .container
                    .and_then(|key| self.world.block_entities.get_mut(&key))
                {
                    Some(BlockEntity::Furnace(f)) => match p.id {
                        0 => f.lit_remaining = v,
                        1 => f.lit_total = v,
                        2 => f.cooking_timer = v,
                        3 => f.cooking_total = v,
                        _ => {}
                    },
                    // CrafterBlockEntity.containerData.
                    Some(BlockEntity::Crafter(c)) => match usize::try_from(p.id) {
                        Ok(k @ 0..9) => c.disabled[k] = v == 1,
                        Ok(9) => c.triggered = v,
                        _ => {}
                    },
                    // BrewingStandBlockEntity.dataAccess.
                    Some(BlockEntity::Brewing(b)) => match p.id {
                        0 => b.brew_time = v,
                        1 => b.fuel = v,
                        2 => b.total_brew_time = v,
                        3 => b.total_fuel = v,
                        _ => {}
                    },
                    _ => {}
                }
            }
            C::OpenScreen(p) => self.remote_open_screen(p),
            C::CustomPayload(p) if self.remote_java_payload(&p.channel, &p.data) => {}
            C::MountScreenOpen(p) => self.remote_open_mount(p),
            C::RecipeBookAdd(p) => self.remote_recipe_book_add(p),
            C::RecipeBookRemove(p) => self.remote_recipe_book_remove(p),
            C::RecipeBookSettings(p) => self.remote_recipe_book_settings(p),
            C::PlaceGhostRecipe(p) => self.remote_ghost_recipe(p),
            C::MerchantOffers(p) => self.remote_merchant_offers(p),
            C::ContainerClose(_) => {
                // `LocalPlayer.clientSideCloseContainer` sets the screen to
                // null, including dialogs and WaitingForResponseScreen. This
                // differs from clearDialog, which restores the parent screen.
                let dialog = self.dialog_open();
                if dialog {
                    self.close_dialogs();
                    self.ui.chat_screen = None;
                    self.book = None;
                    self.sign_edit = None;
                }
                if !self.remote_lectern_closed() && (self.screen == Screen::Inventory || dialog) {
                    self.remote_close_local();
                    self.screen = Screen::Playing;
                    self.capture(true);
                }
            }
            C::TagQuery(p) => self.answer_tag_query(p.transaction, p.tag),
            C::Unknown(id, data)
                if id == horizon_protocol::ids::play::clientbound::COMMAND_SUGGESTIONS =>
            {
                self.handle_command_suggestions(&data);
            }
            C::CustomChatCompletions(p) => {
                // ClientPacketListener.handleCustomChatCompletions.
                if let Some(r) = self.remote.as_mut() {
                    horizon_client::command::modify_custom_completions(
                        &mut r.custom_completions,
                        horizon_client::command::CustomCompletionAction::from_id(p.action),
                        &p.entries,
                    );
                    self.ui.registries.custom_completions = r.custom_completions.clone();
                }
            }
            C::Commands(p) => {
                let tree = horizon_client::command::network::from_packet(
                    &p,
                    horizon_client::command::vanilla::dispatcher(),
                );
                // The chat's tree follows the server's now; a mod's commands
                // go into it again (`chat_tree_for`).
                let chat_tree = self.chat_tree_for(Some(&tree));
                if let Some(r) = self.remote.as_mut() {
                    r.chat_tree = chat_tree;
                    r.chat_tree_received = true;
                }
            }
            C::Cooldown(p) => {
                // ClientPacketListener.handleItemCooldown: the server starts
                // (or confirms) a group's cooldown; 0 ends it.
                let group: Arc<str> = Arc::from(p.group.as_str());
                if p.duration == 0 {
                    self.cooldowns.remove(&group);
                } else {
                    self.cooldowns.add(group, p.duration);
                }
            }
            // ClientPacketListener.handleGameTestHighlightPos.
            C::GameTestHighlightPos(p) => self.highlight_test_pos(p.absolute, p.relative),
            // ClientPacketListener.handleTestInstanceBlockStatus.
            C::TestInstanceBlockStatus(p) => {
                self.block_editor_status(remote_text(&p.status), p.size);
            }
            C::SystemChat(p) => {
                // The component as sent, translated in the client's own
                // language; mods may change or cancel it. ChatListener.handleOverlay
                // for the action bar.
                let mut text = remote_text(&p.content);
                if !self.chat_received(true, p.overlay, 0, None, &mut text) {
                    return Ok(());
                }
                if p.overlay {
                    self.say_system_queued(&text);
                    self.titles.set_overlay(text, false);
                } else if self.chat_line(text.clone(), horizon_client::chat::ChatSource::Server) {
                    // handleSystemMessage: said once the chat takes it.
                    self.say_system_chat_queued(&text);
                }
            }
            C::PlayerChat(p) => {
                use horizon_client::scoreboard::tag_to_json;
                // SignedMessageBody.Packed.unpack: the last seen it names must
                // be in the cache; then the message joins it
                // (MessageSignatureCache.push).
                let Some(r) = self.remote.as_mut() else {
                    return Ok(());
                };
                // handlePlayerChat: the global index counts every chat packet; one out of
                // order disconnects.
                let expected = r.next_chat_index;
                r.next_chat_index = r.next_chat_index.wrapping_add(1);
                if p.global_index != expected {
                    log::error!(
                        "Missing or out-of-order chat message from server, expected index {expected} but got {}",
                        p.global_index
                    );
                    let reason = horizon_client::text::Text::translate(
                        "multiplayer.disconnect.bad_chat_index",
                        &[],
                    );
                    return self.leave_server(Some(reason.plain()));
                }
                let last_seen: Option<Vec<Signature>> =
                    p.last_seen.iter().map(|s| r.signatures.unpack(s)).collect();
                let Some(last_seen) = last_seen else {
                    log::error!(
                        "Message from player with ID {:032x} referenced unrecognized signature id",
                        p.sender
                    );
                    let reason = horizon_client::text::Text::translate(
                        "multiplayer.disconnect.invalid_packet",
                        &[],
                    );
                    return self.leave_server(Some(reason.plain()));
                };
                let signature = p
                    .signature
                    .as_deref()
                    .and_then(horizon_protocol::secure_chat::signature);
                r.signatures.push(&last_seen, signature.as_ref());
                // The sender's PlayerInfo: none is an error line; else their
                // `SignedMessageValidator` decides (`updateAndValidate`), and a message it
                // takes without a signature of its own shows as unsigned.
                if !r.tab_players.contains_key(&p.sender) {
                    log::error!(
                        "Received player chat packet for unknown player with ID: {:032x}",
                        p.sender
                    );
                    self.handle_chat_message_error(&p.chat_type, signature.as_ref());
                    return Ok(());
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as i64);
                let incoming = secure_chat::Incoming {
                    sender: p.sender,
                    index: p.index,
                    signature: signature.as_ref(),
                    body: secure_chat::Body {
                        content: &p.content,
                        timestamp: p.timestamp,
                        salt: p.salt,
                        last_seen: &last_seen,
                    },
                    unsigned_content: p.unsigned_content.as_ref(),
                    filter: &p.filter,
                };
                let Some(validated) = r.validate_chat(&incoming, now) else {
                    self.handle_chat_message_error(&p.chat_type, signature.as_ref());
                    return Ok(());
                };
                let signature = signature.filter(|_| validated == secure_chat::Validated::Signed);
                // ChatListener.handlePlayerChatMessage: Only Show Secure Chat
                // shows the signed content, never the server's replacement.
                // The narrator says the message's own decorated content
                // (`narrateChatMessage(boundChatType, message.decoratedContent())`).
                let decorated = p
                    .unsigned_content
                    .as_ref()
                    .map_or_else(|| serde_json::Value::from(p.content.as_str()), tag_to_json);
                let content = if self.settings.file.bool("onlyShowSecureChat") {
                    serde_json::Value::from(p.content.as_str())
                } else {
                    decorated.clone()
                };
                let narration = self
                    .decorate_chat(&p.chat_type, &decorated, Decoration::Narration)
                    .plain();
                let line = self.decorate_chat(&p.chat_type, &content, Decoration::Chat);
                let trust = self.chat_trust(
                    p.sender,
                    signature.is_some(),
                    p.timestamp,
                    &p.content,
                    p.unsigned_content.as_ref(),
                    &line,
                );
                // A hidden or fully filtered message counts as processed, not shown.
                let Some((line, narration)) = self.shown_player_chat(&p, line, narration) else {
                    if let Some(signature) = &signature {
                        self.mark_message_processed(signature, false);
                    }
                    return Ok(());
                };
                self.handle_player_chat(QueuedChat {
                    line,
                    signature,
                    trust,
                    narration,
                    sender: p.sender,
                    chat_type: p.chat_type.chat_type,
                });
            }
            C::DisguisedChat(p) => {
                let content = horizon_client::scoreboard::tag_to_json(&p.message);
                let narration = self
                    .decorate_chat(&p.chat_type, &content, Decoration::Narration)
                    .plain();
                let line = self.decorate_chat(&p.chat_type, &content, Decoration::Chat);
                self.handle_player_chat(QueuedChat {
                    line,
                    signature: None,
                    trust: ChatTrust::Secure,
                    narration,
                    sender: 0,
                    chat_type: p.chat_type.chat_type,
                });
            }
            C::DeleteChat(p) => self.handle_delete_chat(&p.signature)?,
            C::PlayerInfoUpdate(p) => {
                if let Some(r) = self.remote.as_mut() {
                    // handlePlayerInfoUpdate: new entries first, then every
                    // action on every known entry.
                    // INITIALIZE_CHAT: the session (or none) each entry carries; the
                    // player's validator is made again when their chat is next read.
                    if p.actions & 2 != 0 {
                        for e in &p.entries {
                            r.chat_sessions.insert(e.uuid, e.chat_session.clone());
                            r.chat_validators.remove(&e.uuid);
                        }
                    }
                    if p.actions & 1 != 0 {
                        for e in &p.entries {
                            if let Some(name) = &e.name {
                                r.tab_players.entry(e.uuid).or_insert_with(|| {
                                    horizon_client::tab_list::TabPlayer::new(e.uuid, name.clone())
                                });
                                if let Some(t) = e.properties.iter().find(|p| p.name == "textures")
                                {
                                    // Without the services' profile property keys a
                                    // signature can't be checked; a signed property
                                    // counts as genuine.
                                    use horizon_client::skins::{self, Signature};
                                    let signature = if t.signature.is_some() {
                                        Signature::Signed
                                    } else {
                                        Signature::Unsigned
                                    };
                                    let textures =
                                        skins::unpack(&t.value, signature, skins::texture_uris());
                                    r.skin_textures.entry(e.uuid).or_insert(textures);
                                }
                            }
                        }
                    }
                    for e in p.entries {
                        if let Some(t) = r.tab_players.get_mut(&e.uuid) {
                            if let Some(mode) = e.game_mode {
                                t.game_mode = mode;
                            }
                            if let Some(latency) = e.latency {
                                t.latency = latency;
                            }
                            if let Some(name) = &e.display_name {
                                t.display_name =
                                    name.as_ref().map(horizon_client::scoreboard::tag_to_json);
                            }
                            if let Some(order) = e.list_order {
                                t.list_order = order;
                            }
                            if let Some(hat) = e.show_hat {
                                t.show_hat = hat;
                            }
                            match e.listed {
                                Some(true) => {
                                    r.listed.insert(e.uuid);
                                }
                                Some(false) => {
                                    r.listed.remove(&e.uuid);
                                }
                                None => {}
                            }
                        }
                        if let Some(name) = e.name {
                            r.players.insert(e.uuid, name);
                        }
                    }
                }
            }
            C::PlayerInfoRemove(p) => {
                if let Some(r) = self.remote.as_mut() {
                    for u in p.uuids {
                        r.players.remove(&u);
                        r.tab_players.remove(&u);
                        r.chat_sessions.remove(&u);
                        r.chat_validators.remove(&u);
                        r.skin_textures.remove(&u);
                        r.listed.remove(&u);
                        r.attachments.remove(&Holder::Player(u));
                    }
                }
            }
            C::TabList(p) => {
                // handleTabListCustomisation: empty text clears it.
                let text = |t: &Tag| {
                    let json = horizon_client::scoreboard::tag_to_json(t);
                    let text = horizon_client::scoreboard::comp_text(&json);
                    text.spans
                        .iter()
                        .any(|span| !span.text.is_empty())
                        .then_some(text)
                };
                self.tab_list.header = text(&p.header);
                self.tab_list.footer = text(&p.footer);
            }
            C::InitializeBorder(p) => {
                let b = &mut self.border;
                b.set_center([p.center_x, p.center_z]);
                if p.lerp_time > 0 {
                    b.lerp_between(p.old_size, p.new_size, p.lerp_time);
                } else {
                    b.set_size(p.new_size);
                }
                b.set_absolute_max_size(p.absolute_max_size);
                b.warning_distance = p.warning_blocks;
                b.warning_time = p.warning_time;
            }
            C::SetBorderCenter(p) => self.border.set_center([p.x, p.z]),
            C::SetBorderLerpSize(p) => {
                self.border
                    .lerp_between(p.old_size, p.new_size, p.lerp_time)
            }
            C::SetBorderSize(p) => self.border.set_size(p.size),
            C::SetBorderWarningDelay(p) => self.border.warning_time = p.delay,
            C::SetBorderWarningDistance(p) => self.border.warning_distance = p.blocks,
            C::SetObjective(p) => self.remote_objective(p),
            C::SetDisplayObjective(p) => {
                // DisplaySlot ids: list, sidebar, below_name, then the team sidebars.
                let slot = match p.slot {
                    0 => "list".to_owned(),
                    2 => "below_name".to_owned(),
                    n @ 3..=18 => format!(
                        "sidebar.team.{}",
                        horizon_client::text::COLORS[(n - 3) as usize].1
                    ),
                    _ => "sidebar".to_owned(),
                };
                let board = &mut self.score.board;
                let objective = board.objective(&p.objective).map(|o| o.name.clone());
                board.set_display_objective(&slot, objective.as_deref());
            }
            C::SetScore(p) => {
                let board = &mut self.score.board;
                if board.objective(&p.objective).is_some() {
                    let s = board.score_mut(&p.owner, &p.objective);
                    s.value = p.score;
                    s.display = p
                        .display
                        .as_ref()
                        .map(horizon_client::scoreboard::tag_to_json);
                    s.number_format = p.number_format.as_ref().map(number_format);
                }
            }
            C::ResetScore(p) => match &p.objective {
                Some(o) => self.score.board.reset_one(&p.owner, o),
                None => self.score.board.reset_all(&p.owner),
            },
            C::SetPlayerTeam(p) => self.remote_team(p),
            C::SetSimulationDistance(p) => {
                if let Some(r) = self.remote.as_mut() {
                    r.simulation_distance = p.distance;
                }
            }
            C::SetChunkCacheRadius(p) => {
                self.streaming.radius = self
                    .settings
                    .render_distance
                    .min(p.distance.saturating_add(1))
                    .max(2);
                if let Some(r) = self.remote.as_mut() {
                    r.view_distance = p.distance;
                }
                // ClientChunkCache.updateViewRadius: a new storage of that size.
                self.drop_remote_chunks_out_of_range();
            }
            C::SetChunkCacheCenter(p) => {
                if let Some(r) = self.remote.as_mut() {
                    r.chunk_center = (p.x, p.z);
                }
                self.drop_remote_chunks_out_of_range();
            }
            C::BlockEntityData(p) => self.remote_block_entity_data(p.pos, p.kind, &p.tag),
            C::OpenSignEditor(p) => self.remote_open_sign_editor(p),
            C::OpenBook(p) => self.remote_open_book(p.hand),
            // handleSetSpawn: ClientLevel.setRespawnData, which spawn compasses point at.
            C::SetDefaultSpawnPosition(p) => {
                let [x, y, z] = p.pos.pos;
                self.spawn = (p.pos.dimension == "minecraft:overworld")
                    .then(|| Vec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));
                // 1.20.2 handleSetSpawn notified ReceivingLevelScreen here; the later
                // LEVEL_CHUNKS_LOAD_START game event did not exist yet.
                if let Some(r) = self.remote.as_mut()
                    && r.session
                        .server_version()
                        .at_most(horizon_protocol::adapter::ServerVersion::V1_20_2)
                {
                    r.level_load.loading_packets_received();
                }
            }
            C::ServerData(p) => {
                if let Some(r) = self.remote.as_mut()
                    && let Some(enforced) = r.session.legacy_secure_chat_enforcement()
                    && r.enforces_secure_chat != enforced
                {
                    r.enforces_secure_chat = enforced;
                    r.chat_validators.clear();
                }
                self.remote_server_data(p.icon.as_deref());
            }
            C::MapItemData(p) => self.remote_map_item_data(&p),
            C::UpdateTags(p) => self.remote_update_tags(&p),
            // handleTickingState / handleTickingStep: the level's TickRateManager.
            C::TickingState(p) => {
                self.clock.set_rate(p.rate);
                self.clock.set_frozen(p.frozen);
            }
            C::TickingStep(p) => self.clock.set_frozen_ticks_to_run(p.steps),
            C::UpdateAdvancements(p) => {
                let icons = p
                    .added
                    .iter()
                    .map(|a| {
                        a.display
                            .as_ref()
                            .map_or_else(String::new, |d| self.item_name(d.icon.item).to_string())
                    })
                    .collect();
                self.remote_advancements(p, icons);
            }
            // handleAwardStats: StatsCounter.setValue for each.
            C::AwardStats(p) => {
                let Some(r) = self.remote.as_ref() else {
                    return Ok(());
                };
                let mut known = Vec::new();
                for (ty, value, count) in p.stats {
                    let Some(ty) = r
                        .builtin
                        .get("minecraft:stat_type")
                        .and_then(|t| t.name(ty))
                    else {
                        continue;
                    };
                    let values = match ty {
                        "minecraft:mined" => "minecraft:block",
                        "minecraft:killed" | "minecraft:killed_by" => "minecraft:entity_type",
                        "minecraft:custom" => "minecraft:custom_stat",
                        _ => "minecraft:item",
                    };
                    if let Some(key) = r.builtin.get(values).and_then(|v| v.name(value)) {
                        known.push((ty.to_owned(), key.to_owned(), count));
                    }
                }
                for (ty, key, count) in known {
                    self.stats.values.entry(ty).or_default().insert(key, count);
                }
            }
            // ClientPacketListener.setActionBarText, setTitleText,
            // setSubtitleText, setTitlesAnimation and handleTitlesClear.
            C::BossEvent(p) => self.apply_boss_event(&p),
            C::SetActionBarText(p) => self.titles.set_overlay(remote_text(&p.text), false),
            C::SetTitleText(p) => self.titles.set_title(remote_text(&p.text)),
            C::SetSubtitleText(p) => self.titles.set_subtitle(remote_text(&p.text)),
            C::SetTitlesAnimation(p) => self.titles.set_times(p.fade_in, p.stay, p.fade_out),
            // ClientCommonPacketListenerImpl.handleShowDialog / handleClearDialog.
            C::ShowDialog(p) => self.show_play_dialog(&p.dialog),
            C::ClearDialog(_) => self.clear_dialog(),
            // ClientPacketListener.handleLowDiskSpaceWarning.
            C::LowDiskSpaceWarning(_) => horizon_client::chunk_io::report_low_disk_space(),
            C::ClearTitles(p) => {
                self.titles.clear();
                if p.value {
                    self.titles.reset_times();
                }
            }
            _ => {}
        }
        Ok(())
||||||| base
    pub(super) fn handle_packet(&mut self, packet: C) -> Result<()> {
        self.native_client_packet(&packet);
        match packet {
            // ClientCommonPacketListenerImpl.handlePing runs on the game
            // thread, after all preceding packets (especially teleports).
            C::Ping(p) => self.send(S::Pong(p)),
            // A native mod's server half to its client half.
            C::CustomPayload(p) if p.channel == horizon_protocol::mod_net::CHANNEL => {
                self.remote_native_net(&p.data);
            }
            // The synced attachments of a holder the server shows the window.
            C::CustomPayload(p) if p.channel == horizon_protocol::attachment_sync::CHANNEL => {
                self.remote_attachment_sync(&p.data);
            }
            // A mod menu type's block and its mod's bytes, just before the
            // menu's `OpenScreen`.
            C::CustomPayload(p) if p.channel == horizon_protocol::menu_extra::CHANNEL => {
                match horizon_protocol::menu_extra::decode(&p.data) {
                    Ok(extra) => {
                        if let Some(r) = self.remote.as_mut() {
                            r.menu_extra = Some(extra);
                        }
                    }
                    Err(e) => log::debug!("Horizon: a bad menu extra payload: {e}"),
                }
            }
            C::CustomPayload(p) if p.channel == horizon_client::lan::recipes::TRANSFER_CHANNEL => {
                self.remote_recipe_transfer(&p.data);
            }
            // The server's recipes its native mods asked to send.
            C::CustomPayload(p) if p.channel == horizon_client::lan::recipes::CONTENT_CHANNEL => {
                self.remote_recipe_content(&p.data);
            }
            // PingDebugMonitor.onPongReceived.
            C::PongResponse(p) => {
                let delay = self.debug_millis() - p.time;
                self.debug_charts.ping.log(delay);
            }
            C::Login(p) => {
                self.test_highlights.clear();
                self.forget_synced_recipes();
                self.level.hardcore = p.hardcore;
                // handleLogin: startWaitingForNewLevel with Reason.OTHER, which
                // replaces ServerReconfigScreen.
                if let Some(r) = self.remote.as_mut() {
                    r.reconfiguring = None;
                    r.attachments.clear();
                }
                // handleLogin makes a new MultiPlayerGameMode (carried index 0) and a
                // new LocalPlayer (slot 0, no effects, riding nothing: the server
                // pairs a vehicle again with ClientboundSetPassengersPacket).
                self.selected = 0;
                self.potions.effects.clear();
                self.riding = None;
                if let Some(r) = self.remote.as_mut() {
                    r.carried.set(Some(0));
                    // handleLogin makes a new ClientLevel: its predictions count from 0.
                    r.sequence = 0;
                    r.own_invisible = false;
                    r.own_on_fire = false;
                    r.online_mode = p.online_mode;
                    r.enforces_secure_chat = p.enforces_secure_chat;
                    r.entity_id = p.player_id;
                    r.local_entity_silent = false;
                    // handleLogin: the new LocalPlayer is the camera.
                    r.camera = None;
                    r.logged_in = true;
                    r.view_distance = p.chunk_radius;
                    r.simulation_distance = p.simulation_distance;
                    r.show_death_screen = p.show_death_screen;
                    r.reduced_debug_info = p.reduced_debug_info;
                    r.do_limited_crafting = p.limited_crafting;
                }
                self.streaming.radius = self
                    .settings
                    .render_distance
                    .min(p.chunk_radius.saturating_add(1))
                    .max(2);
                // handleLogin: startWaitingForNewLevel(.., Reason.OTHER).
                self.enter_remote_dimension(
                    &p.spawn,
                    false,
                    horizon_client::level_loading::Reason::Other,
                );
                // handleLogin: ClientPlayerNetworkEvent.LoggingIn, the new level made.
                self.native_client_player_network_logging_in();
                // No ClientInformation here: the configuration phase sent it,
                // and ClientPacketListener.broadcastClientInformation sends it
                // again only when an option changes it.
                self.remote_chat_login();
            }
            C::Respawn(p) => {
                if let Some(r) = self.remote.as_ref() {
                    self.audio.remove_sound_entity(r.entity_id as u32);
                }
                // A new player, without effects (the server sends them
                // again) and with no item in use.
                self.potions.effects.clear();
                // The server sends the level, chunks and entities again.
                if let Some(r) = self.remote.as_mut() {
                    r.attachments
                        .retain(|holder, _| matches!(holder, Holder::Player(_)));
                }
                self.using = None;
                self.end_mod_use();
                // After a death (no KEEP_ENTITY_DATA) the new LocalPlayer
                // rides nothing: the server pairs a vehicle again with its
                // passengers. A dimension change keeps the vehicle it travels in.
                if p.keep & 2 == 0 {
                    self.riding = None;
                    // KEEP_ENTITY_DATA copies the old player's synched data
                    // (its shared flags, its score) to the new one; without it they reset.
                    self.experience.score = 0;
                    if let Some(r) = self.remote.as_mut() {
                        r.own_invisible = false;
                        r.own_on_fire = false;
                        r.local_entity_silent = false;
                    }
                }
                let dimension_changed = self.dim != Dim::from_id(&p.spawn.dimension);
                let died = self.screen == Screen::Dead
                    || self
                        .player
                        .as_ref()
                        .is_some_and(|m| m.health.health() <= 0.0);
                let reason = horizon_client::level_loading::reason_for_respawn(
                    died,
                    &p.spawn.dimension,
                    self.dim.id(),
                );
                if dimension_changed {
                    horizon_client::spans::startup::arm("dimension.change");
                }
                // handleRespawn makes a new ClientLevel for another dimension
                // key (a proxy's server switch too): its predictions count from 0.
                if let Some(r) = self.remote.as_mut()
                    && r.level != p.spawn.dimension
                {
                    r.sequence = 0;
                }
                self.portals.respawn_attributes.respawn(p.keep & 1 != 0);
                self.enter_remote_dimension(&p.spawn, true, reason);
                // handleRespawn adds the new LocalPlayer after what the kept
                // level holds; a new level holds nothing yet.
                let before: std::collections::HashSet<u32> =
                    self.mobs.iter().map(|m| m.id).collect();
                if let Some(r) = self.remote.as_mut() {
                    r.ticked_before_player = before;
                }
                if dimension_changed {
                    // LevelExtractor.setLevel: a new level drops the highlights.
                    self.test_highlights.clear();
                    let (music, _) = self.situational_music();
                    self.music.stop_playing(music.as_ref(), &mut self.audio);
                }
                self.screen = Screen::Playing;
                if let Some(r) = self.remote.as_mut() {
                    r.death_message = None;
                    r.health = (20.0, 20, 5.0);
                    // handleRespawn: the new LocalPlayer is the camera.
                    r.camera = None;
                }
                self.capture(true);
            }
            C::PlayerPosition(p) => self.remote_teleport(&p)?,
            C::PlayerRotation(p) => self.remote_rotation(&p),
            C::PlayerLookAt(p) => self.remote_look_at(&p),
            // ClientCommonPacketListenerImpl.handlePostEffects.
            C::PostEffects(p) => self.post_effects = p.effects,
            C::SetCamera(p) => self.remote_set_camera(p.camera_id),
            C::LevelChunkWithLight(p) => self.remote_chunk(&p, None),
            C::LightUpdate(p) => self.remote_light_update(&p),
            C::ChunksBiomes(p) => self.remote_chunks_biomes(&p),
            // `poll_remote` forgets a run of them at once.
            C::ForgetLevelChunk(p) => {
                self.forget_remote_columns(&mut [(p.pos[0], p.pos[1])].into_iter().collect());
            }
            C::ChunkBatchFinished(_) => {
                // ChunkBatchSizeCalculator's ceiling; meshing runs on workers.
                self.send(S::ChunkBatchReceived(play::ChunkBatchReceived {
                    chunks_per_tick: 64.0,
                }));
            }
            C::BlockUpdate(p) => self.server_block(p.pos, p.state),
            C::AddTransientBlock(p) => self.add_transient_block(p.pos, p.state),
            // ClientPacketListener.handleBlockDestruction: another player's crack.
            C::BlockDestruction(p) => {
                self.destroy_block_progress(p.id, p.pos, i32::from(p.progress));
            }
            C::SectionBlocksUpdate(p) => {
                let [sx, sy, sz] = p.section;
                for change in p.changes {
                    let local = (change & 0xFFF) as i32;
                    let pos = [
                        sx * 16 + (local >> 8 & 15),
                        sy * 16 + (local & 15),
                        sz * 16 + (local >> 4 & 15),
                    ];
                    self.server_block(pos, (change >> 12) as i32);
                }
            }
            C::BlockChangedAck(p) => self.end_predictions(p.sequence),
            // Level.blockEvent on the client level: the block's triggerEvent
            // runs on the client's copy of whatever is at pos (a piston's
            // moveBlocks builds the moving block entities it animates, a note
            // block plays); what it doesn't handle reaches the client copy of
            // the block entity (chest lids, shulker boxes).
            C::BlockEvent(p) => {
                let pos = horizon_core::position::BlockPos::new(p.pos[0], p.pos[1], p.pos[2]);
                let triggered = self.with_level(|b, level, ticks| {
                    b.client_block_event(level, ticks, pos, i32::from(p.b0), i32::from(p.b1))
                });
                self.moving_pistons_stale = true;
                self.process_level_events()?;
                if !triggered {
                    let state = self.world.block(p.pos[0], p.pos[1], p.pos[2]);
                    let block = &self.res.facts().get(state).name;
                    let named = self.remote.as_ref().and_then(|r| {
                        r.builtin
                            .get("minecraft:block")?
                            .name(p.block)
                            .map(str::to_owned)
                    });
                    if named.as_deref() == Some(&**block) {
                        self.block_entity_event(p.pos, i32::from(p.b0), i32::from(p.b1));
                    }
                }
            }
            // ClientLevel.levelEvent: LevelEventHandler, as this client's own
            // level plays them (particles and sounds: a spawner's burst, bone
            // meal, waxing, a block broken by someone else).
            // ClientPacketListener.handleLevelEvent: a global one is heard
            // from anywhere (ClientLevel.globalLevelEvent).
            C::LevelEvent(p) if p.global => self.global_level_event_sound(p.kind, p.pos),
            C::LevelEvent(p) => self.level_event_at(p.kind, p.pos, p.data),
            C::LevelParticles(p) => self.remote_level_particles(&p),
            C::Explode(p) => self.remote_explode(&p),
            C::Sound(p) => {
                if let Some(name) = self.remote_sound_name(&p.sound)
                    && !self.pvp_mutes_sound(&name)
                {
                    let at = [
                        f64::from(p.x) / 8.0,
                        f64::from(p.y) / 8.0,
                        f64::from(p.z) / 8.0,
                    ];
                    // ClientPacketListener.handleSoundEvent plays with packet.getSource().
                    match usize::try_from(p.source)
                        .ok()
                        .filter(|s| *s < horizon_client::settings::SOUND_SOURCES.len())
                    {
                        Some(source) => {
                            self.audio
                                .play_on(source, &name, Some(at), p.volume, p.pitch)
                        }
                        None => self.audio.play(&name, Some(at), p.volume, p.pitch),
                    }
                }
            }
            C::SoundEntity(p) => {
                if let Some(name) = self.remote_sound_name(&p.sound)
                    && !self.pvp_mutes_sound(&name)
                    && let Some(state) = self.remote_sound_entity_state(p.id)
                {
                    let source = usize::try_from(p.source)
                        .ok()
                        .filter(|s| *s < horizon_client::settings::SOUND_SOURCES.len())
                        .unwrap_or_else(|| horizon_client::settings::sound_source(&name));
                    let entity = horizon_client::sound_engine::SoundEntity::new(
                        p.id as u32,
                        state.position,
                        state.silent,
                    );
                    entity.update(state);
                    self.audio
                        .play_entity_seeded(&name, source, p.volume, p.pitch, p.seed, entity);
                }
            }
            C::StopSound(p) => self
                .audio
                .stop_sounds(p.sound.as_deref(), p.source.map(|s| s as usize)),
            C::AddEntity(p) => {
                self.audio.remove_sound_entity(p.id as u32);
                let kind = self
                    .remote
                    .as_ref()
                    .and_then(|r| {
                        r.builtin
                            .get("minecraft:entity_type")?
                            .name(p.kind)
                            .map(str::to_owned)
                    })
                    .unwrap_or_default();
                let rotation =
                    [p.yaw, p.pitch, p.head_yaw].map(horizon_protocol::buf::unpack_degrees);
                if let Some(r) = self.remote.as_mut() {
                    // ClientLevel.addEntity: an id it holds already is removed
                    // and added again, after the player.
                    r.ticked_before_player.remove(&(p.id as u32));
                    // A new entity looks its PlayerInfo up afresh.
                    r.level_player_textures.remove(&p.id);
                    if kind == "minecraft:player" {
                        r.level_players.insert(p.id, p.uuid);
                        // createEntityFromPacket: the player takes the profile of its list entry.
                        match r.players.get(&p.uuid) {
                            Some(name) => r.level_player_names.insert(p.id, name.clone()),
                            None => r.level_player_names.remove(&p.id),
                        };
                    } else {
                        r.level_players.remove(&p.id);
                        r.level_player_names.remove(&p.id);
                    }
                }
                // Entity.getTeam: a mob is on a team by its UUID, a player by
                // its profile name (`Game::scoreboard_name`).
                self.persist.uuids.insert(
                    p.id as u32,
                    horizon_client::lan::playerdata::uuid_ints(p.uuid),
                );
                let replace = self.tracks_remote_entity(p.id);
                self.spawn_entity(p.id, &kind, p.position, rotation);
                if let Some(r) = self.remote.as_mut() {
                    r.cart_lerps.remove(&p.id);
                    r.lerps.remove(&p.id);
                }
                if !self.spawn_remote_object(
                    p.id,
                    &kind,
                    p.position,
                    (rotation[0], rotation[1]),
                    p.data,
                ) {
                    self.spawn_added_projectile(&p, &kind, (rotation[0], rotation[1]), replace);
                }
                // Entity.recreateFromPacket: objects must exist before applying motion.
                self.remote_motion(p.id, p.velocity);
                if self.minecart_improvements()
                    && self.carts.iter().any(|c| c.id == p.id as u32)
                    && let Some(r) = self.remote.as_mut()
                {
                    r.cart_lerps.insert(p.id, CartLerp::new());
                }
                self.move_remote_text_display(p.id, p.position, Some((rotation[0], rotation[1])));
                self.move_remote_interaction(p.id, p.position);
            }
            C::RemoveEntities(p) => {
                for &id in &p.ids {
                    if let Some(r) = self.remote.as_mut() {
                        if let Some(uuid) = r.level_players.remove(&id) {
                            r.attachments.remove(&Holder::Player(uuid));
                        }
                        r.level_player_names.remove(&id);
                        r.level_player_textures.remove(&id);
                        r.attachments.remove(&Holder::Entity(id as u32));
                    }
                }
                self.remove_entities(&p.ids);
            }
            C::MoveEntityPos(play::MoveEntityPos(m))
            | C::MoveEntityPosRot(play::MoveEntityPosRot(m))
            | C::MoveEntityRot(play::MoveEntityRot(m)) => {
                // handleMoveEntity: the delta counts from the position codec's
                // base, which then moves to the delta's end.
                let base = self
                    .remote
                    .as_ref()
                    .and_then(|r| r.entities.get(&m.id))
                    .map(|t| t.codec_base);
                let Some(base) = base else { return Ok(()) };
                // VecDeltaCodec.decode: each step's position from the last.
                let path = m.delta.as_ref().map(|d| match d {
                    play::VecDelta::Linear(_) => Path::Linear(d.end(base)),
                    play::VecDelta::Stepped(steps) => {
                        let mut at = base;
                        Path::Stepped(
                            steps
                                .iter()
                                .map(|(delta, ticks)| {
                                    at = play::VecDelta::apply(at, *delta);
                                    (at, *ticks)
                                })
                                .collect(),
                        )
                    }
                });
                let rotation = m.rotation.map(|(y, x)| {
                    (
                        horizon_protocol::buf::unpack_degrees(y),
                        horizon_protocol::buf::unpack_degrees(x),
                    )
                });
                if let (Some(path), Some(t)) = (
                    &path,
                    self.remote.as_mut().and_then(|r| r.entities.get_mut(&m.id)),
                ) {
                    t.codec_base = path.end();
                }
                self.move_entity(m.id, path, rotation, m.on_ground, MovePacket::Move);
            }
            C::EntityPositionSync(p) => {
                let path = match p.path {
                    play::PositionPath::Linear(at) => Path::Linear(at),
                    play::PositionPath::Stepped(steps) => Path::Stepped(steps),
                };
                // handleEntityPositionSync: the position codec's base is set.
                if let Some(t) = self.remote.as_mut().and_then(|r| r.entities.get_mut(&p.id)) {
                    t.codec_base = path.end();
                }
                self.move_entity(
                    p.id,
                    Some(path),
                    Some((p.yaw, p.pitch)),
                    p.on_ground,
                    MovePacket::PositionSync,
                )
            }
            C::TeleportEntity(p) => {
                let base = self
                    .remote
                    .as_ref()
                    .and_then(|r| r.entities.get(&p.id))
                    .map_or([0.0; 3], |t| t.position);
                let rel = |bit: i32, i: usize| {
                    if p.relatives & bit != 0 {
                        base[i] + p.position[i]
                    } else {
                        p.position[i]
                    }
                };
                let position = [
                    rel(play::relative::X, 0),
                    rel(play::relative::Y, 1),
                    rel(play::relative::Z, 2),
                ];
                // setValuesFromPositionPacket: interpolated, unless too far.
                self.move_entity(
                    p.id,
                    Some(Path::Linear(position)),
                    Some((p.yaw, p.pitch)),
                    p.on_ground,
                    MovePacket::Teleport,
                );
                // handleTeleportEntity: the mount this client moves is put
                // there at once.
                if self.driven_mount().is_some_and(|m| m.id as i32 == p.id) {
                    self.remote_move_vehicle(position, p.yaw);
                }
            }
            C::SetPassengers(p) => self.remote_set_passengers(p.vehicle, &p.passengers),
            // ClientPacketListener.handleEntityLinkPacket: who holds a mob's
            // lead (this player, another player, a mob or a fence knot), for
            // the rope.
            C::SetEntityLink(p) => {
                let me = self.remote.as_ref().map(|r| r.entity_id);
                use horizon_client::mobs::leash::Holder;
                let holder = match p.dest {
                    0 => None,
                    d if Some(d) == me => Some(Holder::Player),
                    d => match self.remote_knots.get(&(d as u32)) {
                        Some(&block) => Some(Holder::Knot(block)),
                        // Not a known knot yet: kept by id until the knot
                        // arrives (game/remote_objects.rs).
                        None if self.boats.iter().any(|b| b.id == d as u32) => {
                            Some(Holder::Boat(d as u32))
                        }
                        None => Some(Holder::Mob(d as u32)),
                    },
                };
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.source as u32) {
                    m.extra.leash = holder;
                } else if let Some(b) = self.boats.iter_mut().find(|b| b.id == p.source as u32) {
                    // A leashed boat or raft (AbstractBoat is Leashable).
                    b.leash = holder;
                }
            }
            C::MoveVehicle(p) => self.remote_move_vehicle(p.position, p.yaw),
            C::MoveMinecartAlongTrack(p) => self.remote_move_minecart(p),
            // ClientPacketListener.handleRotateMob: lerpHeadTo(yHeadRot, 3).
            C::RotateHead(p) => {
                if let Some(r) = self.remote.as_mut() {
                    let yaw = horizon_protocol::buf::unpack_degrees(p.head_yaw);
                    r.head_lerps.insert(p.id, (yaw, 3));
                }
            }
            C::SetEntityData(p) => {
                if let Some(r) = self.remote.as_mut() {
                    for (index, _, value) in &p.values {
                        if let (4, Value::Bool(silent)) = (index, value) {
                            if p.id == r.entity_id {
                                r.local_entity_silent = *silent;
                            }
                            if let Some(t) = r.entities.get_mut(&p.id) {
                                t.silent = *silent;
                            }
                        }
                    }
                }
                // Entity.DATA_SHARED_FLAGS_ID, DATA_CUSTOM_NAME and
                // DATA_CUSTOM_NAME_VISIBLE: server NPCs and holograms rely on
                // these instead of saved NBT.
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            (0, Value::Byte(flags)) => {
                                m.invisible = flags & 0x20 != 0;
                                m.glowing = flags & 0x40 != 0;
                            }
                            (2, Value::OptText(name)) => {
                                let text = name.as_ref().map(horizon_client::entity_nbt::name_text);
                                m.custom_name =
                                    text.as_ref().map(horizon_client::text::Text::plain);
                                m.custom_name_text = text;
                            }
                            (2, Value::Text(name)) => {
                                let text = horizon_client::entity_nbt::name_text(name);
                                m.custom_name = Some(text.plain());
                                m.custom_name_text = Some(text);
                            }
                            (3, Value::Bool(visible)) => m.name_visible = *visible,
                            _ => {}
                        }
                    }
                }
                if let Some((_, _, Value::Byte(flags))) = p.values.iter().find(|v| v.0 == 0)
                    && let Some(r) = self.remote.as_mut()
                {
                    if flags & 0x40 != 0 {
                        r.glowing.insert(p.id);
                    } else {
                        r.glowing.remove(&p.id);
                    }
                }
                self.remote_mob_data(p.id, &p.values);
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                    && m.species() == horizon_client::mobs::Species::Creeper
                {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            // Creeper.DATA_SWELL_DIR, DATA_IS_POWERED, DATA_IS_IGNITED.
                            (16, Value::Int(dir)) => m.extra.counter = *dir,
                            (17, Value::Bool(powered)) => m.extra.charged = *powered,
                            (18, Value::Bool(ignited)) => m.extra.flag = *ignited,
                            _ => {}
                        }
                    }
                }
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            // Entity.DATA_TICKS_FROZEN: isFullyFrozen shakes it.
                            (7, Value::Int(ticks)) => m.ticks_frozen = *ticks,
                            // Strider.DATA_SUFFOCATING (after AgeableMob's two
                            // and DATA_BOOST_TIME): a cold strider shakes.
                            (19, Value::Bool(cold))
                                if m.species() == horizon_client::mobs::Species::Strider =>
                            {
                                m.extra.flag = *cold;
                            }
                            _ => {}
                        }
                    }
                }
                // LivingEntity.DATA_LIVING_ENTITY_FLAGS: for another player
                // too, remote_mob_data's apply reads index 8 above.
                let mannequin = self
                    .mobs
                    .iter()
                    .any(|m| m.id == p.id as u32 && &*m.kind.id == "minecraft:mannequin");
                let other = mannequin
                    || self.remote.as_ref().is_some_and(|r| {
                        r.level_players.contains_key(&p.id) && r.entity_id != p.id
                    });
                // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): another
                // player lies down in their bed (drawn lying in it), or gets up.
                if other {
                    for (index, _, value) in &p.values {
                        if let (14, Value::OptPos(bed)) = (index, value) {
                            let code = bed.map_or(0, |b| self.bed_facing(b));
                            if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                                m.extra.ints[1] = code;
                                if let Some(head) = *bed {
                                    // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): setPosToBed.
                                    let at = Vec3::new(
                                        f64::from(head[0]) + 0.5,
                                        f64::from(head[1]) + 0.5625 + 0.125,
                                        f64::from(head[2]) + 0.5,
                                    );
                                    m.position = at;
                                    m.previous = at;
                                    m.head_pitch = 0.0;
                                    m.previous_head_pitch = 0.0;
                                }
                            }
                        }
                        // Avatar.DATA_PLAYER_MAIN_HAND (HumanoidArm: 0 left, 1
                        // right): the arm a swing of the main hand moves.
                        if let (15, Value::Id(arm)) = (index, value)
                            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                        {
                            m.extra.main_hand_left = *arm == 0;
                        }
                        // Avatar.DATA_PLAYER_MODE_CUSTOMISATION: the skin layers
                        // the player shows (PlayerModelPart bits).
                        if let (16, Value::Byte(bits)) = (index, value)
                            && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                        {
                            m.extra.hidden_model_parts = !(*bits as u8) & 0x7F;
                        }
                    }
                }
                // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): this
                // player lies down in the bed, or gets up.
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    for (index, _, value) in &p.values {
                        match (index, value) {
                            (14, Value::OptPos(bed)) => {
                                if bed.is_some() && self.sleeping.is_none() {
                                    self.sleep_timer = 0;
                                }
                                self.sleeping = *bed;
                                if let Some(head) = *bed {
                                    // LivingEntity.onSyncedDataUpdated(SLEEPING_POS_ID): setPosToBed.
                                    let at = Vec3::new(
                                        f64::from(head[0]) + 0.5,
                                        f64::from(head[1]) + 0.5625 + 0.125,
                                        f64::from(head[2]) + 0.5,
                                    );
                                    if let Some(pl) = self.player.as_mut() {
                                        pl.position = at;
                                        pl.velocity = Vec3::ZERO;
                                        pl.pitch = 0.0;
                                    }
                                    self.previous_position = at;
                                    self.pitch = 0.0;
                                    self.snap_player_model_pitch();
                                }
                            }
                            // Entity.DATA_SHARED_FLAGS_ID's sprinting bit: the
                            // server stops the sprint after a hit's extra
                            // knockback (the flag only, as assignValues sets
                            // it: the speed modifier and wasSprinting stay).
                            // Its invisible bit (Entity.isInvisible): the
                            // Invisibility effect's, or a plugin's that hides
                            // the real body under a pose (#3788).
                            (0, Value::Byte(flags)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.sprinting = flags & 0x08 != 0;
                                    // Entity.DATA_SHARED_FLAGS_ID's fall-flying
                                    // bit (0x80, `Entity.setSharedFlag(7, ...)`):
                                    // `LivingEntity.updateFallFlying` clears the
                                    // glide on the server only and `LocalPlayer`
                                    // never ends one itself, so this packet is
                                    // the glide's own end. Applied when it is
                                    // handled — the frame's poll, before its
                                    // ticks — as ClientPacketListener does, so a
                                    // clear that arrives mid-tick lets that tick
                                    // keep its glide travel (#7616).
                                    pl.fall_flying = flags & -128 != 0;
                                }
                                let on_fire = flags & 0x01 != 0;
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_invisible = flags & 0x20 != 0;
                                    let was_on_fire =
                                        std::mem::replace(&mut r.own_on_fire, on_fire);
                                    // The server put the fire out: the
                                    // window's countdown ends with it.
                                    if was_on_fire
                                        && !on_fire
                                        && let Some(pl) = self.player.as_mut()
                                    {
                                        pl.fire_ticks = pl.fire_ticks.min(0);
                                    }
                                }
                            }
                            // Entity.DATA_AIR_SUPPLY_ID and
                            // Player.DATA_PLAYER_ABSORPTION_ID: the bubbles
                            // and golden hearts the server's player has.
                            (1, Value::Int(air)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.air = *air;
                                }
                            }
                            // Entity.DATA_TICKS_FROZEN: the server's count of the
                            // powder snow freeze, which the client's own copy reads
                            // for the frost overlay, the frozen hearts and the
                            // shiver of its third-person model (#11217).
                            (7, Value::Int(ticks)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    pl.ticks_frozen = *ticks;
                                }
                            }
                            // LivingEntity.DATA_ARROW_COUNT_ID and DATA_STINGER_COUNT_ID.
                            (12, Value::Int(count)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_arrows = *count;
                                }
                            }
                            (13, Value::Int(count)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_stingers = *count;
                                }
                            }
                            (19 | 20, Value::Id(v)) => {
                                if let Some(r) = self.remote.as_mut() {
                                    r.own_shoulders[usize::from(*index == 20)] =
                                        (*v > 0).then(|| (*v - 1).clamp(0, 4));
                                }
                            }
                            (17, Value::Float(amount)) => {
                                if let Some(pl) = self.player.as_mut() {
                                    let _ = pl.internal_set_absorption(*amount);
                                }
                            }
                            (18, Value::Int(score)) => self.experience.score = *score,
                            // LivingEntity.DATA_LIVING_ENTITY_FLAGS: the server
                            // starts or stops this player's use, as an axe that
                            // disables the raised shield does (Player.disableShield's
                            // stopUsingItem). LocalPlayer.onSyncedDataUpdated.
                            (8, Value::Byte(flags)) => {
                                let selected = self.selected;
                                let using = self.using;
                                self.using = synced_local_use(using, *flags, selected, |slot| {
                                    self.stack_in(slot).is_some()
                                });
                                // onSyncedDataUpdated's useItemRemaining: getUseDuration,
                                // without startUsingItem's event.
                                if using.is_none()
                                    && let Some((slot, _)) = self.using
                                    && let Some(stack) = self.stack_in(slot).cloned()
                                {
                                    self.use_duration = self.use_duration(&stack);
                                    self.use_start = super::using_item::UseStart::of(stack);
                                }
                            }
                            _ => {}
                        }
                    }
                }
                // Entity.DATA_SHARED_FLAGS_ID's crouching and sprinting bits:
                // another player's or a mannequin's crouch (their model and
                // Entity.isDiscrete's name tag), and the dust a runner raises.
                if (mannequin
                    || self
                        .remote
                        .as_ref()
                        .is_some_and(|r| r.level_players.contains_key(&p.id)))
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    for (index, _, value) in &p.values {
                        if let (0, Value::Byte(flags)) = (index, value) {
                            m.extra.crouching = flags & 2 != 0;
                            m.extra.sprinting = flags & 8 != 0;
                        }
                    }
                }
                for (index, _, value) in &p.values {
                    if *index == ITEM_ENTITY_ITEM
                        && let Value::Item(stack) = value
                    {
                        self.remote_item_data(p.id, stack);
                    }
                    // ExperienceOrb.DATA_VALUE (the icon's size).
                    if *index == 8
                        && let Value::Int(v) = value
                        && let Some(o) = self.orbs.iter_mut().find(|o| o.id == p.id as u32)
                    {
                        o.value = *v;
                    }
                }
                self.remote_object_data(p.id, &p.values);
                self.remote_projectile_data(p.id, &p.values);
                self.remote_dragon_data(p.id, &p.values);
            }
            // ClientPacketListener.handleSetEquipment: what a mob or another
            // player holds and wears (EquipmentSlot ordinals).
            C::SetEquipment(p) => {
                self.remote_object_equipment(p.id, &p.slots);
                let items: Vec<(u8, Option<Arc<str>>, Option<ItemStack>)> = p
                    .slots
                    .iter()
                    .map(|(slot, stack)| {
                        (
                            *slot,
                            stack.as_ref().map(|s| self.item_name(s.item)),
                            self.to_stack(stack),
                        )
                    })
                    .collect();
                // CrossbowItem.isCharged: the main hand's `charged_projectiles`
                // (AvatarRenderer.getArmPose holds a loaded crossbow up).
                let charged = p.slots.iter().find(|(slot, _)| *slot == 0).map(|(_, s)| {
                    s.as_ref().is_some_and(|s| {
                        matches!(
                            s.component(horizon_protocol::item::component::CHARGED_PROJECTILES),
                            Some(horizon_protocol::item::Component::Stacks(list))
                                if !list.is_empty()
                        )
                    })
                });
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    if let Some(charged) = charged
                        && &*m.kind.id == "minecraft:player"
                    {
                        m.extra.charged = charged;
                    }
                    for (slot, item, stack) in items {
                        let e = &mut m.extra;
                        match slot {
                            0 => match stack {
                                // The components the server sent (a loaded
                                // crossbow's `charged_projectiles`, dyed
                                // leather, an enchanted book's id, ...) are
                                // what the item model draws.
                                Some(stack) => e.set_main_hand_stack(Some(stack)),
                                None => e.set_main_hand(item),
                            },
                            1 => e.off_hand = item,
                            2 => {
                                e.feet = item;
                                e.armor[3] = stack;
                            }
                            3 => {
                                e.legs = item;
                                e.armor[2] = stack;
                            }
                            4 => {
                                e.chest = item;
                                e.armor[1] = stack;
                            }
                            5 => {
                                e.head = item;
                                e.armor[0] = stack;
                            }
                            6 => {
                                // A wolf's armor shows its cracks.
                                e.body_damage = stack.as_ref().map_or(0, |s| s.damage as i32);
                                e.body = item;
                            }
                            7 => m.set_saddle(stack),
                            _ => {}
                        }
                    }
                }
            }
            C::TakeItemEntity(p) => {
                // ClientPacketListener.handleTakeItemEntity: the pickup sound at
                // the orb, item or arrow, whoever collects it (the server sends
                // none); an entity this client doesn't track sounds at this player.
                let orb_at = self
                    .orbs
                    .iter()
                    .find(|o| o.id == p.item as u32)
                    .map(|o| o.position);
                let orb = orb_at.is_some();
                let item = self.dropped.iter().find(|d| d.id == p.item as u32);
                let shot = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|s| s.id == p.item as u32);
                let shot = shot.map(|s| s.position).or_else(|| {
                    self.tridents
                        .iter()
                        .find(|t| t.id == p.item as u32)
                        .map(|t| t.position)
                });
                let at = match (item, shot) {
                    (Some(d), _) => Some([d.position.x, d.position.y, d.position.z]),
                    (None, Some(s)) => Some([s.x, s.y, s.z]),
                    (None, None)
                        if self
                            .remote
                            .as_ref()
                            .is_some_and(|r| r.entity_id == p.player) =>
                    {
                        self.eye_position()
                    }
                    (None, None) => None,
                };
                // `ClientPacketListener.random`, the listener's own stream: a
                // pickup pitch is never a draw of the level's.
                if let Some(o) = orb_at {
                    let mut r = horizon_client::random_source::create();
                    let pitch = (r.next_float() - r.next_float()) * 0.35 + 0.9;
                    self.audio.play(
                        "minecraft:entity.experience_orb.pickup",
                        Some([o.x, o.y, o.z]),
                        0.1,
                        pitch,
                    );
                } else if let Some(at) = at {
                    let mut r = horizon_client::random_source::create();
                    let pitch = ((r.next_float() - r.next_float()) * 0.7 + 1.0) * 2.0;
                    self.audio
                        .play("minecraft:entity.item.pickup", Some(at), 0.2, pitch);
                }
                // ItemPickupParticle toward the collector (this player, another one or a mob).
                let collector = if self
                    .remote
                    .as_ref()
                    .is_some_and(|r| r.entity_id == p.player)
                {
                    super::pickups::Collector::Player
                } else {
                    super::pickups::Collector::Mob(p.player as u32)
                };
                if let Some(item) = self.dropped.iter().find(|d| d.id == p.item as u32).cloned() {
                    self.add_pickup(super::pickups::Taken::Item(Box::new(item)), collector);
                } else if let Some(orb) = self.orbs.iter().find(|o| o.id == p.item as u32).cloned()
                {
                    self.add_pickup(super::pickups::Taken::Orb(Box::new(orb)), collector);
                } else if let Some(shot) = self
                    .creatures
                    .projectiles
                    .iter()
                    .find(|s| s.id == p.item as u32)
                    .cloned()
                {
                    // An arrow flies to its collector too, then goes.
                    self.add_pickup(super::pickups::Taken::Arrow(Box::new(shot)), collector);
                } else if let Some(i) = self.tridents.iter().position(|t| t.id == p.item as u32) {
                    // So does a trident: a server's client keeps them apart.
                    let trident = self.tridents.remove(i);
                    self.add_pickup(super::pickups::Taken::Trident(Box::new(trident)), collector);
                }
                // An orb stays: the server may still hold merged ones in it.
                if !orb {
                    self.remove_entity(p.item);
                }
            }
            // ClientPacketListener.handleSetEntityMotion: knockback on this player.
            C::SetEntityMotion(p) if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) => {
                if let Some(pl) = self.player.as_mut() {
                    pl.velocity = Vec3::new(p.velocity[0], p.velocity[1], p.velocity[2]);
                }
            }
            // The same packet for another entity: Entity.lerpMotion.
            C::SetEntityMotion(p) => self.remote_motion(p.id, p.velocity),
            // ClientPacketListener.handleProjectilePowerPacket: the power a
            // hurting projectile accelerates by, sent bundled with its motion.
            C::ProjectilePower(p) => {
                super::remote_projectiles::remote_projectile_power(
                    &mut self.creatures.projectiles,
                    p.id,
                    p.power,
                );
            }
            // ClientPacketListener.handleDamageEvent: LivingEntity.handleDamageEvent
            // jolts the walk animation and restarts the hurt flash, and the
            // hurt player's own client plays its hurt sound (`Player.getHurtSound`:
            // the damage type's effects) at `getSoundVolume` and
            // `Player.getVoicePitch`, which is 1.0 for every player. The
            // server's sound packet reaches every other player around, never
            // this one (#4149), and that copy is played at 1.0.
            // This player's own hurt tilt and flash come from the server too
            // (sent to the player itself): its own moves never hurt it.
            C::DamageEvent(p) => {
                let me = self.remote.as_ref().map(|r| r.entity_id);
                let own = me == Some(p.id);
                self.pvp_hurt(p.id, own, p.direct.map(|d| Some(d) == me));
                if own {
                    let (attacker, player) = own_hurt_attacker(p.cause, |id| {
                        self.mob_index
                            .get(&self.mobs, id)
                            .map(|m| &*m.kind.id == "minecraft:player")
                    });
                    self.native_client_hurt(attacker, player);
                    // For `events`: this client has no combat tracker of its own here.
                    let cause = self.remote.as_ref().and_then(|r| {
                        r.registries
                            .name("minecraft:damage_type", p.source_type)
                            .map(str::to_owned)
                    });
                    let by = attacker.and_then(|id| {
                        self.mob_index
                            .get(&self.mobs, id)
                            .map(|m| m.kind.id.to_string())
                    });
                    self.events
                        .note_hit(cause.unwrap_or_else(|| "unknown".into()), by);
                    let at = self
                        .player
                        .as_ref()
                        .map(|pl| [pl.position.x, pl.position.y, pl.position.z]);
                    if let Some(pl) = self.player.as_mut() {
                        pl.health.animate_hurt();
                    }
                    if let Some(at) = at {
                        let sound = self.remote_hurt_sound(p.source_type);
                        let pitch = player_voice_pitch();
                        self.audio.play_on(
                            horizon_client::settings::sound_source(sound),
                            sound,
                            Some(at),
                            1.0,
                            pitch,
                        );
                    }
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.limb_speed = 1.5;
                    m.hurt_time = 10;
                } else {
                    self.remote_dragon_hurt(p.id);
                }
            }
            C::HurtAnimation(p) => {
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    // Player.animateHurt: the tilt turns around the yaw.
                    if let Some(pl) = self.player.as_mut() {
                        pl.health.animate_hurt_dir(p.yaw);
                    }
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.hurt_time = 10;
                } else {
                    self.remote_dragon_hurt(p.id);
                }
            }
            C::EntityEvent(p) => {
                if p.event == super::fireworks::FIREWORK_EXPLODE_EVENT {
                    self.remote_firework_explosion(p.id);
                }
                self.remote_fangs_event(p.id as u32, p.event);
                self.remote_boat_event(p.id, p.event);
                // MinecartTNT.handleEntityEvent 70: prime the fuse the window draws.
                self.remote_tnt_cart_event(p.id, p.event);
                // Arrow.handleEntityEvent 0: a tipped arrow's burst of particles.
                if p.event == 0 {
                    self.remote_arrow_puff(p.id as u32);
                }
                // LivingEntity.handleEntityEvent 2: a spear's hit felt.
                if p.event == 2 {
                    self.remote_kinetic_hit(p.id);
                }
                // LocalPlayer.setPermissions also refreshes an open World Options screen.
                if (24..=28).contains(&p.event)
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                {
                    self.ui.registries.permission = (p.event - 24) as u8;
                    self.remote_world_options_permission(p.event >= 26);
                }
                // Player.handleEntityEvent 22 / 23: reduced debug info on / off.
                if (22..=23).contains(&p.event)
                    && let Some(r) = self.remote.as_mut().filter(|r| r.entity_id == p.id)
                {
                    r.reduced_debug_info = p.event == 22;
                }
                // Ravager.handleEntityEvent 69: the roar's puff and push.
                if p.event == 69 {
                    self.remote_ravager_roar(p.id);
                }
                // ClientPacketListener.handleEntityEvent 63: a sniffer's digging sound.
                if p.event == 63 {
                    self.start_sniffer_digging_sound(p.id as u32);
                }
                // ClientPacketListener.handleEntityEvent 21: a guardian's hum.
                if p.event == 21 {
                    self.start_guardian_sound(p.id as u32);
                }
                // ZombieVillager.handleEntityEvent 16: the cure starts.
                if p.event == 16 {
                    self.zombie_villager_cure_sound(p.id as u32);
                }
                // Witch.handleEntityEvent 15: the magic puffs over its hat.
                if p.event == 15 {
                    self.witch_magic_particles(p.id as u32);
                }
                // Fox.handleEntityEvent 45: the crumbs of the food in its mouth.
                if p.event == 45 {
                    self.fox_eat_particles(p.id as u32);
                }
                // Rabbit.handleEntityEvent(1): the hop clocks, and
                // Entity.spawnSprintParticle (one block-dust crumb).
                if p.event == 1 {
                    let hop = {
                        let m = self.mob_index.get_mut(&mut self.mobs, p.id as u32);
                        if let Some(m) = m
                            && m.species() == horizon_client::mobs::Species::Rabbit
                        {
                            let sample =
                                (m.position, m.velocity, f64::from(m.width()), m.supporting);
                            horizon_client::mobs::rabbit::hop_event(m);
                            Some(sample)
                        } else {
                            None
                        }
                    };
                    if let Some((position, movement, width, support)) = hop {
                        let id = p.id as u32;
                        self.sprint_particle(position, movement, support, width, |game| {
                            // this.random: the rabbit's own, x then z.
                            game.mob_index
                                .get_mut(&mut game.mobs, id)
                                .map(|m| (m.random.next_double(), m.random.next_double()))
                        });
                    }
                }
                if p.event == 19
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    horizon_client::mobs::aquatic::squid_cycle_event(m);
                }
                // LivingEntity.handleEntityEvent 55: another player swapped
                // its main and off hand (`swapHandItems`).
                if p.event == 55
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    std::mem::swap(&mut m.extra.main_hand, &mut m.extra.off_hand);
                }
                // LivingEntity.handleEntityEvent 47 to 52, 65 and 68: an
                // equipped item broke (onEquippedItemBroken).
                if let Some(slot) = horizon_client::mobs::item_break::equipment_break_slot(p.event)
                {
                    self.remote_equipment_break(p.id, slot);
                }
                // EntityEvent.DEATH: start the death animation.
                if p.event == 3
                    && let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32)
                {
                    m.health = 0.0;
                }
                // LivingEntity.handleEntityEvent 3 on this player: its death
                // sound, which Player.playSound plays for this client alone
                // (the server's goes to everyone else).
                if p.event == 3
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                    && let Some(pl) = self.player.as_ref()
                {
                    let at = [pl.position.x, pl.position.y, pl.position.z];
                    let sound = "minecraft:entity.player.death";
                    let pitch = player_voice_pitch();
                    self.audio.play_on(
                        horizon_client::settings::sound_source(sound),
                        sound,
                        Some(at),
                        1.0,
                        pitch,
                    );
                }
                // The events that start a mob's animation (the warden's).
                if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    horizon_client::mobs::sync::entity_event(m, p.event as u8);
                }
                // LivingEntity.handleEntityEvent 46: portal particles from the
                // old position to the new one (chorus fruit, an enderman).
                if p.event == 46 {
                    self.entity_teleport_particles(p.id);
                }
                // LivingEntity.handleEntityEvent 60: the poof as the dead body goes.
                if p.event == 60
                    && let Some(m) = self.mob_index.get(&self.mobs, p.id as u32)
                {
                    let (at, w, h) = (m.position, f64::from(m.width()), f64::from(m.height()));
                    self.death_poof([at.x, at.y, at.z], w, h);
                }
                if p.event == super::totem::TOTEM_EVENT {
                    self.remote_totem(p.id);
                }
                // Mob.handleEntityEvent 20: the spawn poof.
                if p.event == 20 && self.mobs.iter().any(|m| m.id == p.id as u32) {
                    self.poof_particles(p.id as u32);
                }
                // The particle bursts of Villager 12 to 14 and 42, TamableAnimal
                // and AbstractHorse 6 and 7, Animal and Allay 18, Ocelot 40 and
                // 41, Dolphin 38: the table in `entity_event_burst` is the one
                // list of them; any other event draws nothing here.
                self.entity_event_particles(p.id as u32, p.event);
                // Snowball and ThrownEgg.handleEntityEvent 3: the crumbs where
                // the projectile is (the server removes it right after).
                if p.event == 3 {
                    let crumbs = self
                        .creatures
                        .projectiles
                        .iter()
                        .find(|pr| pr.id == p.id as u32)
                        .and_then(|pr| {
                            let item = match pr.kind {
                                horizon_client::projectiles::ProjectileKind::Snowball => {
                                    "minecraft:snowball"
                                }
                                horizon_client::projectiles::ProjectileKind::Egg => "minecraft:egg",
                                horizon_client::projectiles::ProjectileKind::Thrown(t) => t.item,
                                _ => return None,
                            };
                            Some(([pr.position.x, pr.position.y, pr.position.z], item))
                        });
                    if let Some((at, item)) = crumbs {
                        self.item_burst(at, item);
                    }
                }
                if p.event == super::combat::DROWN_EVENT {
                    self.remote_drown(p.id);
                }
                // Entity events 53 and 54: honey crumbs while sliding, and the
                // ten from a landing (`HoneyBlock.showSlideParticles` /
                // `showJumpParticles`).
                if horizon_client::landing::honey_particle_count(p.event).is_some() {
                    let at = if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                        self.player
                            .as_ref()
                            .map(|q| [q.position.x, q.position.y, q.position.z])
                    } else {
                        self.mobs
                            .iter()
                            .find(|m| m.id == p.id as u32)
                            .map(|m| [m.position.x, m.position.y, m.position.z])
                    };
                    if let Some(at) = at {
                        self.honey_entity_particles(at, p.event);
                    }
                }
                if p.event == super::consume::USE_ITEM_COMPLETE_EVENT
                    && self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                {
                    self.remote_complete_using();
                }
            }
            C::Animate(p) => self.remote_animate(p.id, p.action),
            // ClientPacketListener.handleSwingAnimation: the entity swings
            // (LivingEntity.swing, which a swing in its first half ignores).
            C::SwingAnimation(p) => {
                let hand = if p.hand == 1 { Hand::Off } else { Hand::Main };
                let animation = horizon_client::first_person::SwingAnimation {
                    kind: p.kind.clamp(0, 255) as u8,
                    duration: p.duration,
                };
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) {
                    self.swing_hand(hand, animation);
                } else if let Some(m) = self.mob_index.get_mut(&mut self.mobs, p.id as u32) {
                    m.swing_hand_modified(hand == Hand::Off, animation);
                }
            }
            // The player's own instances: its maximum health (the hearts the
            // HUD draws and the health it may hold), reach, speed and so on.
            // Then the received health again, which the old maximum clamped.
            C::UpdateAttributes(p) if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id) => {
                let nameplate = self.remote_nameplate_values(&p.values);
                self.apply_own_attributes(p.values);
                if let Some(m) = self.player_model.as_mut() {
                    if let Some(value) = nameplate.name_tag {
                        m.set_name_tag_distance(value);
                    }
                    if let Some(value) = nameplate.below_name {
                        m.set_below_name_distance(value);
                    }
                }
                self.apply_remote_health();
            }
            C::UpdateAttributes(p) => self.remote_mob_attributes(p.id, &p.values),
            C::UpdateMobEffect(p) => self.remote_effect(&p),
            C::RemoveMobEffect(p) => {
                if self.remote.as_ref().is_some_and(|r| r.entity_id == p.id)
                    && let Some(id) = self.remote_effect_id(p.effect)
                {
                    self.potions.effects.list.retain(|e| e.id != id);
                }
            }
            C::ChangeDifficulty(p) => {
                self.ui.difficulty = p.difficulty.clamp(0, 3) as u8;
                self.level.difficulty_locked = p.locked;
                self.remote_world_options_difficulty(self.ui.difficulty, p.locked);
            }
            C::GameRuleValues(p) => self.remote_world_options_rules(p.entries),
            // ClientPacketListener.handleSetHealth: LocalPlayer.hurtTo.
            C::SetHealth(p) => {
                let flash = self.remote.as_mut().is_some_and(|r| {
                    r.health = (p.health, p.food, p.saturation);
                    std::mem::replace(&mut r.flash_on_set_health, true)
                });
                if flash && let Some(pl) = self.player.as_mut() {
                    pl.health.flash_health_change(p.health);
                }
                self.apply_remote_health();
                // The death screen opens (or the respawn is asked for) with the
                // death packet when the rule hides it.
                let shown = self.remote.as_ref().is_none_or(|r| r.show_death_screen);
                if p.health <= 0.0 && shown && self.screen != Screen::Dead && self.player.is_some()
                {
                    self.close_dialogs();
                    self.screen = Screen::Dead;
                    self.ui.death_time = 0;
                    self.ui.death_confirm = None;
                    self.capture(false);
                }
            }
            C::PlayerCombatKill(p) => {
                // The component resolves in the client's current language, as
                // the chat line does, not in the language the session joined
                // with (`Remote.lang` is the vanilla server-language table).
                let message = remote_text(&p.message);
                let me = self.remote.as_ref().is_some_and(|r| r.entity_id == p.id);
                if let Some(r) = self.remote.as_mut() {
                    r.death_message = Some(message);
                }
                // ClientPacketListener.handlePlayerCombatKill: the death
                // screen, whatever health was last sent, or `respawn()` at
                // once when it is hidden.
                if me && self.screen != Screen::Dead && self.player.is_some() {
                    if self.remote.as_ref().is_none_or(|r| r.show_death_screen) {
                        self.close_dialogs();
                        self.screen = Screen::Dead;
                        self.ui.death_time = 0;
                        self.ui.death_confirm = None;
                        self.capture(false);
                    } else {
                        self.respawn_player()?;
                    }
                }
            }
            C::TrackedWaypoint(p) => self.handle_waypoint(&p),
            C::SetExperience(p) => {
                self.experience = Experience {
                    level: p.level,
                    progress: p.progress,
                    total: p.total,
                    score: self.experience.score,
                }
            }
            // ClientPacketListener.handleSetTime: the game time, then
            // ClientClockManager.handleUpdates: the clock catches up with the
            // game time and takes the server's state. Only this level's
            // default world clock is applied — a packet that only updates
            // another clock (the End's) must not overwrite day_time, or the
            // sky follows game_time and looks inverted vs the server's day.
            C::SetTime(p) => {
                self.world.game_time = p.game_time;
                self.remote_clock_tick();
                let clock = {
                    let r = self.remote.as_ref();
                    let names =
                        r.and_then(|r| r.registries.registries.get("minecraft:world_clock"));
                    let tracked = r.and_then(|r| r.clock);
                    let wanted = r
                        .and_then(|r| r.dimension.default_clock.clone())
                        .or_else(|| r.map(|r| r.dimension.name.clone()))
                        .unwrap_or_else(|| "minecraft:overworld".into());
                    let name = |id: i32| {
                        names
                            .and_then(|n| n.get(id as usize))
                            .map(|(n, _)| n.as_str())
                    };
                    p.clocks
                        .iter()
                        .find(|c| name(c.clock) == Some(wanted.as_str()))
                        .or_else(|| tracked.and_then(|id| p.clocks.iter().find(|c| c.clock == id)))
                        .cloned()
                };
                if let Some(c) = clock {
                    self.world.day_time = c.total_ticks;
                    if let Some(r) = self.remote.as_mut() {
                        r.clock = Some(c.clock);
                        r.clock_rate = c.rate;
                        r.clock_partial = c.partial_tick;
                    }
                }
            }
            C::GameEvent(p) => match p.event {
                // NO_RESPAWN_BLOCK_AVAILABLE: ClientPacketListener.handleGameEvent
                // sends the player this system message.
                0 => self.system_message(horizon_client::text::Text::translate(
                    "block.minecraft.spawn.not_valid",
                    &[],
                )),
                // LEVEL_CHUNKS_LOAD_START: the server has begun sending the level.
                13 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.level_load.loading_packets_received();
                    }
                }
                // GUARDIAN_ELDER_EFFECT: the apparition, and the curse sound
                // unless the elder guardian is silent.
                10 => {
                    self.creatures.elder_apparition = super::mob_blocks::APPARITION_TICKS;
                    if p.param == 1.0
                        && let Some(at) = self.player.as_ref().map(|m| m.position)
                    {
                        self.audio.play(
                            "minecraft:entity.elder_guardian.curse",
                            Some([at.x, at.y, at.z]),
                            1.0,
                            1.0,
                        );
                    }
                }
                // WIN_GAME: the poem and credits, then PERFORM_RESPAWN.
                4 if self.screen != Screen::Win => {
                    self.open_win_screen(true, super::win::Finish::RemoteRespawn);
                }
                1 => {
                    self.world.weather.raining = true;
                    self.world.weather.rain_level = (0.0, 0.0);
                }
                2 => {
                    self.world.weather.raining = false;
                    self.world.weather.rain_level = (1.0, 1.0);
                }
                3 => {
                    let mode = p.param as i32;
                    self.set_mode_id(mode);
                    if let Some(r) = self.remote.as_mut() {
                        r.game_mode = mode;
                    }
                }
                // IMMEDIATE_RESPAWN: LocalPlayer.setShowDeathScreen.
                11 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.show_death_screen = p.param == 0.0;
                    }
                }
                // PLAY_ARROW_HIT_SOUND: ClientPacketListener.handleGameEvent
                // plays ARROW_HIT_PLAYER at player eye height.
                6 => {
                    if let Some(at) = self.eye_position() {
                        self.audio
                            .play("minecraft:entity.arrow.hit_player", Some(at), 0.18, 0.45);
                    }
                }
                // RAIN_LEVEL_CHANGE: ClientPacketListener.handleGameEvent.
                7 => {
                    self.world.weather.rain_level = (p.param, p.param);
                    self.world.weather.raining = p.param > 0.0;
                }
                // THUNDER_LEVEL_CHANGE: ClientPacketListener.handleGameEvent.
                8 => {
                    self.world.weather.thunder_level = (p.param, p.param);
                    self.world.weather.thundering = p.param > 0.0;
                }
                // PUFFER_FISH_STING: ClientPacketListener.handleGameEvent
                // plays PUFFER_FISH_STING at player position.
                9 => {
                    if let Some(at) = self.player.as_ref().map(|m| m.position) {
                        self.audio.play(
                            "minecraft:entity.puffer_fish.sting",
                            Some([at.x, at.y, at.z]),
                            1.0,
                            1.0,
                        );
                    }
                }
                // LIMITED_CRAFTING: LocalPlayer.setDoLimitedCrafting.
                12 => {
                    if let Some(r) = self.remote.as_mut() {
                        r.do_limited_crafting = p.param == 1.0;
                    }
                }
                _ => {}
            },
            C::PlayerAbilities(p) => {
                self.creative = p.flags & 8 != 0;
                if let Some(r) = self.remote.as_mut() {
                    r.abilities = Some(p);
                }
                self.apply_remote_abilities();
            }
            // ClientPacketListener.handleSetHeldSlot: a hotbar slot only, and the
            // carried index stays, so the next tick answers with SetCarriedItem.
            C::SetHeldSlot(p) => {
                if (0..9).contains(&p.slot) {
                    self.selected = p.slot as usize;
                }
            }
            C::ContainerSetContent(p) => {
                self.remote_menu_state(p.container, p.state);
                for (slot, item) in p.items.iter().enumerate() {
                    let stack = self.to_stack(item);
                    self.set_menu_slot(p.container, slot as i32, stack);
                }
                self.carried = self.to_stack(&p.carried);
                self.remote_synced();
            }
            C::ContainerSetSlot(p) => {
                self.remote_menu_state(p.container, p.state);
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                // ClientPacketListener.handleContainerSetSlot: a hotbar slot that
                // gains items squeezes for 5 ticks.
                if p.container == 0
                    && (36..=45).contains(&p.slot)
                    && let Some(new) = &stack
                {
                    let (pop, old) = match p.slot {
                        45 => (9, self.offhand.as_ref()),
                        s => (s as usize - 36, self.inventory[s as usize - 36].as_ref()),
                    };
                    if old.is_none_or(|old| old.count < new.count) {
                        self.hotbar_pop[pop] = 5;
                    }
                }
                let before = self.creative_stacks();
                self.set_menu_slot(p.container, i32::from(p.slot), stack);
                self.remote_synced_since(&before);
            }
            C::SetCursorItem(p) => {
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                self.carried = stack;
            }
            C::SetPlayerInventory(p) => {
                let stack = self.to_stack(&p.item);
                self.tutorial_get_item(stack.as_ref());
                let before = self.creative_stacks();
                let held = match p.slot {
                    0..=35 => self.inventory[p.slot as usize].as_ref(),
                    40 => self.offhand.as_ref(),
                    _ => None,
                };
                let changed = !super::hand::matches_for_swap(held, stack.as_ref());
                if let Ok(slot) = usize::try_from(p.slot) {
                    self.remote_hand_slot_set(slot, changed);
                }
                match p.slot {
                    0..=35 => self.inventory[p.slot as usize] = stack,
                    36..=39 => self.armor[3 - (p.slot - 36) as usize] = stack,
                    // Inventory.SLOT_OFFHAND.
                    40 => self.offhand = stack,
                    _ => {}
                }
                self.remote_synced_since(&before);
            }
            C::ContainerSetData(p) if self.open_menu_name(p.container) == "minecraft:lectern" => {
                self.remote_lectern_page(i32::from(p.value));
            }
            C::ContainerSetData(p) if self.remote_station_open(p.container) => {
                self.remote_station_data(usize::try_from(p.id).unwrap_or(usize::MAX), p.value);
            }
            C::ContainerSetData(p) if self.native_screen_data(p.container, p.id, p.value) => {}
            C::ContainerSetData(p) => {
                let v = i32::from(p.value);
                match self
                    .container
                    .and_then(|key| self.world.block_entities.get_mut(&key))
                {
                    Some(BlockEntity::Furnace(f)) => match p.id {
                        0 => f.lit_remaining = v,
                        1 => f.lit_total = v,
                        2 => f.cooking_timer = v,
                        3 => f.cooking_total = v,
                        _ => {}
                    },
                    // CrafterBlockEntity.containerData.
                    Some(BlockEntity::Crafter(c)) => match usize::try_from(p.id) {
                        Ok(k @ 0..9) => c.disabled[k] = v == 1,
                        Ok(9) => c.triggered = v,
                        _ => {}
                    },
                    // BrewingStandBlockEntity.dataAccess.
                    Some(BlockEntity::Brewing(b)) => match p.id {
                        0 => b.brew_time = v,
                        1 => b.fuel = v,
                        2 => b.total_brew_time = v,
                        3 => b.total_fuel = v,
                        _ => {}
                    },
                    _ => {}
                }
            }
            C::OpenScreen(p) => self.remote_open_screen(p),
            C::MountScreenOpen(p) => self.remote_open_mount(p),
            C::RecipeBookAdd(p) => self.remote_recipe_book_add(p),
            C::RecipeBookRemove(p) => self.remote_recipe_book_remove(p),
            C::RecipeBookSettings(p) => self.remote_recipe_book_settings(p),
            C::PlaceGhostRecipe(p) => self.remote_ghost_recipe(p),
            C::MerchantOffers(p) => self.remote_merchant_offers(p),
            C::ContainerClose(_) => {
                // `LocalPlayer.clientSideCloseContainer` sets the screen to
                // null, including dialogs and WaitingForResponseScreen. This
                // differs from clearDialog, which restores the parent screen.
                let dialog = self.dialog_open();
                if dialog {
                    self.close_dialogs();
                    self.ui.chat_screen = None;
                    self.book = None;
                    self.sign_edit = None;
                }
                if !self.remote_lectern_closed() && (self.screen == Screen::Inventory || dialog) {
                    self.remote_close_local();
                    self.screen = Screen::Playing;
                    self.capture(true);
                }
            }
            C::TagQuery(p) => self.answer_tag_query(p.transaction, p.tag),
            C::Unknown(id, data)
                if id == horizon_protocol::ids::play::clientbound::COMMAND_SUGGESTIONS =>
            {
                self.handle_command_suggestions(&data);
            }
            C::CustomChatCompletions(p) => {
                // ClientPacketListener.handleCustomChatCompletions.
                if let Some(r) = self.remote.as_mut() {
                    horizon_client::command::modify_custom_completions(
                        &mut r.custom_completions,
                        horizon_client::command::CustomCompletionAction::from_id(p.action),
                        &p.entries,
                    );
                    self.ui.registries.custom_completions = r.custom_completions.clone();
                }
            }
            C::Commands(p) => {
                let tree = horizon_client::command::network::from_packet(
                    &p,
                    horizon_client::command::vanilla::dispatcher(),
                );
                // The chat's tree follows the server's now; a mod's commands
                // go into it again (`chat_tree_for`).
                let chat_tree = self.chat_tree_for(Some(&tree));
                if let Some(r) = self.remote.as_mut() {
                    r.chat_tree = chat_tree;
                    r.chat_tree_received = true;
                }
            }
            C::Cooldown(p) => {
                // ClientPacketListener.handleItemCooldown: the server starts
                // (or confirms) a group's cooldown; 0 ends it.
                let group: Arc<str> = Arc::from(p.group.as_str());
                if p.duration == 0 {
                    self.cooldowns.remove(&group);
                } else {
                    self.cooldowns.add(group, p.duration);
                }
            }
            // ClientPacketListener.handleGameTestHighlightPos.
            C::GameTestHighlightPos(p) => self.highlight_test_pos(p.absolute, p.relative),
            // ClientPacketListener.handleTestInstanceBlockStatus.
            C::TestInstanceBlockStatus(p) => {
                self.block_editor_status(remote_text(&p.status), p.size);
            }
            C::SystemChat(p) => {
                // The component as sent, translated in the client's own
                // language; mods may change or cancel it. ChatListener.handleOverlay
                // for the action bar.
                let mut text = remote_text(&p.content);
                if !self.chat_received(true, p.overlay, 0, None, &mut text) {
                    return Ok(());
                }
                if p.overlay {
                    self.say_system_queued(&text);
                    self.titles.set_overlay(text, false);
                } else if self.chat_line(text.clone(), horizon_client::chat::ChatSource::Server) {
                    // handleSystemMessage: said once the chat takes it.
                    self.say_system_chat_queued(&text);
                }
            }
            C::PlayerChat(p) => {
                use horizon_client::scoreboard::tag_to_json;
                // SignedMessageBody.Packed.unpack: the last seen it names must
                // be in the cache; then the message joins it
                // (MessageSignatureCache.push).
                let Some(r) = self.remote.as_mut() else {
                    return Ok(());
                };
                // handlePlayerChat: the global index counts every chat packet; one out of
                // order disconnects.
                let expected = r.next_chat_index;
                r.next_chat_index = r.next_chat_index.wrapping_add(1);
                if p.global_index != expected {
                    log::error!(
                        "Missing or out-of-order chat message from server, expected index {expected} but got {}",
                        p.global_index
                    );
                    let reason = horizon_client::text::Text::translate(
                        "multiplayer.disconnect.bad_chat_index",
                        &[],
                    );
                    return self.leave_server(Some(reason.plain()));
                }
                let last_seen: Option<Vec<Signature>> =
                    p.last_seen.iter().map(|s| r.signatures.unpack(s)).collect();
                let Some(last_seen) = last_seen else {
                    log::error!(
                        "Message from player with ID {:032x} referenced unrecognized signature id",
                        p.sender
                    );
                    let reason = horizon_client::text::Text::translate(
                        "multiplayer.disconnect.invalid_packet",
                        &[],
                    );
                    return self.leave_server(Some(reason.plain()));
                };
                let signature = p
                    .signature
                    .as_deref()
                    .and_then(horizon_protocol::secure_chat::signature);
                r.signatures.push(&last_seen, signature.as_ref());
                // The sender's PlayerInfo: none is an error line; else their
                // `SignedMessageValidator` decides (`updateAndValidate`), and a message it
                // takes without a signature of its own shows as unsigned.
                if !r.tab_players.contains_key(&p.sender) {
                    log::error!(
                        "Received player chat packet for unknown player with ID: {:032x}",
                        p.sender
                    );
                    self.handle_chat_message_error(&p.chat_type, signature.as_ref());
                    return Ok(());
                }
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_millis() as i64);
                let incoming = secure_chat::Incoming {
                    sender: p.sender,
                    index: p.index,
                    signature: signature.as_ref(),
                    body: secure_chat::Body {
                        content: &p.content,
                        timestamp: p.timestamp,
                        salt: p.salt,
                        last_seen: &last_seen,
                    },
                    unsigned_content: p.unsigned_content.as_ref(),
                    filter: &p.filter,
                };
                let Some(validated) = r.validate_chat(&incoming, now) else {
                    self.handle_chat_message_error(&p.chat_type, signature.as_ref());
                    return Ok(());
                };
                let signature = signature.filter(|_| validated == secure_chat::Validated::Signed);
                // ChatListener.handlePlayerChatMessage: Only Show Secure Chat
                // shows the signed content, never the server's replacement.
                // The narrator says the message's own decorated content
                // (`narrateChatMessage(boundChatType, message.decoratedContent())`).
                let decorated = p
                    .unsigned_content
                    .as_ref()
                    .map_or_else(|| serde_json::Value::from(p.content.as_str()), tag_to_json);
                let content = if self.settings.file.bool("onlyShowSecureChat") {
                    serde_json::Value::from(p.content.as_str())
                } else {
                    decorated.clone()
                };
                let narration = self
                    .decorate_chat(&p.chat_type, &decorated, Decoration::Narration)
                    .plain();
                let line = self.decorate_chat(&p.chat_type, &content, Decoration::Chat);
                let trust = self.chat_trust(
                    p.sender,
                    signature.is_some(),
                    p.timestamp,
                    &p.content,
                    p.unsigned_content.as_ref(),
                    &line,
                );
                // A hidden or fully filtered message counts as processed, not shown.
                let Some((line, narration)) = self.shown_player_chat(&p, line, narration) else {
                    if let Some(signature) = &signature {
                        self.mark_message_processed(signature, false);
                    }
                    return Ok(());
                };
                self.handle_player_chat(QueuedChat {
                    line,
                    signature,
                    trust,
                    narration,
                    sender: p.sender,
                    chat_type: p.chat_type.chat_type,
                });
            }
            C::DisguisedChat(p) => {
                let content = horizon_client::scoreboard::tag_to_json(&p.message);
                let narration = self
                    .decorate_chat(&p.chat_type, &content, Decoration::Narration)
                    .plain();
                let line = self.decorate_chat(&p.chat_type, &content, Decoration::Chat);
                self.handle_player_chat(QueuedChat {
                    line,
                    signature: None,
                    trust: ChatTrust::Secure,
                    narration,
                    sender: 0,
                    chat_type: p.chat_type.chat_type,
                });
            }
            C::DeleteChat(p) => self.handle_delete_chat(&p.signature)?,
            C::PlayerInfoUpdate(p) => {
                if let Some(r) = self.remote.as_mut() {
                    // handlePlayerInfoUpdate: new entries first, then every
                    // action on every known entry.
                    // INITIALIZE_CHAT: the session (or none) each entry carries; the
                    // player's validator is made again when their chat is next read.
                    if p.actions & 2 != 0 {
                        for e in &p.entries {
                            r.chat_sessions.insert(e.uuid, e.chat_session.clone());
                            r.chat_validators.remove(&e.uuid);
                        }
                    }
                    if p.actions & 1 != 0 {
                        for e in &p.entries {
                            if let Some(name) = &e.name {
                                r.tab_players.entry(e.uuid).or_insert_with(|| {
                                    horizon_client::tab_list::TabPlayer::new(e.uuid, name.clone())
                                });
                                if let Some(t) = e.properties.iter().find(|p| p.name == "textures")
                                {
                                    // Without the services' profile property keys a
                                    // signature can't be checked; a signed property
                                    // counts as genuine.
                                    use horizon_client::skins::{self, Signature};
                                    let signature = if t.signature.is_some() {
                                        Signature::Signed
                                    } else {
                                        Signature::Unsigned
                                    };
                                    let textures =
                                        skins::unpack(&t.value, signature, skins::texture_uris());
                                    r.skin_textures.entry(e.uuid).or_insert(textures);
                                }
                            }
                        }
                    }
                    for e in p.entries {
                        if let Some(t) = r.tab_players.get_mut(&e.uuid) {
                            if let Some(mode) = e.game_mode {
                                t.game_mode = mode;
                            }
                            if let Some(latency) = e.latency {
                                t.latency = latency;
                            }
                            if let Some(name) = &e.display_name {
                                t.display_name =
                                    name.as_ref().map(horizon_client::scoreboard::tag_to_json);
                            }
                            if let Some(order) = e.list_order {
                                t.list_order = order;
                            }
                            if let Some(hat) = e.show_hat {
                                t.show_hat = hat;
                            }
                            match e.listed {
                                Some(true) => {
                                    r.listed.insert(e.uuid);
                                }
                                Some(false) => {
                                    r.listed.remove(&e.uuid);
                                }
                                None => {}
                            }
                        }
                        if let Some(name) = e.name {
                            r.players.insert(e.uuid, name);
                        }
                    }
                }
            }
            C::PlayerInfoRemove(p) => {
                if let Some(r) = self.remote.as_mut() {
                    for u in p.uuids {
                        r.players.remove(&u);
                        r.tab_players.remove(&u);
                        r.chat_sessions.remove(&u);
                        r.chat_validators.remove(&u);
                        r.skin_textures.remove(&u);
                        r.listed.remove(&u);
                        r.attachments.remove(&Holder::Player(u));
                    }
                }
            }
            C::TabList(p) => {
                // handleTabListCustomisation: empty text clears it.
                let text = |t: &Tag| {
                    let json = horizon_client::scoreboard::tag_to_json(t);
                    let text = horizon_client::scoreboard::comp_text(&json);
                    text.spans
                        .iter()
                        .any(|span| !span.text.is_empty())
                        .then_some(text)
                };
                self.tab_list.header = text(&p.header);
                self.tab_list.footer = text(&p.footer);
            }
            C::InitializeBorder(p) => {
                let b = &mut self.border;
                b.set_center([p.center_x, p.center_z]);
                if p.lerp_time > 0 {
                    b.lerp_between(p.old_size, p.new_size, p.lerp_time);
                } else {
                    b.set_size(p.new_size);
                }
                b.set_absolute_max_size(p.absolute_max_size);
                b.warning_distance = p.warning_blocks;
                b.warning_time = p.warning_time;
            }
            C::SetBorderCenter(p) => self.border.set_center([p.x, p.z]),
            C::SetBorderLerpSize(p) => {
                self.border
                    .lerp_between(p.old_size, p.new_size, p.lerp_time)
            }
            C::SetBorderSize(p) => self.border.set_size(p.size),
            C::SetBorderWarningDelay(p) => self.border.warning_time = p.delay,
            C::SetBorderWarningDistance(p) => self.border.warning_distance = p.blocks,
            C::SetObjective(p) => self.remote_objective(p),
            C::SetDisplayObjective(p) => {
                // DisplaySlot ids: list, sidebar, below_name, then the team sidebars.
                let slot = match p.slot {
                    0 => "list".to_owned(),
                    2 => "below_name".to_owned(),
                    n @ 3..=18 => format!(
                        "sidebar.team.{}",
                        horizon_client::text::COLORS[(n - 3) as usize].1
                    ),
                    _ => "sidebar".to_owned(),
                };
                let board = &mut self.score.board;
                let objective = board.objective(&p.objective).map(|o| o.name.clone());
                board.set_display_objective(&slot, objective.as_deref());
            }
            C::SetScore(p) => {
                let board = &mut self.score.board;
                if board.objective(&p.objective).is_some() {
                    let s = board.score_mut(&p.owner, &p.objective);
                    s.value = p.score;
                    s.display = p
                        .display
                        .as_ref()
                        .map(horizon_client::scoreboard::tag_to_json);
                    s.number_format = p.number_format.as_ref().map(number_format);
                }
            }
            C::ResetScore(p) => match &p.objective {
                Some(o) => self.score.board.reset_one(&p.owner, o),
                None => self.score.board.reset_all(&p.owner),
            },
            C::SetPlayerTeam(p) => self.remote_team(p),
            C::SetSimulationDistance(p) => {
                if let Some(r) = self.remote.as_mut() {
                    r.simulation_distance = p.distance;
                }
            }
            C::SetChunkCacheRadius(p) => {
                self.streaming.radius = self
                    .settings
                    .render_distance
                    .min(p.distance.saturating_add(1))
                    .max(2);
                if let Some(r) = self.remote.as_mut() {
                    r.view_distance = p.distance;
                }
                // ClientChunkCache.updateViewRadius: a new storage of that size.
                self.drop_remote_chunks_out_of_range();
            }
            C::SetChunkCacheCenter(p) => {
                if let Some(r) = self.remote.as_mut() {
                    r.chunk_center = (p.x, p.z);
                }
                self.drop_remote_chunks_out_of_range();
            }
            C::BlockEntityData(p) => self.remote_block_entity_data(p.pos, p.kind, &p.tag),
            C::OpenSignEditor(p) => self.remote_open_sign_editor(p),
            C::OpenBook(p) => self.remote_open_book(p.hand),
            // handleSetSpawn: ClientLevel.setRespawnData, which spawn compasses point at.
            C::SetDefaultSpawnPosition(p) => {
                let [x, y, z] = p.pos.pos;
                self.spawn = (p.pos.dimension == "minecraft:overworld")
                    .then(|| Vec3::new(f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));
                // 1.20.2 handleSetSpawn notified ReceivingLevelScreen here; the later
                // LEVEL_CHUNKS_LOAD_START game event did not exist yet.
                if let Some(r) = self.remote.as_mut()
                    && r.session
                        .server_version()
                        .at_most(horizon_protocol::adapter::ServerVersion::V1_20_2)
                {
                    r.level_load.loading_packets_received();
                }
            }
            C::ServerData(p) => {
                if let Some(r) = self.remote.as_mut()
                    && let Some(enforced) = r.session.legacy_secure_chat_enforcement()
                    && r.enforces_secure_chat != enforced
                {
                    r.enforces_secure_chat = enforced;
                    r.chat_validators.clear();
                }
                self.remote_server_data(p.icon.as_deref());
            }
            C::MapItemData(p) => self.remote_map_item_data(&p),
            C::UpdateTags(p) => self.remote_update_tags(&p),
            // handleTickingState / handleTickingStep: the level's TickRateManager.
            C::TickingState(p) => {
                self.clock.set_rate(p.rate);
                self.clock.set_frozen(p.frozen);
            }
            C::TickingStep(p) => self.clock.set_frozen_ticks_to_run(p.steps),
            C::UpdateAdvancements(p) => {
                let icons = p
                    .added
                    .iter()
                    .map(|a| {
                        a.display
                            .as_ref()
                            .map_or_else(String::new, |d| self.item_name(d.icon.item).to_string())
                    })
                    .collect();
                self.remote_advancements(p, icons);
            }
            // handleAwardStats: StatsCounter.setValue for each.
            C::AwardStats(p) => {
                let Some(r) = self.remote.as_ref() else {
                    return Ok(());
                };
                let mut known = Vec::new();
                for (ty, value, count) in p.stats {
                    let Some(ty) = r
                        .builtin
                        .get("minecraft:stat_type")
                        .and_then(|t| t.name(ty))
                    else {
                        continue;
                    };
                    let values = match ty {
                        "minecraft:mined" => "minecraft:block",
                        "minecraft:killed" | "minecraft:killed_by" => "minecraft:entity_type",
                        "minecraft:custom" => "minecraft:custom_stat",
                        _ => "minecraft:item",
                    };
                    if let Some(key) = r.builtin.get(values).and_then(|v| v.name(value)) {
                        known.push((ty.to_owned(), key.to_owned(), count));
                    }
                }
                for (ty, key, count) in known {
                    self.stats.values.entry(ty).or_default().insert(key, count);
                }
            }
            // ClientPacketListener.setActionBarText, setTitleText,
            // setSubtitleText, setTitlesAnimation and handleTitlesClear.
            C::BossEvent(p) => self.apply_boss_event(&p),
            C::SetActionBarText(p) => self.titles.set_overlay(remote_text(&p.text), false),
            C::SetTitleText(p) => self.titles.set_title(remote_text(&p.text)),
            C::SetSubtitleText(p) => self.titles.set_subtitle(remote_text(&p.text)),
            C::SetTitlesAnimation(p) => self.titles.set_times(p.fade_in, p.stay, p.fade_out),
            // ClientCommonPacketListenerImpl.handleShowDialog / handleClearDialog.
            C::ShowDialog(p) => self.show_play_dialog(&p.dialog),
            C::ClearDialog(_) => self.clear_dialog(),
            // ClientPacketListener.handleLowDiskSpaceWarning.
            C::LowDiskSpaceWarning(_) => horizon_client::chunk_io::report_low_disk_space(),
            C::ClearTitles(p) => {
                self.titles.clear();
                if p.value {
                    self.titles.reset_times();
                }
            }
            _ => {}
        }
        Ok(())
=======
    // apply_packet.rs
    pub(in crate::game) fn apply_packet(&mut self, packet: C) -> Result<()> {
        self.apply_packet_body(packet)
>>>>>>> theirs
    }

    // scoreboard.rs
    fn remote_objective(&mut self, p: play::SetObjective) {
        self.remote_objective_body(p)
    }
    fn remote_team(&mut self, p: play::SetPlayerTeam) {
        self.remote_team_body(p)
    }

    // skins.rs
    pub(super) fn publish_tab_object_skins(&mut self) {
        self.publish_tab_object_skins_body()
    }
    pub(super) fn remote_tab_players(
        &mut self,
    ) -> Option<(Vec<horizon_client::tab_list::TabPlayer>, bool, Vec<u128>)> {
        self.remote_tab_players_body()
    }
    pub(super) fn resolve_inline_profiles(
        &mut self,
        profiles: &[std::sync::Arc<serde_json::Value>],
    ) {
        self.resolve_inline_profiles_body(profiles)
    }
    fn poll_own_profile(&mut self) {
        self.poll_own_profile_body()
    }
    fn skin_3d_model(
        &self,
        kind: &std::sync::Arc<horizon_client::mobs::MobType>,
        skin: &str,
        slim: bool,
    ) -> Option<std::sync::Arc<horizon_client::mobs::Part>> {
        self.skin_3d_model_body(kind, skin, slim)
    }
    pub(super) fn update_remote_skins(&mut self) {
        self.update_remote_skins_body()
    }
    pub(super) fn store_skin(&mut self, location: String, rgba: Vec<u8>) {
        self.store_skin_body(location, rgba)
    }
    fn again_skin(&mut self, location: &str) -> bool {
        self.again_skin_body(location)
    }
    pub(super) fn drawn_skin(
        &mut self,
        uuid: u128,
        (skin, slim): (String, bool),
    ) -> (String, bool) {
        self.drawn_skin_body(uuid, (skin, slim))
    }
    pub(super) fn face_skin(&mut self, uuid: u128, skin: &str) -> String {
        self.face_skin_body(uuid, skin)
    }
    pub(super) fn decorate_chat(
        &self,
        bound: &play::ChatTypeBound,
        content: &serde_json::Value,
        decoration: Decoration,
    ) -> horizon_client::text::Text {
        self.decorate_chat_body(bound, content, decoration)
    }

    // entity_updates.rs
    fn remote_item_data(&mut self, id: i32, stack: &Option<Stack>) {
        self.remote_item_data_body(id, stack)
    }
    fn remote_motion(&mut self, id: i32, velocity: [f64; 3]) {
        self.remote_motion_body(id, velocity)
    }
    fn apply_remote_health(&mut self) {
        self.apply_remote_health_body()
    }
    fn place_ghast_under(&mut self, id: u32, spot: Vec3) {
        self.place_ghast_under_body(id, spot)
    }
    fn remote_look_at(&mut self, p: &play::PlayerLookAt) {
        self.remote_look_at_body(p)
    }
    fn remote_teleport(&mut self, p: &play::PlayerPosition) -> Result<()> {
        self.remote_teleport_body(p)
    }

    // tick.rs
    fn tick_client_load(&mut self) {
        self.tick_client_load_body()
    }
    pub(super) fn tick_remote(&mut self) -> Result<()> {
<<<<<<< ours
        // The Java mods' fluids' looks, once their models load (this process's server).
        self.java_mod_fluids();
        self.java_mod_item_models();
        // The 1.20.1 mods' blocks their code drew (this process's server).
        #[cfg(feature = "java-mods-120")]
        self.java120_drawn_models();
        // Their entity types, for the commands typed here.
        #[cfg(feature = "java-mods-120")]
        self.java120_command_types();
        // The tooltips asked of the 1.20.1 mods, to the server.
        #[cfg(feature = "java-mods-120")]
        self.ask_java120_tooltips();
        // Their fluids' looks (this process's server).
        #[cfg(feature = "java-mods-120")]
        self.java120_fluid_looks();
        self.tick_remote_level()?;
        // Its rockets tick after the player (even waiting for chunks), then ParticleEngine.tick.
        self.tick_own_rockets();
        self.tick_remote_particle_engine();
        // Client mods tick on any server, single player's own included.
        self.native_net.to_server = 0;
        self.native_tick();
        self.tell_native_now();
        self.tick_device_client();
        // Far tiles the samplers asked for (`lod_net`).
        if let Some(r) = &self.remote {
            r.session.send_far_terrain();
        }
        self.tick_chat_session();
        // Minecraft.tick sends this even when the player cannot tick yet,
        // including while waiting for chunks. Keep it outside early returns.
        if self.remote.as_ref().is_some_and(Remote::has_level) {
            self.send(S::ClientTickEnd(Empty));
        }
        Ok(())
||||||| base
        self.tick_remote_level()?;
        // Its rockets tick after the player (even waiting for chunks), then ParticleEngine.tick.
        self.tick_own_rockets();
        self.tick_remote_particle_engine();
        // Client mods tick on any server, single player's own included.
        self.native_net.to_server = 0;
        self.native_tick();
        self.tell_native_now();
        // Far tiles the samplers asked for (`lod_net`).
        if let Some(r) = &self.remote {
            r.session.send_far_terrain();
        }
        self.tick_chat_session();
        // Minecraft.tick sends this even when the player cannot tick yet,
        // including while waiting for chunks. Keep it outside early returns.
        if self.remote.as_ref().is_some_and(Remote::has_level) {
            self.send(S::ClientTickEnd(Empty));
        }
        Ok(())
=======
        self.tick_remote_body()
    }
    fn tick_remote_player(&mut self, running: bool, hold: bool) -> Result<()> {
        self.tick_remote_player_body(running, hold)
    }
    fn tick_local_player(
        &mut self,
        entity_collisions: Vec<horizon_core::geometry::Aabb>,
        pushed: Vec3,
        sprint_window: i32,
    ) -> Result<()> {
        self.tick_local_player_body(entity_collisions, pushed, sprint_window)
>>>>>>> theirs
    }

    // tick_entities.rs
    pub(super) fn remote_abilities(&self) -> Option<&play::PlayerAbilities> {
        self.remote_abilities_body()
    }
    fn remote_pushes(&self, before_player: bool) -> Vec3 {
        self.remote_pushes_body(before_player)
    }
    fn tick_remote_guardian_eyes(&mut self, running: bool) {
        self.tick_remote_guardian_eyes_body(running)
    }
    pub(in crate::game) fn tick_remote_arm_poses(&mut self) {
        self.tick_remote_arm_poses_body()
    }
    fn tick_remote_entities(&mut self, running: bool) {
        self.tick_remote_entities_body(running)
    }

    // player_state.rs
    pub(super) fn remote_minecart_lerp(
        &self,
        id: u32,
        partial: f32,
    ) -> Option<([f64; 3], f32, f32)> {
        self.remote_minecart_lerp_body(id, partial)
    }
    fn remote_move_minecart(&mut self, p: play::MoveMinecartAlongTrack) {
        self.remote_move_minecart_body(p)
    }
    fn remote_player_eating(
        &mut self,
        item: &str,
        ticks: u32,
        at: [f64; 3],
        eye: f64,
        pitch: f32,
        yaw: f32,
    ) {
        self.remote_player_eating_body(item, ticks, at, eye, pitch, yaw)
    }
    fn remote_send_state(&mut self, input: u8) {
        self.remote_send_state_body(input)
    }
    pub(in crate::game) fn creative_stacks(&self) -> Vec<Option<ItemStack>> {
        self.creative_stacks_body()
    }
    pub(super) fn send_creative_slots(&mut self) {
<<<<<<< ours
        if !self.creative {
            return;
        }
        let stacks = self.creative_stacks();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut changed = Vec::new();
        for (i, stack) in stacks.into_iter().enumerate() {
            if stack != r.synced[i] {
                r.synced[i] = stack.clone();
                changed.push((creative_sync_slot(i), stack));
            }
        }
        for (slot, stack) in changed {
            let item = self.to_network(&stack);
            self.send(S::SetCreativeModeSlot(play::SetCreativeModeSlot {
                slot,
                item,
            }));
        }
    }
    /// Inventory as the server last described it.
    fn remote_synced(&mut self) {
        let stacks = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            r.synced = stacks;
        }
    }
    /// The slots a server packet changed since `before` are as the server has them; the
    /// others keep what was last sent, so a creative click made since still goes out.
    fn remote_synced_since(&mut self, before: &[Option<ItemStack>]) {
        let now = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            mark_synced(&mut r.synced, before, now);
        }
    }

    /// `MultiPlayerGameMode.stopDestroyBlock`: aborts the break and starts the attack strength
    /// again, if one is running (`if (!isDestroying) return;`), and says whether it did. Only the
    /// progress goes: `destroyBlockPos` and `destroyingItem` keep the target, so a dig resumed on
    /// the same block sends no START and the server's clock runs on (#4124).
    pub(super) fn stop_remote_mining(&mut self) -> bool {
        let kept = self.remote.as_mut().and_then(|r| {
            if !std::mem::take(&mut r.is_destroying) {
                return None;
            }
            let (pos, face, _) = r.destroying?;
            r.destroying = Some((pos, face, 0.0));
            r.destroying
        });
        let Some((pos, _, _)) = kept else {
            return false;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.tutorial_destroy(state, -1.0);
        self.send(S::PlayerAction(play::PlayerAction {
            action: 2,
            pos,
            direction: 0,
            sequence: 0,
        }));
        self.reset_attack_strength();
        true
    }

    /// MultiPlayerGameMode.startDestroyBlock / continueDestroyBlock.
    fn remote_mining(&mut self, start: bool) {
        let debug_stick = self
            .held()
            .is_some_and(|stack| &*stack.item == "minecraft:debug_stick");
        if !self
            .res
            .items()
            .can_destroy_block(self.held(), self.creative)
        {
            self.stop_remote_mining();
            self.mining = None;
            return;
        }
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if !start && r.destroy_delay > 0 {
            r.destroy_delay -= 1;
            return;
        }
        let Some(hit) = self.pick() else {
            self.mining = None;
            return;
        };
        let face = face_direction(hit.face);
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if self.res.facts().is_air(state) {
            return;
        }
        // startDestroyBlock refuses a restricted break before predicting it.
        if self.remote_break_restricted(hit.pos) {
            self.mining = None;
            return;
        }
        if self.creative {
            let seq = self.next_sequence();
            // DebugStickItem.canDestroyBlock never lets the client break the
            // block; the server only selects the property on this START.
            if !debug_stick {
                self.predict_break(hit.pos, state, seq);
            }
            self.send(S::PlayerAction(play::PlayerAction {
                action: 0,
                pos: hit.pos,
                direction: face,
                sequence: seq,
            }));
            if let Some(r) = self.remote.as_mut() {
                r.destroy_delay = 5;
            }
            return;
        }
        // The server's own getDestroyProgress: under water, off the ground,
        // with haste, fatigue, efficiency and the mods' native_break_speed, so
        // the client stops digging when the server is done too (#1254). A
        // block of hardness 0 gives infinity, breaking at once, unless a mod
        // made the speed 0.
        let progress = self.destroy_progress(hit.pos);
        // sameDestroyTarget: the same block and still the item the break began with.
        let held = self.main_hand_key();
        let same = self.remote.as_ref().is_some_and(|r| {
            r.destroying.is_some_and(|(p, _, _)| p == hit.pos) && r.destroying_item == held
        });
        // startDestroyBlock leaves only a break that is still running alone (`isDestroying &&
        // sameDestroyTarget`); a click on the block a stopped break still names starts it again,
        // so the server re-anchors its clock. A held button carries the same target on, and that
        // one keeps the clock (#4124).
        let running = self.remote.as_ref().is_some_and(|r| r.is_destroying);
        if start && same && running {
            return;
        }
        if !same || start {
            // startDestroyBlock aborts only a break that is running; a target left by a stop that
            // ended one is just replaced by the START below.
            let aborted = self.remote.as_mut().and_then(|r| {
                if !std::mem::take(&mut r.is_destroying) {
                    return None;
                }
                r.destroying.take()
            });
            if let Some((old, _, _)) = aborted {
                let old_state = self.world.block(old[0], old[1], old[2]);
                self.tutorial_destroy(old_state, -1.0);
                self.send(S::PlayerAction(play::PlayerAction {
                    action: 2,
                    pos: old,
                    direction: face,
                    sequence: 0,
                }));
            }
            let seq = self.next_sequence();
            self.send(S::PlayerAction(play::PlayerAction {
                action: 0,
                pos: hit.pos,
                direction: face,
                sequence: seq,
            }));
            self.tutorial_destroy(state, 0.0);
            // startDestroyBlock: BlockState.attack on the client, so a redstone
            // ore's dust (RedStoneOreBlock.attack) comes before the break.
            self.redstone_ore_particles(hit.pos);
            if progress >= 1.0 {
                // startDestroyBlock: instant survival breaks have no destroy delay.
                if !debug_stick {
                    self.predict_break(hit.pos, state, seq);
                }
                return;
            }
            if let Some(r) = self.remote.as_mut() {
                r.destroying = Some((hit.pos, face, 0.0));
                r.destroying_item = held;
                r.is_destroying = true;
            }
            // startDestroyBlock plays the first hit and crack itself; the
            // server's level events 2019 and 2020 do the rest.
            self.breaking_block_effects(hit.pos, hit.face, true);
            self.mining = Some(Mining {
                pos: hit.pos,
                progress: 0.0,
                per_tick: 0.0,
                ticks: 0,
            });
            return;
        }
        let Some((pos, last_face, done)) = self.remote.as_ref().and_then(|r| r.destroying) else {
            return;
        };
        let done = done + progress;
        self.tutorial_destroy(state, done.clamp(0.0, 1.0));
        let ticks = self.mining.as_ref().map_or(0, |m| m.ticks) + 1;
        if done >= 1.0 {
            let seq = self.next_sequence();
            // DebugStickItem.canDestroyBlock: the server selects the property
            // and sends the block update instead; never predict a break.
            if !debug_stick {
                self.predict_break(pos, state, seq);
            }
            self.send(S::PlayerAction(play::PlayerAction {
                action: 3,
                pos,
                direction: face,
                sequence: seq,
            }));
            if let Some(r) = self.remote.as_mut() {
                // `destroyBlockPos` remains the last target after the break;
                // if that position is filled again before the next pick, a
                // held attack resumes it without sending a second START.
                r.destroying = Some((pos, face, 0.0));
                r.is_destroying = false;
                r.destroy_delay = 5;
            }
            self.mining = None;
        } else {
            if last_face != face {
                self.send(S::PlayerAction(play::PlayerAction {
                    action: 1,
                    pos,
                    direction: face,
                    sequence: 0,
                }));
            }
            if let Some(r) = self.remote.as_mut() {
                r.destroying = Some((pos, face, done));
            }
            self.mining = Some(Mining {
                pos,
                progress: done,
                per_tick: 0.0,
                ticks,
            });
        }
    }
    /// MultiPlayerGameMode.destroyBlock: the predicted break.
    fn predict_break(&mut self, pos: [i32; 3], state: BlockStateId, sequence: i32) {
        self.player_broke_block(state);
        // BlockStatePredictionHandler.retainKnownServerState.
        let here = self.player_position();
        if let Some(r) = self.remote.as_mut() {
            r.predictions
                .entry(pos)
                .and_modify(|p| p.sequence = sequence)
                .or_insert(Prediction {
                    sequence,
                    state: state.0 as i32,
                    player_pos: here,
                });
        }
        // Block.playerWillDestroy's destroy event (particles and the break
        // sound), which BaseFireBlock skips: the server's extinguish sound
        // is all a player hears.
        let facts = self.res.facts().get(state);
        if !facts.has_class("BaseFireBlock") {
            let s = facts.sounds.clone();
            let at = [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]) + 0.5,
                f64::from(pos[2]) + 0.5,
            ];
            self.audio.play(
                &s.break_sound,
                Some(at),
                (s.volume + 1.0) / 2.0,
                s.pitch * 0.8,
            );
            self.destroy_particles(pos, state);
        }
        // The server's block update follows (waterlogged blocks leave water).
        let air = self.world.air;
        if self.world.set(pos[0], pos[1], pos[2], air).is_ok() {
            self.world.block_entities.remove(&(pos[0], pos[1], pos[2]));
        }
        self.mining = None;
    }
    /// `Player.blockActionRestricted` for the block at `pos`: a spectator
    /// never breaks, an adventure player only with a main-hand item whose
    /// `can_break` matches the block (`ItemStack.canBreakBlockInAdventureMode`).
    /// A predicate this client could not decode (exact components) only
    /// tells that the server gave one, and the server sends the block back
    /// if it does not match.
    fn remote_break_restricted(&self, pos: [i32; 3]) -> bool {
        use horizon_protocol::item::{WirePatch, component::CAN_BREAK};
        if self
            .held()
            .and_then(|s| s.data())
            .is_some_and(|d| d.other("minecraft:can_break").is_some())
        {
            return self.block_action_restricted(pos);
        }
        self.spectator()
            || (!self.may_build()
                && !self
                    .held()
                    .and_then(|s| s.wire_patch())
                    .is_some_and(|p| WirePatch::adds(&p, CAN_BREAK)))
    }
    /// Minecraft.startUseItem: use one crosshair target, then the item itself.
    fn remote_use(&mut self) {
        // Minecraft.startUseItem returns before resetting rightClickDelay
        // while a survival block break is active.
        if self.remote.as_ref().is_some_and(|r| r.is_destroying) {
            return;
        }
        self.use_delay = 4;
        // Minecraft.startUseItem: nothing is used while the hands paddle a boat.
        if self.hands_busy() {
            return;
        }
        // InputEvent.InteractionKeyMappingTriggered for the main hand: a cancel
        // ends the whole click, the off hand gets no turn.
        if let Some(swing) = self.native_interaction(1, Hand::Main) {
            if swing {
                self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
            }
            return;
        }
        // An item a mod claims: its use is the mod's, and the server never hears of it.
        if self.mod_use_start() {
            return;
        }
        // Resolve the crosshair once. A passing entity interaction keeps
        // that target and may only fall back to using the item itself.
        let entity = self.remote_use_targets_entity();
        horizon_client::interaction_input::use_on_target(&mut RemoteUseWorld(self), entity);
    }

    /// `Minecraft.startUseItem`'s entity turn for `hand`: Interact goes to
    /// the server, and whether the click ends there is what this client's
    /// copy of the entity answers (`MultiPlayerGameMode.interact`).
    fn remote_use_entity(&mut self, hand: Hand) -> bool {
        // A menu this use opens (a chest minecart's) is no clicked block's.
        if let Some(r) = self.remote.as_mut() {
            r.use_target = None;
        }
        let sneaking = self.remote_shift_down();
        // A boat, minecart, item frame, armor stand or painting nearer than
        // any mob: the server uses it (AbstractBoat.interact boards it).
        if let Some(hit) = self.remote_object_hit(false)
            && self.mob_hit().is_none_or(|(_, d, _)| hit.distance < d)
            && self.before_block(hit.distance)
        {
            // Interaction.interact answers SUCCESS when it responds and
            // CONSUME otherwise: both end the click (nothing behind the box
            // is used), but only SUCCESS swings; the packet carries the hand.
            let response = self.interactions.get(&hit.id).map(|i| i.response);
            // Objects whose interact passes (paintings, empty or fixed item frames,
            // boats sneaked at or out of control, carts that pass) leave each hand's
            // turn to its item: a crossbow aimed at it shoots.
            let is_painting = self.paintings.iter().any(|p| p.id as i32 == hit.id);
            let held = self.with_hand(hand, |g| g.held().cloned());
            let frame_passes = self
                .item_frames
                .iter()
                .find(|f| f.id as i32 == hit.id)
                .is_some_and(|f| {
                    item_frame_interact_passes(f.fixed, f.item.is_some(), held.is_some())
                });
            let boat_passes = self
                .boats
                .iter()
                .enumerate()
                .find(|(_, b)| b.id as i32 == hit.id)
                .is_some_and(|(i, b)| {
                    boat_interact_passes(
                        b.chest,
                        sneaking,
                        b.out_of_control >= 60.0,
                        self.boat_has_room(i),
                    )
                });
            let (cart_passes, is_rideable_cart) = self
                .carts
                .iter()
                .find(|c| c.id as i32 == hit.id)
                .map(|c| {
                    let ridden = self.riding == Some(c.id)
                        || self.mobs.iter().any(|m| m.extra.mount == Some(c.id));
                    (
                        c.kind
                            .interact_passes(sneaking, ridden, self.can_use_game_master_blocks()),
                        c.kind == super::minecarts::CartKind::Rideable,
                    )
                })
                .unwrap_or((false, false));
            // Cushion.interact on a client: PASS when sneaked at or when
            // something already sits on it, CONSUME (no swing) otherwise.
            let is_cushion = self.cushions.iter().any(|c| c.id as i32 == hit.id);
            let cushion_passes = is_cushion
                && (sneaking
                    || self.riding.is_some_and(|r| r as i32 == hit.id)
                    || self
                        .mobs
                        .iter()
                        .any(|m| m.extra.mount.is_some_and(|r| r as i32 == hit.id)));
            let passes =
                is_painting || frame_passes || boat_passes || cart_passes || cushion_passes;
            // Otherwise the main hand's turn takes it; there is no off hand turn.
            if response.is_none() && hand == Hand::Off && !passes {
                return false;
            }
            self.send(S::Interact(play::Interact {
                id: hit.id,
                hand: hand_id(hand),
                location: hit.location,
                secondary: sneaking,
            }));
            if passes {
                return false;
            }
            // Minecart.interact resets the passenger's easing baseline on
            // a successful boarding click, not on SetPassengers (commands
            // can mount a player without this interaction).
            if let Some(c) = self
                .carts
                .iter_mut()
                .find(|c| c.id as i32 == hit.id && c.kind == super::minecarts::CartKind::Rideable)
            {
                c.player_rotation_offset = c.rotation_offset;
            }
            // MinecartCommandBlock.interact on the client: the editor opens.
            if self.carts.iter().any(|c| {
                c.id as i32 == hit.id && c.kind == super::minecarts::CartKind::CommandBlock
            }) {
                self.open_cart_command_block(hit.id as u32);
            }
            // Minecart.interact boards the cart and answers CONSUME on the client,
            // which ends the click without an arm swing. AbstractBoat.interact
            // answers SUCCESS there, which swings. Cushion.interact answers
            // CONSUME there too.
            if remote_object_swings(response, is_rideable_cart || is_cushion) {
                self.with_hand(hand, Game::swing_arm);
            }
            return true;
        }
        // A Java mod's entity (remote/java.rs).
        if let Some(used) = self.remote_use_java_entity(hand, sneaking) {
            return used;
        }
        if let Some((i, distance, location)) = self.mob_hit()
            && self.before_block(distance)
        {
            let id = self.mobs[i].id as i32;
            self.send(S::Interact(play::Interact {
                id,
                hand: hand_id(hand),
                location,
                secondary: sneaking,
            }));
            // Player.interactOn: another player passes, and so does a mob
            // the held item means nothing to: the item is used instead.
            let player = self
                .remote
                .as_ref()
                .is_some_and(|r| r.level_players.contains_key(&id));
            if !player && self.remote_mob_takes(i, hand) {
                self.remote_feed_particles(i, hand);
                // As for the objects above, only SUCCESS swings the hand: a
                // CONSUME ends the click without one.
                if self.remote_mob_swings(i, hand) {
                    self.with_hand(hand, Game::swing_arm);
                }
                return true;
            }
        }
        false
||||||| base
        if !self.creative {
            return;
        }
        let stacks = self.creative_stacks();
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        let mut changed = Vec::new();
        for (i, stack) in stacks.into_iter().enumerate() {
            if stack != r.synced[i] {
                r.synced[i] = stack.clone();
                changed.push((creative_sync_slot(i), stack));
            }
        }
        for (slot, stack) in changed {
            let item = self.to_network(&stack);
            self.send(S::SetCreativeModeSlot(play::SetCreativeModeSlot {
                slot,
                item,
            }));
        }
    }
    /// Inventory as the server last described it.
    fn remote_synced(&mut self) {
        let stacks = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            r.synced = stacks;
        }
    }
    /// The slots a server packet changed since `before` are as the server has them; the
    /// others keep what was last sent, so a creative click made since still goes out.
    fn remote_synced_since(&mut self, before: &[Option<ItemStack>]) {
        let now = self.creative_stacks();
        if let Some(r) = self.remote.as_mut() {
            mark_synced(&mut r.synced, before, now);
        }
    }

    /// `MultiPlayerGameMode.stopDestroyBlock`: aborts the break and starts the attack strength
    /// again, if one is running (`if (!isDestroying) return;`), and says whether it did. Only the
    /// progress goes: `destroyBlockPos` and `destroyingItem` keep the target, so a dig resumed on
    /// the same block sends no START and the server's clock runs on (#4124).
    pub(super) fn stop_remote_mining(&mut self) -> bool {
        let kept = self.remote.as_mut().and_then(|r| {
            if !std::mem::take(&mut r.is_destroying) {
                return None;
            }
            let (pos, face, _) = r.destroying?;
            r.destroying = Some((pos, face, 0.0));
            r.destroying
        });
        let Some((pos, _, _)) = kept else {
            return false;
        };
        let state = self.world.block(pos[0], pos[1], pos[2]);
        self.tutorial_destroy(state, -1.0);
        self.send(S::PlayerAction(play::PlayerAction {
            action: 2,
            pos,
            direction: 0,
            sequence: 0,
        }));
        self.reset_attack_strength();
        true
    }

    /// MultiPlayerGameMode.startDestroyBlock / continueDestroyBlock.
    fn remote_mining(&mut self, start: bool) {
        let debug_stick = self
            .held()
            .is_some_and(|stack| &*stack.item == "minecraft:debug_stick");
        if !self
            .res
            .items()
            .can_destroy_block(self.held(), self.creative)
        {
            self.stop_remote_mining();
            self.mining = None;
            return;
        }
        let Some(r) = self.remote.as_mut() else {
            return;
        };
        if !start && r.destroy_delay > 0 {
            r.destroy_delay -= 1;
            return;
        }
        let Some(hit) = self.pick() else {
            self.mining = None;
            return;
        };
        let face = face_direction(hit.face);
        let state = self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
        if self.res.facts().is_air(state) {
            return;
        }
        // startDestroyBlock refuses a restricted break before predicting it.
        if self.remote_break_restricted(hit.pos) {
            self.mining = None;
            return;
        }
        if self.creative {
            let seq = self.next_sequence();
            // DebugStickItem.canDestroyBlock never lets the client break the
            // block; the server only selects the property on this START.
            if !debug_stick {
                self.predict_break(hit.pos, state, seq);
            }
            self.send(S::PlayerAction(play::PlayerAction {
                action: 0,
                pos: hit.pos,
                direction: face,
                sequence: seq,
            }));
            if let Some(r) = self.remote.as_mut() {
                r.destroy_delay = 5;
            }
            return;
        }
        // The server's own getDestroyProgress: under water, off the ground,
        // with haste, fatigue, efficiency and the mods' native_break_speed, so
        // the client stops digging when the server is done too (#1254). A
        // block of hardness 0 gives infinity, breaking at once, unless a mod
        // made the speed 0.
        let progress = self.destroy_progress(hit.pos);
        // sameDestroyTarget: the same block and still the item the break began with.
        let held = self.main_hand_key();
        let same = self.remote.as_ref().is_some_and(|r| {
            r.destroying.is_some_and(|(p, _, _)| p == hit.pos) && r.destroying_item == held
        });
        // startDestroyBlock leaves only a break that is still running alone (`isDestroying &&
        // sameDestroyTarget`); a click on the block a stopped break still names starts it again,
        // so the server re-anchors its clock. A held button carries the same target on, and that
        // one keeps the clock (#4124).
        let running = self.remote.as_ref().is_some_and(|r| r.is_destroying);
        if start && same && running {
            return;
        }
        if !same || start {
            // startDestroyBlock aborts only a break that is running; a target left by a stop that
            // ended one is just replaced by the START below.
            let aborted = self.remote.as_mut().and_then(|r| {
                if !std::mem::take(&mut r.is_destroying) {
                    return None;
                }
                r.destroying.take()
            });
            if let Some((old, _, _)) = aborted {
                let old_state = self.world.block(old[0], old[1], old[2]);
                self.tutorial_destroy(old_state, -1.0);
                self.send(S::PlayerAction(play::PlayerAction {
                    action: 2,
                    pos: old,
                    direction: face,
                    sequence: 0,
                }));
            }
            let seq = self.next_sequence();
            self.send(S::PlayerAction(play::PlayerAction {
                action: 0,
                pos: hit.pos,
                direction: face,
                sequence: seq,
            }));
            self.tutorial_destroy(state, 0.0);
            // startDestroyBlock: BlockState.attack on the client, so a redstone
            // ore's dust (RedStoneOreBlock.attack) comes before the break.
            self.redstone_ore_particles(hit.pos);
            if progress >= 1.0 {
                // startDestroyBlock: instant survival breaks have no destroy delay.
                if !debug_stick {
                    self.predict_break(hit.pos, state, seq);
                }
                return;
            }
            if let Some(r) = self.remote.as_mut() {
                r.destroying = Some((hit.pos, face, 0.0));
                r.destroying_item = held;
                r.is_destroying = true;
            }
            // startDestroyBlock plays the first hit and crack itself; the
            // server's level events 2019 and 2020 do the rest.
            self.breaking_block_effects(hit.pos, hit.face, true);
            self.mining = Some(Mining {
                pos: hit.pos,
                progress: 0.0,
                per_tick: 0.0,
                ticks: 0,
            });
            return;
        }
        let Some((pos, last_face, done)) = self.remote.as_ref().and_then(|r| r.destroying) else {
            return;
        };
        let done = done + progress;
        self.tutorial_destroy(state, done.clamp(0.0, 1.0));
        let ticks = self.mining.as_ref().map_or(0, |m| m.ticks) + 1;
        if done >= 1.0 {
            let seq = self.next_sequence();
            // DebugStickItem.canDestroyBlock: the server selects the property
            // and sends the block update instead; never predict a break.
            if !debug_stick {
                self.predict_break(pos, state, seq);
            }
            self.send(S::PlayerAction(play::PlayerAction {
                action: 3,
                pos,
                direction: face,
                sequence: seq,
            }));
            if let Some(r) = self.remote.as_mut() {
                // `destroyBlockPos` remains the last target after the break;
                // if that position is filled again before the next pick, a
                // held attack resumes it without sending a second START.
                r.destroying = Some((pos, face, 0.0));
                r.is_destroying = false;
                r.destroy_delay = 5;
            }
            self.mining = None;
        } else {
            if last_face != face {
                self.send(S::PlayerAction(play::PlayerAction {
                    action: 1,
                    pos,
                    direction: face,
                    sequence: 0,
                }));
            }
            if let Some(r) = self.remote.as_mut() {
                r.destroying = Some((pos, face, done));
            }
            self.mining = Some(Mining {
                pos,
                progress: done,
                per_tick: 0.0,
                ticks,
            });
        }
    }
    /// MultiPlayerGameMode.destroyBlock: the predicted break.
    fn predict_break(&mut self, pos: [i32; 3], state: BlockStateId, sequence: i32) {
        self.player_broke_block(state);
        // BlockStatePredictionHandler.retainKnownServerState.
        let here = self.player_position();
        if let Some(r) = self.remote.as_mut() {
            r.predictions
                .entry(pos)
                .and_modify(|p| p.sequence = sequence)
                .or_insert(Prediction {
                    sequence,
                    state: state.0 as i32,
                    player_pos: here,
                });
        }
        // Block.playerWillDestroy's destroy event (particles and the break
        // sound), which BaseFireBlock skips: the server's extinguish sound
        // is all a player hears.
        let facts = self.res.facts().get(state);
        if !facts.has_class("BaseFireBlock") {
            let s = facts.sounds.clone();
            let at = [
                f64::from(pos[0]) + 0.5,
                f64::from(pos[1]) + 0.5,
                f64::from(pos[2]) + 0.5,
            ];
            self.audio.play(
                &s.break_sound,
                Some(at),
                (s.volume + 1.0) / 2.0,
                s.pitch * 0.8,
            );
            self.destroy_particles(pos, state);
        }
        // The server's block update follows (waterlogged blocks leave water).
        let air = self.world.air;
        if self.world.set(pos[0], pos[1], pos[2], air).is_ok() {
            self.world.block_entities.remove(&(pos[0], pos[1], pos[2]));
        }
        self.mining = None;
    }
    /// `Player.blockActionRestricted` for the block at `pos`: a spectator
    /// never breaks, an adventure player only with a main-hand item whose
    /// `can_break` matches the block (`ItemStack.canBreakBlockInAdventureMode`).
    /// A predicate this client could not decode (exact components) only
    /// tells that the server gave one, and the server sends the block back
    /// if it does not match.
    fn remote_break_restricted(&self, pos: [i32; 3]) -> bool {
        use horizon_protocol::item::{WirePatch, component::CAN_BREAK};
        if self
            .held()
            .and_then(|s| s.data())
            .is_some_and(|d| d.other("minecraft:can_break").is_some())
        {
            return self.block_action_restricted(pos);
        }
        self.spectator()
            || (!self.may_build()
                && !self
                    .held()
                    .and_then(|s| s.wire_patch())
                    .is_some_and(|p| WirePatch::adds(&p, CAN_BREAK)))
    }
    /// Minecraft.startUseItem: use one crosshair target, then the item itself.
    fn remote_use(&mut self) {
        // Minecraft.startUseItem returns before resetting rightClickDelay
        // while a survival block break is active.
        if self.remote.as_ref().is_some_and(|r| r.is_destroying) {
            return;
        }
        self.use_delay = 4;
        // Minecraft.startUseItem: nothing is used while the hands paddle a boat.
        if self.hands_busy() {
            return;
        }
        // InputEvent.InteractionKeyMappingTriggered for the main hand: a cancel
        // ends the whole click, the off hand gets no turn.
        if let Some(swing) = self.native_interaction(1, Hand::Main) {
            if swing {
                self.swing_hand(Hand::Main, SwingAnimation::DEFAULT);
            }
            return;
        }
        // An item a mod claims: its use is the mod's, and the server never hears of it.
        if self.mod_use_start() {
            return;
        }
        // Resolve the crosshair once. A passing entity interaction keeps
        // that target and may only fall back to using the item itself.
        let entity = self.remote_use_targets_entity();
        horizon_client::interaction_input::use_on_target(&mut RemoteUseWorld(self), entity);
    }

    /// `Minecraft.startUseItem`'s entity turn for `hand`: Interact goes to
    /// the server, and whether the click ends there is what this client's
    /// copy of the entity answers (`MultiPlayerGameMode.interact`).
    fn remote_use_entity(&mut self, hand: Hand) -> bool {
        // A menu this use opens (a chest minecart's) is no clicked block's.
        if let Some(r) = self.remote.as_mut() {
            r.use_target = None;
        }
        let sneaking = self.remote_shift_down();
        // A boat, minecart, item frame, armor stand or painting nearer than
        // any mob: the server uses it (AbstractBoat.interact boards it).
        if let Some(hit) = self.remote_object_hit(false)
            && self.mob_hit().is_none_or(|(_, d, _)| hit.distance < d)
            && self.before_block(hit.distance)
        {
            // Interaction.interact answers SUCCESS when it responds and
            // CONSUME otherwise: both end the click (nothing behind the box
            // is used), but only SUCCESS swings; the packet carries the hand.
            let response = self.interactions.get(&hit.id).map(|i| i.response);
            // Objects whose interact passes (paintings, empty or fixed item frames,
            // boats sneaked at or out of control, carts that pass) leave each hand's
            // turn to its item: a crossbow aimed at it shoots.
            let is_painting = self.paintings.iter().any(|p| p.id as i32 == hit.id);
            let held = self.with_hand(hand, |g| g.held().cloned());
            let frame_passes = self
                .item_frames
                .iter()
                .find(|f| f.id as i32 == hit.id)
                .is_some_and(|f| {
                    item_frame_interact_passes(f.fixed, f.item.is_some(), held.is_some())
                });
            let boat_passes = self
                .boats
                .iter()
                .enumerate()
                .find(|(_, b)| b.id as i32 == hit.id)
                .is_some_and(|(i, b)| {
                    boat_interact_passes(
                        b.chest,
                        sneaking,
                        b.out_of_control >= 60.0,
                        self.boat_has_room(i),
                    )
                });
            let (cart_passes, is_rideable_cart) = self
                .carts
                .iter()
                .find(|c| c.id as i32 == hit.id)
                .map(|c| {
                    let ridden = self.riding == Some(c.id)
                        || self.mobs.iter().any(|m| m.extra.mount == Some(c.id));
                    (
                        c.kind
                            .interact_passes(sneaking, ridden, self.can_use_game_master_blocks()),
                        c.kind == super::minecarts::CartKind::Rideable,
                    )
                })
                .unwrap_or((false, false));
            // Cushion.interact on a client: PASS when sneaked at or when
            // something already sits on it, CONSUME (no swing) otherwise.
            let is_cushion = self.cushions.iter().any(|c| c.id as i32 == hit.id);
            let cushion_passes = is_cushion
                && (sneaking
                    || self.riding.is_some_and(|r| r as i32 == hit.id)
                    || self
                        .mobs
                        .iter()
                        .any(|m| m.extra.mount.is_some_and(|r| r as i32 == hit.id)));
            let passes =
                is_painting || frame_passes || boat_passes || cart_passes || cushion_passes;
            // Otherwise the main hand's turn takes it; there is no off hand turn.
            if response.is_none() && hand == Hand::Off && !passes {
                return false;
            }
            self.send(S::Interact(play::Interact {
                id: hit.id,
                hand: hand_id(hand),
                location: hit.location,
                secondary: sneaking,
            }));
            if passes {
                return false;
            }
            // Minecart.interact resets the passenger's easing baseline on
            // a successful boarding click, not on SetPassengers (commands
            // can mount a player without this interaction).
            if let Some(c) = self
                .carts
                .iter_mut()
                .find(|c| c.id as i32 == hit.id && c.kind == super::minecarts::CartKind::Rideable)
            {
                c.player_rotation_offset = c.rotation_offset;
            }
            // MinecartCommandBlock.interact on the client: the editor opens.
            if self.carts.iter().any(|c| {
                c.id as i32 == hit.id && c.kind == super::minecarts::CartKind::CommandBlock
            }) {
                self.open_cart_command_block(hit.id as u32);
            }
            // Minecart.interact boards the cart and answers CONSUME on the client,
            // which ends the click without an arm swing. AbstractBoat.interact
            // answers SUCCESS there, which swings. Cushion.interact answers
            // CONSUME there too.
            if remote_object_swings(response, is_rideable_cart || is_cushion) {
                self.with_hand(hand, Game::swing_arm);
            }
            return true;
        }
        if let Some((i, distance, location)) = self.mob_hit()
            && self.before_block(distance)
        {
            let id = self.mobs[i].id as i32;
            self.send(S::Interact(play::Interact {
                id,
                hand: hand_id(hand),
                location,
                secondary: sneaking,
            }));
            // Player.interactOn: another player passes, and so does a mob
            // the held item means nothing to: the item is used instead.
            let player = self
                .remote
                .as_ref()
                .is_some_and(|r| r.level_players.contains_key(&id));
            if !player && self.remote_mob_takes(i, hand) {
                self.remote_feed_particles(i, hand);
                // As for the objects above, only SUCCESS swings the hand: a
                // CONSUME ends the click without one.
                if self.remote_mob_swings(i, hand) {
                    self.with_hand(hand, Game::swing_arm);
                }
                return true;
            }
        }
        false
=======
        self.send_creative_slots_body()
>>>>>>> theirs
    }
    fn remote_synced(&mut self) {
        self.remote_synced_body()
    }
    pub(in crate::game) fn remote_synced_since(&mut self, before: &[Option<ItemStack>]) {
        self.remote_synced_since_body(before)
    }

    // mining.rs
    pub(super) fn stop_remote_mining(&mut self) -> bool {
        self.stop_remote_mining_body()
    }
    fn remote_mining(&mut self, start: bool) -> bool {
        self.remote_mining_body(start)
    }
    fn veto_mining_prediction(
        &self,
        pos: [i32; 3],
        face: [i32; 3],
        starting: bool,
        progress: f32,
    ) -> bool {
        self.veto_mining_prediction_body(pos, face, starting, progress)
    }
    fn remote_break_restricted(&self, pos: [i32; 3]) -> bool {
        self.remote_break_restricted_body(pos)
    }
    fn remote_use_block(&mut self, hand: Hand) -> bool {
        self.remote_use_block_body(hand)
    }
    pub(in crate::game) fn predict_item_on_block(
        &mut self,
        stack: &ItemStack,
        hit: &super::Hit,
    ) -> bool {
        self.predict_item_on_block_body(stack, hit)
    }
    fn predict_map_on_banner(&self, stack: &ItemStack, hit: &super::Hit) -> bool {
        self.predict_map_on_banner_body(stack, hit)
    }
    fn remote_bone_meal_succeeds(&mut self, hit: &super::Hit, stack: &ItemStack) -> bool {
        self.remote_bone_meal_succeeds_body(hit, stack)
    }
    fn predict_use(&mut self, (changes, sounds): &DryUse, seq: i32) {
        self.predict_use_body((changes, sounds), seq)
    }

    // use_prediction.rs
    pub(super) fn is_level_player(&self, id: i32) -> bool {
        self.is_level_player_body(id)
    }
    pub(super) fn block_takes_click(&mut self, hit: &super::Hit, held: Option<&ItemStack>) -> bool {
        self.block_takes_click_body(hit, held)
    }
    pub(super) fn block_swallows_click(
        &mut self,
        hit: &super::Hit,
        held: Option<&ItemStack>,
    ) -> bool {
        self.block_swallows_click_body(hit, held)
    }
    fn remote_block_reacts(
        &mut self,
        hit: &super::Hit,
        held: Option<&ItemStack>,
    ) -> (InteractionResult, DryUse) {
        self.remote_block_reacts_body(hit, held)
    }
    pub(super) fn remote_block_template(
        &mut self,
        hit: &super::Hit,
        held: Option<&ItemStack>,
        without_item: bool,
    ) -> (InteractionResult, DryUse) {
        self.remote_block_template_body(hit, held, without_item)
    }
    fn predict_place(&mut self, (pos, changes): &DryPlace, seq: i32) {
        self.predict_place_body((pos, changes), seq)
    }
    pub(super) fn predict_blocks(&mut self, changes: &DryChanges, seq: i32) {
        self.predict_blocks_body(changes, seq)
    }
    pub(super) fn remote_can_place(
        &mut self,
        hit: &super::Hit,
        stack: &ItemStack,
    ) -> Option<DryPlace> {
        self.remote_can_place_body(hit, stack)
    }
    fn remote_use_bucket(
        &mut self,
        stack: &ItemStack,
    ) -> (bool, Option<DryChanges>, Vec<DrySound>) {
        self.remote_use_bucket_body(stack)
    }
    fn use_finds_no_target(&self, held: &ItemStack) -> bool {
        self.use_finds_no_target_body(held)
    }
    fn remote_start_held_use(&mut self, held: &ItemStack) -> bool {
        self.remote_start_held_use_body(held)
    }
    fn remote_use_instrument(&mut self, held: &ItemStack) {
        self.remote_use_instrument_body(held)
    }
    fn held_until_released(&self, stack: &ItemStack) -> bool {
        self.held_until_released_body(stack)
    }
    fn patched_held_use(&self, stack: &ItemStack) -> bool {
        self.patched_held_use_body(stack)
    }
    fn remote_use_hands(&mut self, attempt: mod_api::InteractionAttempt) {
        self.remote_use_hands_body(attempt)
    }
    fn remote_tick_using(&mut self) {
        self.remote_tick_using_body()
    }
    fn remote_complete_using(&mut self) {
<<<<<<< ours
        let Some((slot, _)) = self.using.take() else {
            return;
        };
        let Some(stack) = self.stack_in(slot).cloned() else {
            return;
        };
        let Some(info) = self.items().get(&stack.item).cloned() else {
            return;
        };
        // Consumable.onConsume's emitParticlesAndSounds(16): the last crumbs
        // and bite, which the server plays for the others only.
        if let Some(consumable) = self.consumable(&stack) {
            self.emit_consume(&consumable, &stack, 16);
        }
        if let Some((nutrition, saturation)) = info.food
            && let Some(r) = self.remote.as_mut()
        {
            // FoodData.eat(nutrition, saturation), as finish_consuming does;
            // the next ClientboundSetHealth replaces it.
            let (_, food, sat) = &mut r.health;
            *food = (*food + nutrition as i32).clamp(0, 20);
            *sat = (*sat + saturation).clamp(0.0, *food as f32);
            self.apply_remote_health();
        }
        // ItemStack.finishUsingItem's applyAfterUseComponentSideEffects.
        self.apply_use_cooldown(&stack);
    }
    /// MouseHandler updates KeyMapping; handleKeybinds sends packets on a tick.
    pub(super) fn remote_mouse(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        use horizon_client::interaction_input::Button;
        self.java_bind_mouse(button, pressed);
        let input = match button {
            MouseButton::Left => {
                self.toggles
                    .button(2, pressed, false, self.settings.toggle_attack);
                self.left = self.toggles.is_down(2);
                Some(Button::Attack)
            }
            MouseButton::Right => {
                self.toggles
                    .button(3, pressed, false, self.settings.toggle_use);
                self.right = self.toggles.is_down(3);
                Some(Button::Use)
            }
            MouseButton::Middle => Some(Button::Pick),
            _ => None,
        };
        if pressed
            && let Some(button) = input
            && let Some(remote) = self.remote.as_mut()
        {
            remote.input.press(button);
        }
        Ok(())
||||||| base
        let Some((slot, _)) = self.using.take() else {
            return;
        };
        let Some(stack) = self.stack_in(slot).cloned() else {
            return;
        };
        let Some(info) = self.items().get(&stack.item).cloned() else {
            return;
        };
        // Consumable.onConsume's emitParticlesAndSounds(16): the last crumbs
        // and bite, which the server plays for the others only.
        if let Some(consumable) = self.consumable(&stack) {
            self.emit_consume(&consumable, &stack, 16);
        }
        if let Some((nutrition, saturation)) = info.food
            && let Some(r) = self.remote.as_mut()
        {
            // FoodData.eat(nutrition, saturation), as finish_consuming does;
            // the next ClientboundSetHealth replaces it.
            let (_, food, sat) = &mut r.health;
            *food = (*food + nutrition as i32).clamp(0, 20);
            *sat = (*sat + saturation).clamp(0.0, *food as f32);
            self.apply_remote_health();
        }
        // ItemStack.finishUsingItem's applyAfterUseComponentSideEffects.
        self.apply_use_cooldown(&stack);
    }
    /// MouseHandler updates KeyMapping; handleKeybinds sends packets on a tick.
    pub(super) fn remote_mouse(&mut self, button: MouseButton, pressed: bool) -> Result<()> {
        use horizon_client::interaction_input::Button;
        let input = match button {
            MouseButton::Left => {
                self.toggles
                    .button(2, pressed, false, self.settings.toggle_attack);
                self.left = self.toggles.is_down(2);
                Some(Button::Attack)
            }
            MouseButton::Right => {
                self.toggles
                    .button(3, pressed, false, self.settings.toggle_use);
                self.right = self.toggles.is_down(3);
                Some(Button::Use)
            }
            MouseButton::Middle => Some(Button::Pick),
            _ => None,
        };
        if pressed
            && let Some(button) = input
            && let Some(remote) = self.remote.as_mut()
        {
            remote.input.press(button);
        }
        Ok(())
=======
        self.remote_complete_using_body()
>>>>>>> theirs
    }

    // input.rs
    pub(super) fn request_remote_mining_stop(&mut self) {
        self.request_remote_mining_stop_body()
    }
    fn remote_handle_input(&mut self) {
        self.remote_handle_input_body()
    }
    fn remote_send_carried(&self) {
        self.remote_send_carried_body()
    }
<<<<<<< ours

    fn remote_start_attack(&mut self) -> bool {
        if self.clicks.miss_time > 0 || self.hands_busy() {
            return false;
        }
        if self.spectator() {
            // Minecraft.startAttack: a spectator's click on an entity asks
            // to watch through it (MultiPlayerGameMode.spectate); a click on
            // nothing still tells the server (spectatorNoAction).
            let target = self.spectator_click_target();
            self.send(S::SpectatorAction(play::SpectatorAction { target }));
            return true;
        }
        let spear = self
            .held()
            .and_then(|s| horizon_client::spear::Kinetic::of(&s.item));
        if spear.is_some() && self.cannot_attack_with_item(self.hand_strength.ticker, 0) {
            return false;
        }
        // InputEvent.InteractionKeyMappingTriggered, after the checks that stop
        // an attack: a cancel ends it there, swinging unless the mod said not to.
        if let Some(swing) = self.native_interaction(0, Hand::Main) {
            if swing {
                let animation = spear.map_or(SwingAnimation::DEFAULT, |k| k.attack_animation());
                self.swing_hand(Hand::Main, animation);
            }
            return false;
        }
        // A spear jabs (`MultiPlayerGameMode.piercingAttack`): syncs look, reports reach, stabs.
        if let Some(kinetic) = spear {
            return self.remote_spear_attack(kinetic);
        }
        let mut instant = false;
        // A ghast's fireball or a wind charge nearer than any mob: the
        // server turns it (game/deflect.rs).
        if let Some((i, distance)) = self.redirectable_hit()
            && self.mob_hit().is_none_or(|(_, d, _)| distance <= d)
            && self.before_block(distance)
            && self.creatures.projectiles[i].id != 0
        {
            self.send(S::Attack(play::Attack {
                id: self.creatures.projectiles[i].id as i32,
            }));
            self.reset_attack_strength();
        } else if let Some(hit) = self.remote_object_hit(true)
            && self.mob_hit().is_none_or(|(_, d, _)| hit.distance < d)
            && self.before_block(hit.distance)
        {
            // VehicleEntity.hurtServer, ItemFrame and ArmorStand hurtServer,
            // a painting broken or an EndCrystal hurtServer: the server decides.
            self.send(S::Attack(play::Attack { id: hit.id }));
            self.reset_attack_strength();
        } else if self.remote_attack_java_entity() {
            // A Java mod's entity: the server's mods attack it (remote/java.rs).
        } else if let Some((i, distance, _)) = self.mob_hit()
            && self.before_block(distance)
        {
            // Minecraft.startAttack: an item's attack_range must hold the hit.
            if self.held_range_allows(distance) {
                self.send(S::Attack(play::Attack {
                    id: self.mobs[i].id as i32,
                }));
                self.pvp_attacked(self.mobs[i].id as i32, distance);
                let (target, player) = (
                    self.mobs[i].id,
                    &*self.mobs[i].kind.id == "minecraft:player",
                );
                self.native_client_attack(target, player);
                self.remote_player_attack(i);
                self.reset_attack_strength();
            }
        } else if !self.entity_targeted()
            && let Some(hit) = self.pick()
        {
            self.remote_mining(true);
            instant = self
                .res
                .facts()
                .is_air(self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]));
        } else {
            if !self.creative {
                self.clicks.miss_time = 10;
            }
            self.reset_attack_strength();
        }
        self.swing_arm();
        self.send(S::Punch(Empty));
        instant
||||||| base

    fn remote_start_attack(&mut self) -> bool {
        if self.clicks.miss_time > 0 || self.hands_busy() {
            return false;
        }
        if self.spectator() {
            // Minecraft.startAttack: a spectator's click on an entity asks
            // to watch through it (MultiPlayerGameMode.spectate); a click on
            // nothing still tells the server (spectatorNoAction).
            let target = self.spectator_click_target();
            self.send(S::SpectatorAction(play::SpectatorAction { target }));
            return true;
        }
        let spear = self
            .held()
            .and_then(|s| horizon_client::spear::Kinetic::of(&s.item));
        if spear.is_some() && self.cannot_attack_with_item(self.hand_strength.ticker, 0) {
            return false;
        }
        // InputEvent.InteractionKeyMappingTriggered, after the checks that stop
        // an attack: a cancel ends it there, swinging unless the mod said not to.
        if let Some(swing) = self.native_interaction(0, Hand::Main) {
            if swing {
                let animation = spear.map_or(SwingAnimation::DEFAULT, |k| k.attack_animation());
                self.swing_hand(Hand::Main, animation);
            }
            return false;
        }
        // A spear jabs (`MultiPlayerGameMode.piercingAttack`): syncs look, reports reach, stabs.
        if let Some(kinetic) = spear {
            return self.remote_spear_attack(kinetic);
        }
        let mut instant = false;
        // A ghast's fireball or a wind charge nearer than any mob: the
        // server turns it (game/deflect.rs).
        if let Some((i, distance)) = self.redirectable_hit()
            && self.mob_hit().is_none_or(|(_, d, _)| distance <= d)
            && self.before_block(distance)
            && self.creatures.projectiles[i].id != 0
        {
            self.send(S::Attack(play::Attack {
                id: self.creatures.projectiles[i].id as i32,
            }));
            self.reset_attack_strength();
        } else if let Some(hit) = self.remote_object_hit(true)
            && self.mob_hit().is_none_or(|(_, d, _)| hit.distance < d)
            && self.before_block(hit.distance)
        {
            // VehicleEntity.hurtServer, ItemFrame and ArmorStand hurtServer,
            // a painting broken or an EndCrystal hurtServer: the server decides.
            self.send(S::Attack(play::Attack { id: hit.id }));
            self.reset_attack_strength();
        } else if let Some((i, distance, _)) = self.mob_hit()
            && self.before_block(distance)
        {
            // Minecraft.startAttack: an item's attack_range must hold the hit.
            if self.held_range_allows(distance) {
                self.send(S::Attack(play::Attack {
                    id: self.mobs[i].id as i32,
                }));
                self.pvp_attacked(self.mobs[i].id as i32, distance);
                let (target, player) = (
                    self.mobs[i].id,
                    &*self.mobs[i].kind.id == "minecraft:player",
                );
                self.native_client_attack(target, player);
                self.remote_player_attack(i);
                self.reset_attack_strength();
            }
        } else if !self.entity_targeted()
            && let Some(hit) = self.pick()
        {
            self.remote_mining(true);
            instant = self
                .res
                .facts()
                .is_air(self.world.block(hit.pos[0], hit.pos[1], hit.pos[2]));
        } else {
            if !self.creative {
                self.clicks.miss_time = 10;
            }
            self.reset_attack_strength();
        }
        self.swing_arm();
        self.send(S::Punch(Empty));
        instant
=======
    fn remote_sweep_event(&mut self, target: &str, living: bool) {
        self.remote_sweep_event_body(target, living)
>>>>>>> theirs
    }
    fn remote_attacked_object(&self, id: i32) -> Option<(String, bool)> {
        self.remote_attacked_object_body(id)
    }
    fn remote_player_attack(&mut self, mob: usize) {
        self.remote_player_attack_body(mob)
    }
    pub(in crate::game) fn own_latency(&self) -> Option<i32> {
        self.own_latency_body()
    }
    fn own_attribute(&mut self, id: &str) -> Option<f64> {
        self.own_attribute_body(id)
    }
    pub(super) fn remote_key(&mut self, key: KeyCode) -> Result<bool> {
        self.remote_key_body(key)
    }
    pub(super) fn release_use_for_keybind_screen(&mut self) {
        self.release_use_for_keybind_screen_body()
    }
    fn release_remote_use(&mut self) {
        self.release_remote_use_body()
    }
    pub(super) fn send_signed_chat_input(&mut self, message: &str) {
        self.send_signed_chat_input_body(message)
    }
    pub(super) fn send_chat_from_screen(&mut self, message: &str) {
        self.send_chat_from_screen_body(message)
    }
    pub(super) fn remote_drop(&mut self, all: bool) {
        self.remote_drop_body(all)
    }
    pub(super) fn remote_swap_hands(&mut self) {
        self.remote_swap_hands_body()
    }
    fn remote_hand_slot_set(&mut self, slot: usize, changed: bool) {
        self.remote_hand_slot_set_body(slot, changed)
    }
    pub(super) fn remote_click(&mut self, right: bool) {
        self.remote_click_body(right)
    }
    fn remote_menu_state(&mut self, container: i32, state: i32) {
        self.remote_menu_state_body(container, state)
    }

    // menus.rs
    pub(super) fn remote_menu(&self) -> (i32, String) {
        self.remote_menu_body()
    }
    pub(super) fn remote_menu_ops(&mut self, ops: &[(Option<usize>, i32, Input)]) -> bool {
        self.remote_menu_ops_body(ops)
    }
    pub(super) fn remote_quick_move_unpredicted(&mut self, internal: usize) -> bool {
        self.remote_quick_move_unpredicted_body(internal)
    }
    pub(super) fn remote_quick_move_all(&mut self, slot: usize, moved: &ItemStack) {
        self.remote_quick_move_all_body(slot, moved)
    }
    fn send_clicks(&mut self, container: i32, clicks: Vec<Predicted>) {
        self.send_clicks_body(container, clicks)
    }
    pub(super) fn remote_storage_open(&self) -> bool {
        self.remote_storage_open_body()
    }
    pub(super) fn remote_throw(&mut self, right: bool, input: horizon_client::menu::Input) {
        self.remote_throw_body(right, input)
    }
    pub(super) fn remote_creative_drop(&mut self, right: bool) {
        self.remote_creative_drop_body(right)
    }
    pub(super) fn remote_creative_throw(&mut self, thrown: ItemStack) {
        self.remote_creative_throw_body(thrown)
    }
    pub(super) fn remote_close_container(&mut self) {
        self.remote_close_container_body()
    }
    pub(super) fn remote_button_click(&self, button: i32) -> bool {
        self.remote_button_click_body(button)
    }
    fn remote_reject_container(&mut self, container: i32) {
        self.remote_reject_container_body(container)
    }
    fn remote_close_local(&mut self) {
        self.remote_close_local_body()
    }
    fn remote_open_merchant(
        &mut self,
        container: i32,
        title: String,
        text: horizon_client::text::Text,
    ) {
        self.remote_open_merchant_body(container, title, text)
    }
    fn remote_merchant_offers(&mut self, p: play::MerchantOffers) {
        self.remote_merchant_offers_body(p)
    }
    fn remote_open_screen(&mut self, p: play::OpenScreen) {
        self.remote_open_screen_body(p)
    }
    fn remote_open_mount(&mut self, p: play::MountScreenOpen) {
        self.remote_open_mount_body(p)
    }

    // screens.rs
    pub(super) fn remote_menu_type(&self) -> Option<&str> {
        self.remote_menu_type_body()
    }
    pub(super) fn remote_menu_title(&self) -> Option<horizon_client::text::Text> {
        self.remote_menu_title_body()
    }
    pub(super) fn refresh_remote_language(&mut self) {
        self.refresh_remote_language_body()
    }
    pub(super) fn remote_chest_menu(&self) -> Option<(usize, horizon_client::text::Text)> {
        self.remote_chest_menu_body()
    }
    pub(super) fn remote_stop_sleeping(&mut self) {
        self.remote_stop_sleeping_body()
    }
    pub(super) fn remote_respawn(&mut self) {
        self.remote_respawn_body()
    }
    pub(super) fn remote_screen_up(&self) -> bool {
        self.remote_screen_up_body()
    }
    pub(super) fn remote_overlay(&mut self, batch: &mut HudBatch) -> bool {
        self.remote_overlay_body(batch)
    }
    pub(super) fn remote_loading(&self) -> bool {
        self.remote_loading_body()
    }
    fn reconfigure_layout(&self, (w, h): (f32, f32)) -> ((f32, f32), (f32, f32)) {
        self.reconfigure_layout_body((w, h))
    }
    fn draw_reconfiguring(&self, batch: &mut HudBatch, since: u64) {
        self.draw_reconfiguring_body(batch, since)
    }
    fn connect_cancel(&self, (w, h): (f32, f32)) -> Option<(f32, f32)> {
        self.connect_cancel_body((w, h))
    }
    pub(super) fn connect_screen_click(&mut self) -> Result<()> {
        self.connect_screen_click_body()
    }
    pub(super) fn remote_hud(&self, batch: &mut HudBatch) {
        self.remote_hud_body(batch)
    }
    pub(super) fn remote_glowing(&self, id: u32) -> Option<bool> {
        self.remote_glowing_body(id)
    }
    pub(super) fn remote_player_name(&self, id: u32) -> Option<&str> {
        self.remote_player_name_body(id)
    }
    pub(super) fn remote_player_uuid(&self, id: u32) -> Option<u128> {
        self.remote_player_uuid_body(id)
    }
    pub(super) fn remote_player_names(&self) -> Vec<(u32, String)> {
        self.remote_player_names_body()
    }
    fn remote_clock_tick(&mut self) {
        self.remote_clock_tick_body()
    }
    pub(super) fn remote_status(&self) -> Option<serde_json::Value> {
        self.remote_status_body()
    }
}

/// `Entity.lookAt`'s rotation from `from` towards `to`, as (yaw, pitch).
fn look_at_rotation(from: [f64; 3], to: [f64; 3]) -> (f32, f32) {
    use horizon_core::geometry::trig::atan2;
    let (dx, dy, dz) = (to[0] - from[0], to[1] - from[1], to[2] - from[2]);
    let horizontal = (dx * dx + dz * dz).sqrt();
    // `* 57.2957763671875` (`Mth.RAD_TO_DEG` widened) on the double.
    let degrees = |r: f64| r * f64::from(horizon_core::geometry::trig::RAD_TO_DEG);
    let pitch = wrap_degrees((-degrees(atan2(dy, horizontal))) as f32);
    let yaw = wrap_degrees(degrees(atan2(dz, dx)) as f32 - 90.0);
    (yaw, pitch)
}

/// `PositionMoveRotation.calculateAbsolute`'s pitch: the packet's pitch,
/// added to `source` when it is relative, clamped to +-90.
fn position_packet_pitch(source: f32, change: f32, relative: bool) -> f32 {
    let offset = if relative { source } else { 0.0 };
    (offset + change).clamp(-90.0, 90.0)
}

/// `Mth.wrapDegrees`.
fn wrap_degrees(a: f32) -> f32 {
    let a = a % 360.0;
    if a >= 180.0 {
        a - 360.0
    } else if a < -180.0 {
        a + 360.0
    } else {
        a
    }
}

/// `LivingEntity.tick`'s body target and `tickHeadTurn` for another player:
/// the body turns toward where it walks (backwards when walking back), or to
/// the look while the arm swings, and stays within 50 degrees of the look.
fn remote_player_head_turn(m: &mut Mob) {
    let (dx, dz) = (m.position.x - m.previous.x, m.position.z - m.previous.z);
    let mut target = m.body_yaw;
    if (dx * dx + dz * dz) as f32 > 0.002_500_000_2 {
        let walk = (dz.atan2(dx) as f32).to_degrees() - 90.0;
        let facing = (wrap_degrees(m.yaw) - walk).abs();
        target = if 95.0 < facing && facing < 265.0 {
            walk - 180.0
        } else {
            walk
        };
    }
    if m.anim.attack_anim > 0.0 {
        target = m.yaw;
    }
    m.body_yaw += wrap_degrees(target - m.body_yaw) * 0.3;
    let head = wrap_degrees(m.yaw - m.body_yaw);
    if head.abs() > 50.0 {
        m.body_yaw += head - head.signum() * 50.0;
    }
}

/// A click sent to a server: its slot, button and input, the slots it
/// changed and the carried stack after it.
type Predicted = (
    i16,
    i32,
    Input,
    Vec<(i16, Option<ItemStack>)>,
    Option<ItemStack>,
);

/// How many stacks the creative screen keeps in step with the server (`Remote::synced`).
const CREATIVE_SYNCED: usize = 41;

/// The player's stacks in `Remote::synced` order: the 36 inventory slots, the off hand, then
/// the armour from the head down. `CreativeInventoryListener.slotChanged` sends every
/// InventoryMenu slot the creative screen changes, the armour slots too.
fn creative_sync_stacks(
    inventory: &[Option<ItemStack>],
    offhand: &Option<ItemStack>,
    armor: &[Option<ItemStack>; 4],
) -> Vec<Option<ItemStack>> {
    let mut stacks: Vec<Option<ItemStack>> = inventory.iter().take(36).cloned().collect();
    stacks.resize(36, None);
    stacks.push(offhand.clone());
    stacks.extend(armor.iter().cloned());
    stacks
}

/// Entries of `creative_sync_stacks` that went from `before` to `now` are synced as `now`.
fn mark_synced(
    synced: &mut [Option<ItemStack>],
    before: &[Option<ItemStack>],
    now: Vec<Option<ItemStack>>,
) {
    for (i, stack) in now.into_iter().enumerate() {
        if before.get(i) != Some(&stack)
            && let Some(s) = synced.get_mut(i)
        {
            *s = stack;
        }
    }
}

/// InventoryMenu's slot for entry `i` of `creative_sync_stacks`: the hotbar is 36..=44, the
/// main inventory 9..=35, the off hand 45 and the armour 5..=8 (head first).
fn creative_sync_slot(i: usize) -> i16 {
    (match i {
        0..9 => 36 + i,
        36 => 45,
        37.. => 5 + (i - 37),
        _ => i,
    }) as i16
}

/// `MultiPlayerGameMode.handleContainerInput` for each click on the engine's
/// copy of the menu: the click with the slots it changed and the carried
/// stack after it, and whether a click dropped a stack. Every drop of
/// `AbstractContainerMenu.doClick` (a THROW, a click outside the window, a
/// swap or a result move with no room left) is `Prediction.PREDICTED`.
/// None when the engine refuses a click.
fn predict_clicks(
    m: &mut horizon_client::menu::Menu,
    env: &horizon_client::menu::Env,
    clicks: &[(i16, i32, Input)],
) -> Option<(Vec<Predicted>, bool)> {
    let mut predicted = Vec::with_capacity(clicks.len());
    let mut dropped = false;
    for &(slot, button, input) in clicks {
        let before = menu_items(m);
        let thrown = m.dropped.len();
        m.click(env, i32::from(slot), button, input).ok()?;
        dropped |= m.dropped.len() > thrown;
        let changed = changed_slots(&before, m);
        predicted.push((slot, button, input, changed, m.carried.clone()));
    }
    Some((predicted, dropped))
}

fn menu_items(m: &horizon_client::menu::Menu) -> Vec<Option<ItemStack>> {
    (0..m.slot_count()).map(|i| m.slot(i).cloned()).collect()
}

/// The slots whose stack is no longer `before`'s (`ItemStack.matches`).
/// `MultiPlayerGameMode.handleContainerInput`'s changed slots, in the order the packet writes them.
fn changed_slots(
    before: &[Option<ItemStack>],
    m: &horizon_client::menu::Menu,
) -> Vec<(i16, Option<ItemStack>)> {
    let changed: Vec<i16> = (0..m.slot_count())
        .filter(|&i| m.slot(i) != before.get(i).and_then(Option::as_ref))
        .map(|i| i as i16)
        .collect();
    open_hash_order(&changed)
        .into_iter()
        .map(|i| (i, m.slot(i as usize).cloned()))
        .collect()
}

/// The order a new fastutil `Int2ObjectOpenHashMap` iterates `keys` put in this order: vanilla
/// collects a click's changed slots in one and writes them as it iterates. The key 0 comes first,
/// then the table from its last position down; a key sits at `HashCommon.mix(key) & mask` or the
/// next free position. The table starts at 32 (16 expected, load factor 0.75) and doubles when an
/// insert finds it holding 24 (`size++ >= maxFill`), re-inserting from its last position down.
fn open_hash_order(keys: &[i16]) -> Vec<i16> {
    fn mix(key: i16) -> usize {
        let h = i32::from(key).wrapping_mul(0x9E37_79B9_u32 as i32);
        (h ^ ((h as u32) >> 16) as i32) as u32 as usize
    }
    fn put(table: &mut [Option<i16>], key: i16) {
        let mask = table.len() - 1;
        let mut pos = mix(key) & mask;
        while table[pos].is_some() {
            pos = (pos + 1) & mask;
        }
        table[pos] = Some(key);
    }
    // HashCommon.maxFill(n, 0.75): min(ceil(n * 0.75), n - 1).
    let max_fill = |n: usize| (n * 3).div_ceil(4).min(n - 1);
    let mut table = vec![None; 32];
    let (mut zero, mut size) = (false, 0);
    for &key in keys {
        if key == 0 {
            zero = true;
        } else {
            put(&mut table, key);
        }
        size += 1;
        // `size++ >= maxFill`: the size before this key.
        if size > max_fill(table.len()) {
            // HashCommon.arraySize(size + 1, 0.75): the power of two holding size + 1 at that load.
            let n = ((size + 1) * 4).div_ceil(3).next_power_of_two();
            let mut grown = vec![None; n];
            for &key in table.iter().rev().flatten() {
                put(&mut grown, key);
            }
            table = grown;
        }
    }
    let rest = table.iter().rev().flatten().copied();
    zero.then_some(0).into_iter().chain(rest).collect()
}

/// ServerboundContainerClickPacket's `ContainerInput` ordinal.
pub(super) fn input_ordinal(input: Input) -> i32 {
    match input {
        Input::Pickup => 0,
        Input::QuickMove => 1,
        Input::Swap => 2,
        Input::Clone => 3,
        Input::Throw => 4,
        Input::QuickCraft => 5,
        Input::PickupAll => 6,
    }
}

mod dry_level;
use dry_level::DryLevel;

type DrySound = (
    horizon_core::position::BlockPos,
    std::sync::Arc<str>,
    f32,
    f32,
);
/// The blocks a use on the dry level set.
type DryChanges = Vec<(horizon_core::position::BlockPos, BlockStateId)>;
/// What a use on the dry level changed, and the sounds it played.
pub(super) type DryUse = (DryChanges, Vec<DrySound>);
/// Where `BlockItem.place` on the dry level placed, and the blocks it set.
type DryPlace = (horizon_core::position::BlockPos, DryChanges);

/// The blocks a guest's hand flips that its use predicts (and hears at once): a wooden or
/// copper door, trapdoor or fence gate, a lever, a button or a cauldron, whose fill level
/// changes as the bucket, bottle or dye lands. An iron door or trapdoor has no hand use.
pub(super) fn hand_operated(name: &str) -> bool {
    let name = name.trim_start_matches("minecraft:");
    ((name.ends_with("_door") || name.ends_with("_trapdoor")) && !name.starts_with("iron_"))
        || name.ends_with("_fence_gate")
        || name.ends_with("_button")
        || name == "lever"
        || name.ends_with("cauldron")
}

/// `InteractionHand` on the wire.
fn hand_id(hand: Hand) -> i32 {
    match hand {
        Hand::Main => 0,
        Hand::Off => 1,
    }
}

/// Whether `Item.use` of `info` ends `Minecraft.startUseItem`'s click, so the
/// off hand gets no turn. `FireworkRocketItem.use` answers SUCCESS while the
/// player glides (the rocket boosts them) and PASS otherwise.
fn use_ends_click(info: &horizon_client::items::ItemInfo, fall_flying: bool) -> bool {
    item_has_use(info) || (fall_flying && &*info.id == "minecraft:firework_rocket")
}

/// Whether `Item.use` of this item answers SUCCESS on this client, which
/// swings (`MultiPlayerGameMode.useItem`'s `result.shouldSwing()`), rather
/// than CONSUME, which only ends the click: a bow, crossbow, trident or
/// spear starting its charge, `ItemUtils.startUsingInstantly`'s food, drink,
/// spyglass, shield and goat horn, and the ender eye's SUCCESS_SERVER (the
/// server's swing source, not this client's).
fn remote_use_swings(held: &ItemStack, consumable: bool) -> bool {
    let id = &*held.item;
    !matches!(
        id,
        "minecraft:bow"
            | "minecraft:crossbow"
            | "minecraft:trident"
            | "minecraft:spyglass"
            | "minecraft:goat_horn"
            | "minecraft:ender_eye"
            | "minecraft:brush"
    ) && !consumable
        && horizon_client::spear::Kinetic::of(id).is_none()
}

/// Whether an object interaction on the client predicts a hand swing (`InteractionResult.SUCCESS`).
/// Boarding a minecart answers CONSUME and does not swing.
fn remote_object_swings(response: Option<bool>, is_boarding: bool) -> bool {
    response.unwrap_or(!is_boarding)
}

/// Whether right-clicking an item frame passes (`InteractionResult.PASS`).
/// A fixed frame or an empty frame when clicking with an empty hand passes.
fn item_frame_interact_passes(fixed: bool, frame_has_item: bool, hand_has_item: bool) -> bool {
    fixed || (!frame_has_item && !hand_has_item)
}

/// Whether right-clicking a boat passes (`InteractionResult.PASS`) on a client.
/// `AbstractBoat.interact` passes when sneaking or out of control (60 ticks), and
/// answers SUCCESS otherwise without checking for room (only the server's
/// `startRiding` does). `AbstractChestBoat.interact` opens the chest instead of
/// passing, unless it is out of control, not sneaked at and has room.
fn boat_interact_passes(chest: bool, sneaking: bool, out_of_control: bool, has_room: bool) -> bool {
    if chest {
        !sneaking && out_of_control && has_room
    } else {
        sneaking || out_of_control
    }
}

/// Whether `Item.use` usually acts on this item (eats, drinks, equips, raises,
/// throws, shoots, fills or places), which ends `Minecraft.startUseItem` at
/// its hand. Tools and materials pass, and the off hand gets its turn. The
/// server decides; this is the client's guess of whether to go on. A carrot or warped fungus
/// on a stick is not here: `FoodOnAStickItem.use` passes on a client, and only the server's
/// answer to a boost that can run swings the hand.
pub(super) fn item_has_use(info: &horizon_client::items::ItemInfo) -> bool {
    let id = info.id.trim_start_matches("minecraft:");
    id.ends_with("_spear")
        || info.is("minecraft:fishing_rod") // a content mod's rod too
        || info.consumable.is_some()
        || (info.equip_slot.is_some() && info.swappable)
        || info.blocks_attacks.is_some()
        || horizon_client::items::bucket_contents(&info.id).is_some()
        || id.ends_with("_boat")
        || id.ends_with("_raft")
        || id.ends_with("_bundle")
        || matches!(
            id,
            "bow"
                | "crossbow"
                | "trident"
                | "spyglass"
                | "snowball"
                | "egg"
                | "blue_egg"
                | "brown_egg"
                | "ender_pearl"
                | "ender_eye"
                | "wind_charge"
                | "splash_potion"
                | "lingering_potion"
                | "experience_bottle"
                | "bucket"
                | "glass_bottle"
                | "map"
                | "writable_book"
                | "written_book"
                | "goat_horn"
                | "bundle"
                | "brush"
                | "knowledge_book"
        )
}

/// The view distance a client asks for: its render distance, which Horizon
/// lets go past vanilla's 32 (a vanilla server still caps it at its own).
fn client_view_distance(render_distance: i32) -> i8 {
    render_distance.clamp(2, i32::from(i8::MAX)) as i8
}

/// `ClientPacketListener.handleBlockEntityData` loads the tag into the existing
/// block entity, so the state the client builds itself stays: copied from `old`
/// into `new`, the block entity decoded from the tag.
fn keep_client_state(old: Option<&BlockEntity>, new: &mut BlockEntity) {
    match (old, new) {
        // The turn of a vault's item and a trial spawner's mob (VaultClientData).
        (Some(BlockEntity::Vault(old)), BlockEntity::Vault(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        (Some(BlockEntity::TrialSpawner(old)), BlockEntity::TrialSpawner(new)) => {
            new.spin = old.spin;
            new.o_spin = old.o_spin;
        }
        // A beacon's beam (`BeaconBlockEntity.loadAdditional` reads only the
        // powers): a fresh one draws nothing until the next tick walks it again.
        (Some(BlockEntity::Beacon(old)), BlockEntity::Beacon(new)) => {
            new.sections.clone_from(&old.sections);
            new.checking.clone_from(&old.checking);
            (new.last_check_y, new.levels) = (old.last_check_y, old.levels);
        }
        _ => {}
    }
}

/// `LocalPlayer.onSyncedDataUpdated(DATA_LIVING_ENTITY_FLAGS)`: the server's
/// flags (1 using, 2 the off hand) start a use the client is not running
/// (`startUsingItem`, which needs an item in that hand), and stop one it is
/// (`stopUsingItem`). `using` is the slot in use and its ticks.
fn synced_local_use(
    using: Option<(usize, u32)>,
    flags: i8,
    selected: usize,
    has_item: impl Fn(usize) -> bool,
) -> Option<(usize, u32)> {
    let server_using = flags & 1 != 0;
    match using {
        None if server_using => {
            let slot = if flags & 2 != 0 {
                super::shield::OFFHAND_SLOT
            } else {
                selected
            };
            has_item(slot).then_some((slot, 0))
        }
        Some(_) if !server_using => None,
        using => using,
    }
}

/// The game as the tick's input sees it.
struct RemoteInputWorld<'g, 'a>(&'g mut Game<'a>);

struct RemoteUseWorld<'g, 'a>(&'g mut Game<'a>, mod_api::InteractionAttempt);

impl horizon_client::interaction_input::TargetUseWorld for RemoteUseWorld<'_, '_> {
    fn use_entity(&mut self) -> bool {
        self.0.remote_use_entity(Hand::Main)
    }
    fn use_block(&mut self) -> bool {
        self.0.remote_use_block(Hand::Main)
    }
    fn use_item(&mut self) {
        self.0.remote_use_hands(self.1);
    }
}

impl horizon_client::interaction_input::InteractionWorld for RemoteInputWorld<'_, '_> {
    fn pick_hit_result(&mut self) {
        self.0.pick_tick_targets();
    }
    fn release_hit_result(&mut self) {
        self.0.release_tick_targets();
    }
    fn using_item(&self) -> bool {
        self.0.using.is_some()
    }
    fn release_using(&mut self) {
        if let Some((slot, ticks)) = self.0.using
            && self.0.mod_use_tick(slot, ticks, true)
        {
            return;
        }
        self.0.release_remote_use();
    }
    fn start_attack(&mut self) -> bool {
        self.0.remote_start_attack()
    }
    fn start_use(&mut self, held: bool) {
        let attempt = if held {
            mod_api::InteractionAttempt::Held
        } else {
            mod_api::InteractionAttempt::Press
        };
        self.0.remote_use(attempt);
    }
    fn pick(&mut self) {
        // Minecraft.pickBlockOrEntity: an entity before the block is asked
        // for (`handlePickItemFromEntity`), else the block.
        if let Some(entity) = self.0.pick_entity_hit() {
            if let Some(id) = entity {
                if self
                    .0
                    .native_interaction(
                        InteractionKey::PickBlock,
                        Hand::Main,
                        mod_api::InteractionAttempt::Press,
                    )
                    .canceled
                {
                    return;
                }
                self.0.send(S::PickItemFromEntity(play::PickItemFromEntity {
                    id,
                    include_data: self.0.has_control_down(),
                }));
            }
        } else if let Some(hit) = self.0.pick() {
            if self
                .0
                .native_interaction(
                    InteractionKey::PickBlock,
                    Hand::Main,
                    mod_api::InteractionAttempt::Press,
                )
                .canceled
            {
                return;
            }
            self.0.send(S::PickItemFromBlock(play::PickItemFromBlock {
                pos: hit.pos,
                include_data: self.0.has_control_down(),
            }));
        }
    }
    fn use_delay(&self) -> u32 {
        self.0.use_delay
    }
    fn continue_attack(&mut self, down: bool) {
        self.0.remote_continue_attack(down);
    }
}

#[derive(Default)]
pub(super) struct ClientMenu {
    current: Option<(i32, String)>,
    pub(super) generation: u64,
}

impl ClientMenu {
    pub(super) fn set(&mut self, current: Option<(i32, String)>) {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("menu generation exhausted");
        self.current = current;
    }

    pub(super) fn as_ref(&self) -> Option<&(i32, String)> {
        self.current.as_ref()
    }
}

impl Remote {
    pub(super) fn set_menu_title(&mut self, title: Option<horizon_client::text::Text>) {
        self.menu_title_snapshot = title.try_into_api().ok().flatten().map(Arc::new);
        self.menu_title = title;
    }

    pub(super) fn menu_title_snapshot(&self) -> Option<&mod_api::Text> {
        self.menu_title_snapshot.as_deref()
    }

    pub(super) fn shared_menu_title_snapshot(&self) -> Option<Arc<mod_api::Text>> {
        self.menu_title_snapshot.clone()
    }

    /// Resolve registry keys only when registries or the dimension change.
    fn resolve_clocks(&mut self) {
        self.overworld_clock = self
            .registries
            .id("minecraft:world_clock", "minecraft:overworld");
        self.default_clock = self
            .dimension
            .default_clock
            .as_deref()
            .and_then(|name| self.registries.id("minecraft:world_clock", name));
        for id in [self.overworld_clock, self.default_clock]
            .into_iter()
            .flatten()
        {
            self.clocks.entry(id).or_insert(play::ClockState {
                clock: id,
                total_ticks: 0,
                partial_tick: 0.0,
                rate: 1.0,
            });
        }
    }

    fn clock_time(&self, id: Option<i32>) -> i64 {
        id.and_then(|id| self.clocks.get(&id))
            .map_or(0, |c| c.total_ticks)
    }

    pub(super) fn overworld_clock_time(&self) -> i64 {
        self.clock_time(self.overworld_clock)
    }

    /// `Minecraft.level != null`: from `handleLogin` until the next
    /// `handleConfigurationStart` (`clearClientLevel`). Minecraft.tick sends
    /// CLIENT_TICK_END only then, so a client waiting for the server's Login
    /// (a busy server takes seconds to place a new player) sends nothing.
    pub(in crate::game) fn has_level(&self) -> bool {
        client_has_level(self.logged_in, self.reconfiguring)
    }
    /// An entity the server added to this client level: its type
    /// (`minecraft:text_display`) and its position as last sent.
    pub(super) fn tracked_entity(&self, id: i32) -> Option<(&str, [f64; 3])> {
        self.entities
            .get(&id)
            .map(|t| (t.kind.as_str(), t.position))
    }
    /// The profile of the player entity `id`: this window's own player, or
    /// another one by the UUID it was added with and its profile name.
    pub(super) fn level_player(&self, id: i32) -> Option<(u128, Option<&str>)> {
        if id == self.entity_id {
            return Some((self.profile, Some(self.profile_name.as_str())));
        }
        let uuid = *self.level_players.get(&id)?;
        Some((uuid, self.level_player_names.get(&id).map(String::as_str)))
    }
    /// `ClientPacketListener.getSeenPlayers`: the player-info entries of the
    /// players this connection has seen, without this client's own profile.
    pub(super) fn seen_players_other_than_self(&self) -> usize {
        self.seen_players
            .iter()
            .filter(|id| **id != self.profile)
            .count()
    }
    /// `PlayerInfo.getMessageValidator().updateAndValidate(message)` for `message.sender`.
    fn validate_chat(
        &mut self,
        message: &secure_chat::Incoming,
        now: i64,
    ) -> Option<secure_chat::Validated> {
        let sender = message.sender;
        if let Some(validator) = self.chat_validators.get_mut(&sender) {
            return validator.update_and_validate(message, now);
        }
        let (mut validator, settled) = self.new_chat_validator(sender);
        let validated = validator.update_and_validate(message, now);
        // While the services' keys are still on their way the validator is a stopgap, not kept.
        if settled {
            self.chat_validators.insert(sender, validator);
        }
        validated
    }
    /// `ClientPacketListener.initializeChatSession` for `sender`'s entry: a session the services'
    /// keys vouch for makes a keyed validator, anything else the fallback of
    /// `enforcesSecureChat()` (`canValidateProfileKeys() && serverEnforcesSecureChat`). `false`
    /// while the keys are not known yet.
    fn new_chat_validator(&self, sender: u128) -> (secure_chat::MessageValidator, bool) {
        let keys = self.services_keys.lock().unwrap_or_else(|e| e.into_inner());
        let (can_validate, settled) = match &*keys {
            ServicesKeys::Ready(_) => (true, true),
            ServicesKeys::Missing => (false, true),
            ServicesKeys::Pending => (false, false),
        };
        let fallback =
            secure_chat::MessageValidator::fallback(can_validate && self.enforces_secure_chat);
        let (ServicesKeys::Ready(keys), Some(Some((id, expires_at, key, signature)))) =
            (&*keys, self.chat_sessions.get(&sender))
        else {
            if matches!(&*keys, ServicesKeys::Missing)
                && self.chat_sessions.get(&sender).is_some_and(Option::is_some)
            {
                log::warn!(
                    "Ignoring chat session from {sender:032x} due to missing Services public key"
                );
            }
            return (fallback, settled);
        };
        let profile_key = secure_chat::ProfileKey {
            expires_at: *expires_at,
            key: key.clone(),
            signature: signature.clone(),
        };
        match secure_chat::ChatSession::validate(*id, profile_key, keys, sender) {
            Ok(session) => (secure_chat::MessageValidator::keyed(session), true),
            Err(why) => {
                log::error!("Failed to validate profile key for player {sender:032x}: {why}");
                (fallback, true)
            }
        }
    }
    /// How far the server sends chunks: its view distance and the ring
    /// around (`ChunkTrackingView`), at least two.
    pub(super) fn sent_radius(&self) -> i32 {
        self.view_distance.max(2).saturating_add(1)
    }
    /// A player entity's profile name: the one it was created with, else its
    /// player-list entry's (an entity the list named after it spawned).
    fn level_player_name(&self, id: i32, uuid: &u128) -> Option<&str> {
        self.level_player_names
            .get(&id)
            .or_else(|| self.players.get(uuid))
            .map(String::as_str)
    }
    /// Listed UUIDs always have a tab entry; removal clears both together.
    pub(super) fn listed_player_count(&self) -> usize {
        self.listed.len()
    }
    /// The local player's profile UUID.
    pub(super) fn profile(&self) -> u128 {
        self.profile
    }
    /// Another player's profile UUID, by their entity id (None for the local player).
    pub(super) fn player_uuid(&self, id: u32) -> Option<u128> {
        let uuid = *self.level_players.get(&(id as i32))?;
        (uuid != self.profile).then_some(uuid)
    }
    /// The entity id of a player in the level, by profile UUID.
    pub(super) fn player_entity(&self, uuid: u128) -> Option<u32> {
        self.level_players
            .iter()
            .find(|(_, u)| **u == uuid)
            .map(|(id, _)| *id as u32)
    }
    /// The kind and last sent position of a tracked server entity.
    pub(super) fn tracked(&self, id: i32) -> Option<(&str, [f64; 3])> {
        self.entities.get(&id).map(|t| (&*t.kind, t.position))
    }
}

fn object_skin_changed(published: &HashMap<String, String>, key: &str, skin: &str) -> bool {
    published.get(key).is_none_or(|previous| previous != skin)
}

/// The textures to resolve skins from: every list entry's, and for a player
/// entity whose entry is gone the profile it kept. `kept` first takes, for each
/// player entity without one, its entry's textures (`AbstractClientPlayer.getPlayerInfo`
/// looks the PlayerInfo up once and holds it, removed from the list or not).
fn skin_entries(
    level_players: &HashMap<i32, u128>,
    listed: &HashMap<u128, horizon_client::skins::Textures>,
    kept: &mut HashMap<i32, horizon_client::skins::Textures>,
) -> Vec<(u128, horizon_client::skins::Textures)> {
    for (id, uuid) in level_players {
        if !kept.contains_key(id)
            && let Some(textures) = listed.get(uuid)
        {
            kept.insert(*id, textures.clone());
        }
    }
    let mut entries: Vec<_> = listed.iter().map(|(u, t)| (*u, t.clone())).collect();
    for (id, textures) in kept.iter() {
        if let Some(uuid) = level_players.get(id)
            && !listed.contains_key(uuid)
            && !entries.iter().any(|(u, _)| u == uuid)
        {
            entries.push((*uuid, textures.clone()));
        }
    }
    entries
}

/// `AttributeInstance.calculateValue` before the attribute's range: the base,
/// `ADD_VALUE`s, then `ADD_MULTIPLIED_BASE`s of that, then each
/// `ADD_MULTIPLIED_TOTAL`.
fn snapshot_value(s: &play::AttributeSnapshot) -> f64 {
    let amounts = |op: i32| {
        s.modifiers
            .iter()
            .filter(move |m| m.operation == op)
            .map(|m| m.amount)
    };
    let base = s.base + amounts(0).sum::<f64>();
    let mut value = base + amounts(1).map(|a| base * a).sum::<f64>();
    for amount in amounts(2) {
        value *= 1.0 + amount;
    }
    value
}

/// `Util.getMillis` for the level load tracker's timeouts.
fn client_millis() -> u64 {
    static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    ORIGIN
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// What `ChatScreen.handleChatInput` sends of typed chat once `ClientChatEvent` (`ask`, with
/// the outgoing message and the cancel flag) had its say: nothing when canceled or emptied.
fn chat_after_event(message: &str, ask: impl FnOnce(&mut String, &mut bool)) -> Option<String> {
    let mut outgoing = message.to_owned();
    let mut cancel = false;
    ask(&mut outgoing, &mut cancel);
    (!cancel && !outgoing.is_empty()).then_some(outgoing)
}
/// `Entity.isOnFire` for the window's player: while the server's flag is set
/// the predicted tick's countdown never takes `fire_ticks` below 1.
fn hold_server_fire(fire_ticks: i32, server_on_fire: bool) -> i32 {
    if server_on_fire {
        fire_ticks.max(1)
    } else {
        fire_ticks
    }
}

/// `BlockStatePredictionHandler.ServerVerifiedState`: the sequence of the action
/// that last changed the block, the state the server last said it has, and where
/// the player stood when this client first predicted a change there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Prediction {
    pub sequence: i32,
    pub state: i32,
    pub player_pos: [f64; 3],
}

/// `Entity.isColliding`: whether the collision `shape` of a block at `pos`
/// meets `entity`'s box. Shapes are block-local, so they move onto the block
/// first (`VoxelShape.move`), and any box that overlaps is a collision
/// (`Shapes.joinIsNotEmpty` with `BooleanOp.AND`).
pub(super) fn shape_meets(
    shape: &horizon_core::geometry::VoxelShape,
    pos: [i32; 3],
    entity: horizon_core::geometry::Aabb,
) -> bool {
    let at = Vec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
    shape
        .boxes()
        .iter()
        .any(|b| b.move_by(at).intersects(entity))
}

/// `ClientLevel.syncBlockState`'s snap: a restored block that meets the player
/// sends it back to the position the prediction remembered, unless the player
/// was teleported after that prediction was made
/// (`lastTeleportSequence < sequence`: the remembered position is stale).
pub(super) fn snap_target(
    recorded: [f64; 3],
    last_teleport_sequence: i32,
    sequence: i32,
    meets: bool,
) -> Option<[f64; 3]> {
    (meets && last_teleport_sequence < sequence).then_some(recorded)
}

/// [`Remote::has_level`]: logged in (a Login arrived) and no reconfiguration
/// since (`reconfiguring` is set by StartConfiguration, cleared by the next
/// Login).
fn client_has_level(logged_in: bool, reconfiguring: Option<u64>) -> bool {
    logged_in && reconfiguring.is_none()
}

/// `ClientChunkCache.Storage.inRange`: whether column `x`, `z` is within `calculateStorageRange`
/// (the server's radius, at least two, plus three) of the view centre. The radius and the centre
/// are the server's to send, any `i32`: the distances are taken without overflow, which panicked
/// a debug build and, wrapping in release, dropped every loaded chunk.
fn in_storage_range(view_distance: i32, center: (i32, i32), x: i32, z: i32) -> bool {
    let range = view_distance.max(2).saturating_add(3).unsigned_abs();
    x.abs_diff(center.0) <= range && z.abs_diff(center.1) <= range
}

// A packet's removals (`RemoveEntities`), apart: this file may not grow.
#[cfg(test)]
mod interaction_event_tests;

#[cfg(test)]
mod native_entity_tests;

#[cfg(test)]
mod effect_blend_tests;
#[cfg(test)]
mod level_sound_tests;

#[cfg(test)]
mod attachment_query_tests;
#[cfg(test)]
mod passage_cross_tests;
#[cfg(test)]
pub(crate) mod tests;

<<<<<<< ours
        let done = game.remote_use_block(Hand::Main);
        assert!(done, "flint and steel useOn must end the click");
    }
}

/// The level a server names (`ClientboundLoginPacket`, `ClientboundRespawnPacket`): one
/// beyond vanilla's three (a mod's dimension) becomes an extra level of this client, drawn as
/// its dimension type says, so its own levels stay apart from the Overworld.
fn client_level(
    name: &str,
    dimension: &horizon_protocol::registry::DimensionType,
    shape: horizon_client::chunk::LevelShape,
) -> Dim {
    if let Some(dim) = Dim::known(name) {
        return dim;
    }
    let looks_like = if dimension.has_ceiling || !dimension.has_skylight {
        "minecraft:the_nether"
    } else {
        "minecraft:overworld"
    };
    let level = horizon_client::chunk::ModLevel {
        id: Box::leak(name.to_owned().into_boxed_str()),
        dimension_type: Box::leak(dimension.name.clone().into_boxed_str()),
        shape,
        coordinate_scale: 1.0,
        looks_like,
    };
    horizon_client::chunk::register_extra_level(level).map_or(Dim::Overworld, Dim::Extra)
}
||||||| base
        let done = game.remote_use_block(Hand::Main);
        assert!(done, "flint and steel useOn must end the click");
    }
}
=======
#[cfg(test)]
mod movement_input_tests;
#[cfg(test)]
#[path = "player_tick_tests.rs"]
mod player_tick_tests;
>>>>>>> theirs
