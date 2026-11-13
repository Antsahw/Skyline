//! Java mods on a server: their payloads to this client (screens, HUD, keys, entities,
//! devices), and the clicks on their entities, which only the server's mods know.
use super::*;

impl Game<'_> {
    /// A new server: its Java mods (if any) say which keys they bind once we're in.
    pub(super) fn java_new_server(&mut self) {
        self.java.own_skin_sent = false;
        self.java.binds = Default::default();
        self.java.view = Default::default();
        self.settings.keys.set_mods(Vec::new());
        self.java.overlay = Default::default();
        self.settings.mods.java.clear();
    }

    /// A payload on one of the Java mods' channels (game/java_screen.rs,
    /// game/java_entities.rs, game/java_device.rs), handled: false for any other channel.
    pub(super) fn remote_java_payload(&mut self, channel: &str, data: &[u8]) -> bool {
        // A game running every mod's client itself draws those itself.
        let own = super::super::java_screen::own_java_client();
        if !own && channel.starts_with("horizon:java_") {
            self.send_java_own_skin();
        }
        match channel {
            super::super::java_screen::CONFIG_MODS if !own => self.remote_java_config_mods(data),
            super::super::java_screen::OVERLAY_CHANNEL if !own => self.remote_java_overlay(data),
            super::super::java_screen::HUD_CHANNEL if !own => self.remote_java_hud(data),
            super::super::java_screen::CHANNEL => self.remote_java_screen(data),
            super::super::java_screen::KEYS if !own => self.remote_java_keys(data),
            super::super::java_screen::KEYS_120 => self.remote_java120_keys(data),
            #[cfg(feature = "java-mods-120")]
            super::super::java_mods_120::mobs::CHANNEL => self.remote_java120_mobs(data),
            #[cfg(feature = "java-mods-120")]
            super::super::java_mods_120::block_renders::CHANNEL => {
                self.remote_java120_block_renders(data)
            }
            super::super::java_screen::KEY_MAPPINGS if !own => self.remote_java_key_mappings(data),
            super::super::java_screen::VIEW if !own => self.remote_java_view(data),
            #[cfg(feature = "java-mods")]
            super::super::java_entities::CHANNEL => self.remote_java_entities(data),
            #[cfg(feature = "java-mods")]
            super::super::java_entities::PARTICLES => self.remote_java_particles(data),
            #[cfg(feature = "java-mods")]
            horizon_jvm::device::SERVER_MODS_CHANNEL => {
                super::super::java_device::server_mods(data)
            }
            #[cfg(feature = "java-mods")]
            _ if super::super::java_device::takes(channel) => {
                super::super::java_device::payload(channel, data)
            }
            _ => return false,
        }
        true
    }

    /// The launcher's offline skin, which only this game draws, to a server running Java
    /// mods ([`OWN_SKIN`](super::super::java_screen::OWN_SKIN)), once: what mods build from a
    /// player's skin there (3D skin layers' voxels) is then this player's own. Not when the
    /// server or the account gives the player a skin (the server has that one).
    fn send_java_own_skin(&mut self) {
        if self.java.own_skin_sent {
            return;
        }
        let Some(r) = self.remote.as_ref() else {
            return;
        };
        if r.skins.get(&r.profile).map(String::as_str) != Some("horizon/offline_skin") {
            return;
        }
        let Some((rgba, slim)) = horizon_client::skins::offline_skin() else {
            return;
        };
        self.java.own_skin_sent = true;
        let mut data = Vec::with_capacity(1 + rgba.len());
        data.push(u8::from(*slim));
        data.extend_from_slice(rgba);
        self.send(S::CustomPayload(
            horizon_protocol::packets::configuration::CustomPayload {
                channel: super::super::java_screen::OWN_SKIN.into(),
                data,
            },
        ));
    }

    /// A Java mod's entity: the server's mods use it (Player.interactOn); this client has
    /// no copy of it to ask, so the click ends here, unless the hand holds what is aimed
    /// past entities (a bow, a snowball: no vanilla mob takes those, Mob.interact passes):
    /// the item is used too, as vanilla's PASS goes on to useItem. `None`: no Java mod's
    /// entity is aimed at; else what `remote_use_entity` answers.
    pub(super) fn remote_use_java_entity(&mut self, hand: Hand, sneaking: bool) -> Option<bool> {
        let (id, distance) = self.java_entity_hit()?;
        if !(self.mob_hit().is_none_or(|(_, d)| distance < d) && self.before_block(distance)) {
            return None;
        }
        if hand == Hand::Off {
            return Some(false);
        }
        self.send(S::Interact(play::Interact {
            id,
            hand: hand_id(hand),
            location: self.java_entity_location(id, distance),
            secondary: sneaking,
        }));
        #[cfg(feature = "java-mods")]
        {
            let item = self.stack_in(self.selected).map_or("", |s| &*s.item);
            if super::super::java_entities::used_past_entities(item) {
                return Some(false);
            }
        }
        self.swing_arm();
        Some(true)
    }

    /// A Java mod's entity aimed at, before any mob and block: the server's mods attack it
    /// (Player.attack). False when none is.
    pub(super) fn remote_attack_java_entity(&mut self) -> bool {
        if let Some((id, distance)) = self.java_entity_hit()
            && self.mob_hit().is_none_or(|(_, d)| distance < d)
            && self.before_block(distance)
        {
            self.send(S::Attack(play::Attack { id }));
            self.reset_attack_strength();
            return true;
        }
        false
    }
}
