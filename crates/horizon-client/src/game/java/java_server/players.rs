//! Java mods and the players: hurts, effects, falls, jumps, teleports, deaths, chat, slots,
//! advancements.
use super::*;

impl Game<'_> {
    /// Sets the player acting now (None: no one); the one before, to put back.
    pub(in crate::game) fn set_java_actor(actor: Option<String>) -> Option<String> {
        JAVA_ACTOR.with(|a| a.replace(actor))
    }

    /// A player is about to be hurt (`LivingEntity.hurtServer` past its guards, where native mods
    /// hear it: native_hooks.rs `hurt_server`): the Java mods' head of `hurtServer` and Fabric's
    /// `ALLOW_DAMAGE`, for the player acting now. They may change `amount`; true cancels the hit.
    pub(in crate::game) fn java_hurt_player(
        hurting: &horizon_client::health::Hurting<'_>,
        amount: &mut f32,
    ) -> bool {
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
}

thread_local! {
    /// The player a server turn acts for now (`as_guest`), by UUID: whom a hook from inside
    /// the game's own code (the head of hurtServer) is about.
    static JAVA_ACTOR: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// Java's hurts of Horizon players (a mod's mob's bite), applied at the next Java tick.
pub(super) static PENDING_HURTS: std::sync::Mutex<Vec<horizon_jvm::events::Action>> =
    std::sync::Mutex::new(Vec::new());
/// Teleports Java mods asked for while the hub was out of `self.lan`, done next tick.
pub(super) static PENDING_TELEPORTS: std::sync::Mutex<Vec<horizon_jvm::events::Action>> =
    std::sync::Mutex::new(Vec::new());

/// Whether each player was dead at the last tick Java mods heard of (for deaths and
/// respawns).
pub(super) static JAVA_DEAD: std::sync::Mutex<Option<std::collections::HashMap<String, bool>>> =
    std::sync::Mutex::new(None);

/// What killed each player who died since the last tick (noted at its death message).
pub(super) static JAVA_DEATH_CAUSES: std::sync::Mutex<
    Option<std::collections::HashMap<String, String>>,
> = std::sync::Mutex::new(None);

impl Game<'_> {
    /// A Java hurt of a Horizon player (`Natives.hurtPlayer`): as Horizon's own mobs hurt
    /// it (`hurt_player_from`: armour, blocking, knockback from where it came, death; the
    /// attacker named as a projectile with no owner is), as that guest; the window's own
    /// player in-frame. A type Horizon doesn't know (a mod's)
    /// counts as a mob's attack, or generic damage when it came from nowhere.
    pub(super) fn java_hurt(&mut self, action: horizon_jvm::events::Action) {
        if let horizon_jvm::events::Action::HurtMob {
            dimension,
            id,
            damage_type,
            amount,
            from,
            by,
            reset_invulnerable,
        } = action
        {
            self.java_hurt_mob(
                &dimension,
                id,
                &damage_type,
                amount,
                from,
                by,
                reset_invulnerable,
            );
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

    /// Moves a player as a Java mod asked: Horizon's own `/tp`, run by the server in the
    /// target dimension (`execute in`), so a connection, the window's player and other
    /// dimensions all go the way vanilla's teleports go.
    pub(super) fn java_teleport(&mut self, action: horizon_jvm::events::Action) {
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

    /// The slot changes Java mods made to the player acting now, made at once in the
    /// inventory in place (a guest's is swapped into the game for its turn, and put back
    /// after it): what the game then does to it (an axe's wear for the break) adds to them.
    pub(super) fn apply_java_actor_slots(&mut self, uuid: &str) {
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
        if !java_mods_on() || !self.hears_players() || !super::java_routed("jumps") {
            return;
        }
        let uuid = self.acting_uuid_string();
        if let Err(e) = self.with_java_world(|| horizon_jvm::events::jump_end(&uuid)) {
            log::warn!("Java mods: a jump failed: {e}");
        }
    }

    pub(super) fn acting_uuid_string(&self) -> String {
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
        {
            if !java_mods_on() || !super::java_routed("teleports") {
                return false;
            }
            let uuid = uuid_string(player);
            let r =
                self.with_java_world(|| horizon_jvm::events::teleport(&uuid, dim, at, look, end));
            match r {
                Ok(refused) => return refused,
                Err(e) => log::warn!("Java mods: a teleport failed: {e}"),
            }
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
        {
            if !java_mods_on() || !self.hears_players() || !super::java_routed("advancements") {
                return false;
            }
            let uuid = self.acting_uuid_string();
            let r = self.with_java_world(|| horizon_jvm::events::award(&uuid, id, criterion, done));
            match r {
                Ok(refused) => return refused,
                Err(e) => log::warn!("Java mods: an advancement failed: {e}"),
            }
        }
        false
    }

    /// [`Game::java_add_effect`] for `player` (None: the player acting now).
    pub(in crate::game) fn java_add_effect_to(
        &mut self,
        player: Option<u128>,
        effect: &horizon_client::effects::EffectInstance,
    ) -> bool {
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
            horizon_jvm::events::add_effect(&uuid, &id, duration, amplifier, ambient, visible)
        })
        .unwrap_or(false)
    }

    /// Whether Java mods patch a player's fall damage (`PlayerMotion::route_fall`).
    pub(in crate::game) fn java_fall_routed(&self) -> bool {
        java_mods_on() && self.hears_players() && super::java_routed("fall_damage")
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

    /// A chat message a player sent.
    pub(in crate::game) fn java_player_chat(&mut self, who: Who, text: &str) {
        if self.hears_players() {
            self.mods
                .java
                .events
                .push(ModEvent::Chat(who, text.to_owned()));
        }
    }
}
