//! Java mods' entities: snapshots to players, attacks and interactions, mobs, rides.
use super::*;
use horizon_protocol::packets::play::{self, Clientbound as C};

impl Game<'_> {
    /// A mob a routed attack hit (Java mods' `Player.attack`, `game/java_server.rs`): hurt
    /// by the player with what its copy's `hurtServer` got, pushed as its `knockback` was
    /// (the strength and direction the mods left), and the hit's sounds and particles. The
    /// weapon's wear and the exhaustion were the mirror's (they come back as slots and food).
    pub(in crate::game) fn routed_hit(
        &mut self,
        index: usize,
        amount: f32,
        knockback: f64,
        knock_x: f64,
        knock_z: f64,
        primary: bool,
    ) {
        let from = self
            .player
            .as_ref()
            .map(|p| [p.position.x, p.position.y, p.position.z]);
        self.combat.last_hurt_mob = Some((self.mobs[index].id, self.world.game_time));
        let old_health = self.mobs[index].health;
        let source = match from {
            Some(f) => self.attack_source(f),
            None => horizon_client::mobs::Damage::of(horizon_client::mobs::DamageKind::Generic),
        };
        let landed = amount > 0.0 && self.hurt_by_player(index, amount, source);
        if !landed {
            if primary {
                self.audio
                    .play("minecraft:entity.player.attack.nodamage", from, 1.0, 1.0);
            }
            return;
        }
        let victim = self.mob_entity(&self.mobs[index]);
        self.player_hit(
            &victim,
            super::super::creatures::damage_type(source.kind, true),
            None,
            amount,
        );
        if knockback > 0.0 {
            self.mobs[index].knockback(knockback, knock_x, knock_z, &source);
        }
        let mob = &self.mobs[index];
        let actual = old_health - mob.health;
        let (feet, height) = (
            [mob.position.x, mob.position.y, mob.position.z],
            mob.kind.height,
        );
        self.stats
            .custom("damage_dealt", (actual * 10.0).round() as i32);
        if actual > 2.0 {
            let indicator = horizon_client::particles::ParticleOptions::simple(
                horizon_client::particles::ParticleType::DamageIndicator,
            );
            let at = [feet[0], feet[1] + height * 0.5, feet[2]];
            self.particles.send_particles(
                &indicator,
                at,
                [0.1, 0.0, 0.1],
                0.2,
                (actual * 0.5) as u32,
            );
            self.lan_send_particles(super::super::lan_particles::QueuedParticle {
                dimension: self.dim.id(),
                options: indicator,
                state: None,
                at,
                spread: [0.1, 0.0, 0.1],
                speed: [0.2; 3],
                count: (actual * 0.5) as u32 as i32,
                randomization: 0,
            });
        }
        self.last_hurt_by_player
            .insert(self.mobs[index].id, self.lan_acting);
        if primary {
            self.audio
                .play("minecraft:entity.player.attack.strong", from, 1.0, 1.0);
        }
        for event in std::mem::take(&mut self.mobs[index].outbox) {
            match event {
                horizon_client::mobs::MobEvent::Sound(s, at, volume, pitch) => {
                    self.audio
                        .play(&format!("minecraft:{s}"), Some(at), volume, pitch);
                }
                other => self.mobs[index].outbox.push(other),
            }
        }
    }
}

/// The Java mods' entity types (`/summon` of one goes to their server), once the bridge
/// has its registries.
pub(super) fn java_entity_types() -> &'static std::collections::HashSet<String> {
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

impl Game<'_> {
    /// The mods' entities after their tick (`HorizonEntityRender.snapshot`), to each
    /// player whose level they are in, within 160 blocks of it, on
    /// java_entities::CHANNEL, with the model trees its game has not had; the window's own
    /// player in-frame gets them straight.
    pub(super) fn send_java_entities(&mut self, json: &str) {
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
        let _timed = super::super::java_profile::Timed::named("(Rust) entities: parse and send");
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
            let drawn = !entities.is_empty() || hands.contains_key(uuid_string(g.uuid).as_str());
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

    /// A mod's mob hurt one of Horizon's (`Natives.hurtMob`, its shadow in the mirror): the
    /// mob takes it as a mob's attack (its armour, invulnerable ticks, death) and is knocked
    /// back from where it came, in its own level (in place or parked).
    pub(super) fn java_hurt_mob(
        &mut self,
        dimension: &str,
        id: i32,
        damage_type: &str,
        amount: f32,
        from: Option<[f64; 3]>,
        by: Option<i32>,
        reset_invulnerable: bool,
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
            attacker_species: None,
        };
        let hurt = |m: &mut horizon_client::mobs::Mob| {
            if !m.alive() || m.removed {
                return;
            }
            if reset_invulnerable {
                m.invulnerable = 0;
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
    pub(super) fn java_vanilla_entity(
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
                horizon_jvm::events::interact_entity(&uuid, p.id, p.hand == 1, p.location, selected)
            });
            if let Err(e) = &used {
                log::warn!("Java mods: using an entity failed: {e}");
            }
            // The player's game swung its arm already.
            self.apply_java_actions(Some(host));
            true
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
            let (feet, yaw, pitch) = ([p.position.x, p.position.y, p.position.z], p.yaw, p.pitch);
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

    /// A vehicle of the level as the Java mods get a copy of it: its type, where it is,
    /// and a mob's NBT (empty for a boat or cart).
    pub(super) fn java_vehicle(&mut self, vehicle: u32) -> Option<(String, [f64; 3], String)> {
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
            let r = self.with_java_world(|| horizon_jvm::events::ride(&uuid, dim, &ty, at, &snbt));
            self.apply_java_actor_slots(&uuid);
            r.unwrap_or(false)
        }
    }

    /// The player acting now gets off `vehicle` (sneaking): before the game takes it off
    /// (`after` false), the Java mods' heads of `stopRiding`/`removeVehicle` (true: one
    /// cancelled, it stays on); after (`after` true), their ends.
    pub(in crate::game) fn java_dismount(&mut self, vehicle: u32, after: bool) -> bool {
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
        let r = self
            .with_java_world(|| horizon_jvm::events::dismount(&uuid, dim, &ty, at, &snbt, after));
        self.apply_java_actor_slots(&uuid);
        r.unwrap_or(false)
    }
}
