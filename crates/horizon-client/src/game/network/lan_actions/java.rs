//! Java mods' hooks in what guests do (game/java_server.rs answers them): their cancels of a
//! teleport, a slot change, a ride, and their own item and orb pickups and fall damage.
use super::*;
use horizon_client::lan::guest::Guest;

/// Java mods' hooks are about guest `uuid` while it acts: the player they were about before.
pub(super) fn set_actor(uuid: u128) -> Option<String> {
    Game::set_java_actor(Some(horizon_protocol::crypto::uuid_string(uuid)))
}

impl Game<'_> {
    /// The hub, for the Java mods' answers to what a guest did (always in place while a guest
    /// acts: its packets come through it).
    pub(in crate::game) fn hub_host(&mut self) -> &mut LanHost {
        self.lan.as_mut().expect("the hub is in place while a guest acts")
    }

    /// A guest's payload on a channel nothing before took: the Java mods' devices' (a guest
    /// that runs its own, or what its game sends them; game/java_device.rs).
    #[cfg(feature = "java-mods")]
    pub(super) fn java_device_payload(
        &mut self,
        conn: u32,
        channel: &str,
        data: &[u8],
    ) {
        let Some(mut host) = self.lan.take() else {
            return;
        };
        if channel == horizon_jvm::device::RUNS_OWN_CHANNEL {
            self.java_device_runs_own(&mut host, conn, data)
        } else {
            self.java_device_from_game(&mut host, conn, channel, data)
        }
        self.lan = Some(host);
    }

    /// ServerPlayer.teleport's head: a Java mod that cancels it leaves the guest where it was,
    /// facing as it did (`old_look`; java_server.rs). True when cancelled.
    pub(super) fn java_guest_teleport_refused(
        &mut self,
        g: &mut Guest,
        m: &mut PlayerMotion,
        at: [f64; 3],
        old_look: [f32; 2],
        moves: bool,
    ) -> bool {
        if g.vehicle.is_none() && moves && {
            let dim = self.dim.id();
            self.java_player_teleport(g.uuid, dim, at, [g.yaw, g.pitch], false)
        } {
            m.position = Vec3::new(g.position[0], g.position[1], g.position[2]);
            [g.yaw, g.pitch] = old_look;
            return true;
        }
        false
    }

    /// Java mods hear the teleport (ServerPlayer.teleport; java_server.rs): what moves the
    /// guest during an action is a teleport (commands set the position).
    pub(super) fn java_guest_teleported(&mut self, teleport: Option<(u128, [f64; 3], [f32; 2])>) {
        if let Some((uuid, at, look)) = teleport {
            let dim = self.dim.id();
            self.java_player_teleport(uuid, dim, at, look, true);
        }
    }

    /// Java mods' heads of Inventory.setSelectedSlot (Carry On while carrying): a cancel keeps
    /// the old slot, which the game is told. True when cancelled.
    pub(super) fn java_guest_slot_refused(
        &mut self,
        conn: u32,
        gi: usize,
        slot: i16,
    ) -> bool {
        let wanted = i32::from(slot);
        if (0..9).contains(&slot)
            && self
                .as_guest(conn, |g| g.java_select_slot(wanted))
                .unwrap_or(false)
            && let Some(host) = self.lan.as_mut()
        {
            let kept = host.hub.guests[gi].selected as i32;
            host.hub
                .send_to(conn, &C::SetHeldSlot(play::SetHeldSlot { slot: kept }));
            return true;
        }
        false
    }

    /// Java mods' heads of Entity.startRiding: a cancel takes the guest off (true).
    pub(super) fn java_guest_ride_refused(
        &mut self,
        conn: u32,
        gi: usize,
        before: Option<u32>,
    ) -> bool {
        let vehicle = self.lan.as_ref().and_then(|h| h.hub.guests[gi].vehicle);
        if let Some(vehicle) = vehicle
            && before != Some(vehicle)
            && self
                .as_guest(conn, |g| g.java_ride(vehicle))
                .unwrap_or(false)
        {
            self.as_guest(conn, |g| g.stop_riding());
            if let Some(host) = self.lan.as_mut() {
                host.hub.guests.update(gi, |g| g.vehicle = before);
            }
            return true;
        }
        false
    }

    /// Java mods patch fall damage (PlayerMotion::route_fall): each landing that would hurt
    /// is theirs to judge, then dealt here; its events join `events`.
    pub(super) fn java_routed_falls(&mut self, events: &mut Vec<MotionEvent>) {
        let game = self;
        let routed: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                MotionEvent::Fall(horizon_client::landing::FallEvent::Routed {
                    distance,
                    multiplier,
                    source,
                    safe,
                    damage_multiplier,
                    contact,
                }) => Some((
                    *distance,
                    *multiplier,
                    *source,
                    *safe,
                    *damage_multiplier,
                    *contact,
                )),
                _ => None,
            })
            .collect();
        for (distance, multiplier, source, safe, damage_multiplier, contact) in routed {
            let stalagmite = source == horizon_world::block_landing::LandingDamage::Stalagmite;
            let damage = game
                .java_fall(distance, multiplier, stalagmite, safe, damage_multiplier)
                .unwrap_or_else(|| {
                    ((distance + 1.0e-6 - safe) * f64::from(multiplier) * damage_multiplier)
                        .floor()
                        .max(0.0) as f32
                });
            let Some(player) = game.player.as_mut() else {
                break;
            };
            let _ = MotionWorld {
                entities: &[],
                player,
                scene: &mut game.world,
                assets: &game.pack.assets,
                collision_binding: game.collision,
                resources: game.motion,
                health: &game.health_resources,
                tag_items: Some(world_items!(game)),
                loop_context: &game.loop_context,
                damage_hooks: &game.damage_hooks,
                block_registries: Some(&game.pack.block_registries),
            }
            .routed_fall_damage(damage, source, contact);
            events.append(&mut player.events);
        }
        if let Some(player) = game.player.as_mut() {
            player.route_fall = false;
        }
    }

    /// A Java mod patches item pickup: vanilla's ItemEntity.playerTouch with its patches takes
    /// what it takes (into the guest's slots). The rest of dropped item `i` (`Some(None)`: all
    /// taken), or `None` when no mod took the pickup.
    pub(super) fn java_guest_item_touch(
        &mut self,
        conn: u32,
        i: usize,
    ) -> Option<Option<ItemStack>> {
        let item = self.dropped[i].clone();
        let left = self
            .as_guest(conn, |g| g.java_item_touch(&item))
            .flatten()?;
        let before = item.stack.count;
        Some(match left {
            0 => None,
            left => Some(ItemStack {
                count: left.min(before),
                ..item.stack
            }),
        })
    }

    /// A Java mod patches the orbs' pickup (Clumps: all of it at once, no delay): vanilla's
    /// playerTouch with its patches gives the experience and repairs. True when a mod took
    /// orb `index` (guest entity `id` at `at`).
    pub(super) fn java_guest_orb_touch(
        &mut self,
        conn: u32,
        index: usize,
        id: i32,
        at: [f64; 3],
    ) -> bool {
        let orb = self.orbs[index].clone();
        let Some((left, delay)) = self
            .as_guest(conn, |game| game.java_orb_touch(&orb))
            .flatten()
        else {
            return false;
        };
        let Some(host) = self.lan.as_mut() else {
            return true;
        };
        let play = host.play.entry(conn).or_default();
        play.xp_delay = delay.max(0) as u32;
        if left >= orb.count {
            return true;
        }
        host.hub.take_item(orb.id as i32, id, 1, at);
        if left == 0 {
            self.orbs.remove(index);
        } else {
            self.orbs[index].count = left;
        }
        let pitch = (self.world.random.next_float() - self.world.random.next_float()) * 0.35 + 0.9;
        let p = orb.position;
        self.audio.play(
            "minecraft:entity.experience_orb.pickup",
            Some([p.x, p.y, p.z]),
            0.1,
            pitch,
        );
        true
    }
}
