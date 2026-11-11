//! Java mods and items: Horizon's items for the mods' blocks, item use, crafted takes, item
//! and orb pickups, drops, hoppers on the mods' blocks.
use super::*;
use serde_json::Value;

impl Game<'_> {
    /// Whether the item entity `id` is held by a Java mod's block this tick (its tick is theirs).
    pub(in crate::game) fn java_holds_item(id: u32) -> bool {
        JAVA_HELD_ITEMS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(&id)
    }
}

/// Horizon's items a Java mod's block holds (`HorizonItems`' held: crushing wheels pulling one
/// in): the mirror moves them, by vanilla's tick, so Horizon's item tick leaves them.
pub(super) static JAVA_HELD_ITEMS: std::sync::Mutex<std::collections::BTreeSet<u32>> =
    std::sync::Mutex::new(std::collections::BTreeSet::new());

/// Java mods' blocks a hopper found no item storage on, by dimension, position and face: asked
/// again after this game time (a hopper beside a mod's decorative block doesn't call Java each
/// tick; a block entity that wasn't there yet is found a little later).
pub(super) static JAVA_NO_STORAGE: std::sync::Mutex<
    Option<std::collections::HashMap<(&'static str, [i32; 3], u8), i64>>,
> = std::sync::Mutex::new(None);

/// The blocks the mods' container entities (a mod's chest minecart) were in at the last
/// snapshot, by dimension: Horizon's hoppers and droppers ask Java there too.
pub(super) static JAVA_CONTAINER_BLOCKS: std::sync::Mutex<
    Option<std::collections::HashMap<String, std::collections::HashSet<[i32; 3]>>>,
> = std::sync::Mutex::new(None);

impl Game<'_> {
    /// Whether the block at `pos` is a Java mod's that a hopper may ask about `face`, or a mod's
    /// container entity is in it.
    pub(super) fn java_hopper_block(
        &self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
    ) -> bool {
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

    pub(super) fn java_hopper_no_storage(
        &self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
    ) {
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
    pub(in crate::game) fn java_hopper_insert(
        &mut self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
        stacks: &[Option<&horizon_client::items::ItemStack>],
    ) -> Option<Option<usize>> {
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
        let moved = self
            .with_java_world(|| horizon_jvm::events::hopper_insert(dim, pos, face as i32, &refs));
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

    /// A hopper pulls one item out of the Java mod's block at `pos` through `face` (Fabric
    /// Transfer API's patch of `suckInItems`): the first it offers that fits (`any_room`: the
    /// hopper has an empty slot; else one of `partial`, its stacks not full yet). None when
    /// the block isn't a Java mod's or holds no items there; else the item, if one came.
    pub(in crate::game) fn java_hopper_extract(
        &mut self,
        pos: [i32; 3],
        face: horizon_core::geometry::Direction,
        any_room: bool,
        partial: &[&horizon_client::items::ItemStack],
    ) -> Option<Option<horizon_client::items::ItemStack>> {
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
}

/// A guest's item in vanilla's `Inventory` slot `slot`: 0-35 the inventory, 36 feet to
/// 39 head (Horizon keeps the armour head first), 40 the off hand.
pub(super) fn guest_slot(
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
pub(super) static JAVA_INVENTORIES: std::sync::Mutex<Option<std::collections::HashMap<u128, u64>>> =
    std::sync::Mutex::new(None);

/// A guest's inventory for Java mods (vanilla's order, SNBT per slot), when it changed
/// since they last had it.
pub(super) fn java_inventory(g: &horizon_client::lan::guest::Guest) -> Option<Vec<String>> {
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

pub(super) static JAVA_DROPS: std::sync::Mutex<Vec<(String, horizon_jvm::events::Action)>> =
    std::sync::Mutex::new(Vec::new());

impl Game<'_> {
    /// Horizon's item entities within 32 blocks of a player, per dimension, for the mods'
    /// blocks and machines (`HorizonItems`: a depot or a belt an item lands on, crushing
    /// wheels it falls between, a fan blowing on it): `{"<dimension>": [[id, x, y, z, vx,
    /// vy, vz, on_ground, age, pickup_delay, "stack snbt"]]}`, at most 256 a level.
    pub(super) fn java_items_json(&self, players: &[horizon_jvm::events::Player]) -> String {
        use std::fmt::Write;
        // Each item's stack as SNBT, kept while it doesn't change (most don't, tick to tick).
        static SNBT: std::sync::Mutex<
            Option<std::collections::HashMap<u32, (horizon_client::items::ItemStack, String)>>,
        > = std::sync::Mutex::new(None);
        let mut cache = SNBT.lock().unwrap_or_else(|e| e.into_inner());
        let old = cache.take().unwrap_or_default();
        let mut kept = std::collections::HashMap::new();
        let mut out = String::from("{");
        let mut level =
            |out: &mut String, dim: &str, items: &[horizon_client::entities::ItemEntity]| {
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
                for d in items {
                    let at = [d.position.x, d.position.y, d.position.z];
                    if !near.iter().any(|p| {
                        (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2) + (p[2] - at[2]).powi(2)
                            <= 32.0 * 32.0
                    }) {
                        continue;
                    }
                    let snbt = match old.get(&d.id) {
                        Some((stack, snbt)) if *stack == d.stack => snbt.clone(),
                        _ => horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(
                            &d.stack,
                        )),
                    };
                    if n > 0 {
                        out.push(',');
                    }
                    let _ = write!(
                        out,
                        "[{},{},{},{},{},{},{},{},{},{},{}]",
                        d.id,
                        at[0],
                        at[1],
                        at[2],
                        d.velocity.x,
                        d.velocity.y,
                        d.velocity.z,
                        u8::from(d.on_ground),
                        d.age,
                        d.pickup_delay.clamp(-32768, 32767),
                        serde_json::to_string(&snbt).unwrap_or_default()
                    );
                    kept.insert(d.id, (d.stack.clone(), snbt));
                    n += 1;
                    if n >= 256 {
                        break;
                    }
                }
                out.push(']');
            };
        level(&mut out, self.dim.id(), &self.dropped);
        for dim in self.levels.parked_dims() {
            if let Some(items) = self.levels.parked_items(dim) {
                level(&mut out, dim.id(), items);
            }
        }
        *cache = Some(kept);
        out.push('}');
        out
    }

    /// What the mods did to Horizon's items they were shown (`Action::ItemsChanged`): an
    /// item a mod's block took in leaves the level; one a mod changed (a fan's smelting,
    /// what a block left of it) takes its new stack; one pushed or moved (a fan's wind, a
    /// crushing wheel pulling it in) takes its new velocity and place.
    pub(super) fn java_items_changed(&mut self, json: &str) {
        let Ok(report) = serde_json::from_str::<Value>(json) else {
            return;
        };
        if let Some(Value::Array(held)) = report.get("held") {
            let mut ids = super::JAVA_HELD_ITEMS
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            ids.clear();
            ids.extend(held.iter().filter_map(Value::as_u64).map(|id| id as u32));
        }
        let Some(Value::Array(changes)) = report.get("changes") else {
            return;
        };
        let vec3 = |v: &Value| -> Option<horizon_core::geometry::Vec3> {
            let a = v.as_array()?;
            Some(horizon_core::geometry::Vec3::new(
                a.first()?.as_f64()?,
                a.get(1)?.as_f64()?,
                a.get(2)?.as_f64()?,
            ))
        };
        for change in changes {
            let (Some(dim), Some(id)) = (
                change.get("d").and_then(Value::as_str),
                change.get("id").and_then(Value::as_u64),
            ) else {
                continue;
            };
            let id = id as u32;
            let removed = change.get("removed").and_then(Value::as_bool) == Some(true);
            let position = change.get("pos").and_then(vec3);
            let velocity = change.get("vel").and_then(vec3);
            let new_stack: Option<Option<horizon_client::items::ItemStack>> =
                match change.get("stack") {
                    Some(Value::String(snbt)) => match horizon_world::nbt::parse_snbt(snbt) {
                        Ok((tag, _)) => Some(horizon_items::item_nbt::item_from_nbt(&tag)),
                        Err(_) => None,
                    },
                    // An emptied stack: the item is gone.
                    Some(Value::Null) => Some(None),
                    _ => None,
                };
            let gone = removed || matches!(new_stack, Some(None));
            let apply = |d: &mut horizon_client::entities::ItemEntity| {
                if let Some(Some(stack)) = &new_stack {
                    d.stack = stack.clone();
                }
                if let Some(p) = position {
                    d.position = p;
                }
                if let Some(v) = velocity {
                    d.velocity = v;
                }
                d.needs_sync = true;
            };
            if self.dim.id() == dim {
                let Some(i) = self.dropped.iter().position(|d| d.id == id) else {
                    continue;
                };
                if gone {
                    self.remove_dropped(i);
                } else if let Some(d) = self.dropped.get_mut(i) {
                    apply(d);
                }
                continue;
            }
            let dim = super::super::Dim::from_id(dim);
            if let Some(items) = self.levels.parked_items_mut(dim)
                && let Some(i) = items.iter().position(|d| d.id == id)
            {
                if gone {
                    items.remove(i);
                } else if let Some(d) = items.get_mut(i) {
                    apply(d);
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
                horizon_jvm::events::use_item(&uuid, dim, i32::from(off_hand), selected, yaw, pitch)
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

    /// The player acting now took these crafting results: vanilla's `ResultSlot.onTake`
    /// on copies, for the Java mods' crafted hooks; what they give comes back as slots.
    pub(in crate::game) fn java_crafted(&mut self, takes: &[horizon_client::menu::CraftedTake]) {
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
                        self.audio
                            .play("minecraft:entity.player.levelup", at, volume * 0.75, 1.0);
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
            let snbt =
                horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(&item.stack));
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

    /// The items Java mods dropped and the level events they made in the level in place: Horizon's item entities, with
    /// the motion the mod gave them and vanilla's pickup delay (`ItemEntity.setDefaultPickUpDelay`).
    pub(super) fn apply_java_drops(&mut self) {
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
}
