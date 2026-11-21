//! A player breaking a block, as the 1.20.1 mods hear it (phase 5): a 1.20.1 mod's block is
//! broken by 1.20.1's own `ServerPlayerGameMode.destroyBlock` in the runtime, so its own code
//! runs (contents dropped, its own loot table with the real tool, the tool's wear, Fabric's
//! `PlayerBlockBreakEvents` fired by Fabric itself); any other block is broken by Horizon, with
//! Fabric's `PlayerBlockBreakEvents.BEFORE` (which may stop it) and `AFTER` around it, so a
//! 1.20.1 mod listening for breaks hears vanilla blocks too.

use super::{Level, NAMESPACES, RUNTIME, stack_from_120, state_name, tracing, wait_for_runtime};
use horizon_client::items::ItemStack;
use horizon_jvm_remote::mirror120::{BreakBlock, BreakHow, Broke};
use horizon_world::registry::BlockStateId;

/// What Horizon does with a break once the 1.20.1 mods have heard of it.
pub(in crate::game) enum Break120 {
    /// A mod stopped it: the block stays (the client puts it back on the acknowledgement).
    Stopped,
    /// 1.20.1's code broke it (a 1.20.1 mod's block): its removal, drops and the tool's wear
    /// have landed; nothing more to do.
    Done,
    /// Horizon breaks it; `after`: then `java120_after_break`.
    Horizon { after: bool },
}

/// A held stack as 1.20.1 item NBT: its id, count, damage, enchantments and name, the parts
/// that change what a break drops and what a tool does. (The rest of 26.3's components have no
/// way down yet: the downgrade direction, phase 3.)
pub(in crate::game) fn stack_to_120(stack: &ItemStack) -> String {
    let mut tag: Vec<String> = Vec::new();
    if stack.damage > 0 {
        tag.push(format!("Damage:{}", stack.damage));
    }
    let list = |entries: &[(std::sync::Arc<str>, u32)]| {
        entries
            .iter()
            .map(|(id, level)| format!("{{id:{},lvl:{level}s}}", quoted(id)))
            .collect::<Vec<_>>()
            .join(",")
    };
    if !stack.enchantments().is_empty() {
        tag.push(format!("Enchantments:[{}]", list(stack.enchantments())));
    }
    if !stack.stored_enchantments().is_empty() {
        tag.push(format!(
            "StoredEnchantments:[{}]",
            list(stack.stored_enchantments())
        ));
    }
    if let Some(name) = stack.custom_name() {
        let json = serde_json::json!({ "text": name }).to_string();
        tag.push(format!("display:{{Name:{}}}", quoted(&json)));
    }
    let tag = if tag.is_empty() {
        String::new()
    } else {
        format!(",tag:{{{}}}", tag.join(","))
    };
    format!(
        "{{id:{},Count:{}b{tag}}}",
        quoted(&stack.item),
        stack.count.min(127)
    )
}

/// An SNBT string, quoted and escaped.
fn quoted(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl super::super::Game<'_> {
    /// Whether `state` is one of the 1.20.1 mods' blocks.
    fn java120_block(&self, state: BlockStateId) -> bool {
        let name = state_name(self.pack.assets.registry, state);
        let namespaces = NAMESPACES.lock().unwrap_or_else(|e| e.into_inner());
        name.split_once(':')
            .is_some_and(|(ns, _)| namespaces.as_ref().is_some_and(|n| n.contains(ns)))
    }

    fn java120_break_request(
        &self,
        pos: [i32; 3],
        state: BlockStateId,
        how: BreakHow,
    ) -> BreakBlock {
        BreakBlock {
            uuid: self.java120_actor(),
            name: self.acting_player_name().to_owned(),
            dimension: self.dim.id().to_owned(),
            pos,
            state: state_name(self.pack.assets.registry, state),
            held: self.held().map(stack_to_120).unwrap_or_default(),
            creative: self.mode == horizon_client::command::args::GameMode::Creative,
            stance: self.java120_stance(),
            how,
        }
    }

    /// The player acting now is about to break the block at `pos` (`ServerPlayerGameMode
    /// .destroyBlock`'s head, where the 26.3 bridge asks too).
    pub(in crate::game) fn java120_break(
        &mut self,
        pos: [i32; 3],
        state: BlockStateId,
    ) -> Break120 {
        if self.remote.is_some() {
            return Break120::Horizon { after: false };
        }
        let theirs = self.java120_block(state);
        let how = if theirs {
            BreakHow::Theirs
        } else {
            BreakHow::Before
        };
        let request = self.java120_break_request(pos, state, how);
        let broke = {
            // Their own block: the player waits on it, and nobody else can break it right, so
            // a busy runtime is waited for. Anyone else's: the events only, never worth a stall.
            let guard = if theirs {
                wait_for_runtime()
            } else {
                RUNTIME.try_lock().ok()
            };
            let Some(mut guard) = guard else {
                return Break120::Horizon { after: false };
            };
            let Some(runtime) = guard.as_mut() else {
                return Break120::Horizon { after: false };
            };
            runtime.break_block(&Level(self), &request)
        };
        trace120!("{pos:?}: breaking {} ({how:?}): {broke:?}", request.state);
        let broke = match broke {
            Ok(broke) => broke,
            Err(e) => {
                log::warn!("1.20.1 mods: breaking {} failed: {e}", request.state);
                // Their block, which Horizon can't break right: left as it is. Anyone else's:
                // broken as if no mod were listening.
                return if theirs {
                    Break120::Stopped
                } else {
                    Break120::Horizon { after: false }
                };
            }
        };
        if !theirs {
            return if broke.goes {
                Break120::Horizon { after: true }
            } else {
                Break120::Stopped
            };
        }
        self.java120_land_break(&broke);
        if broke.goes {
            Break120::Done
        } else {
            Break120::Stopped
        }
    }

    /// What 1.20.1's break of their block did: the block gone, what dropped, the tool worn.
    fn java120_land_break(&mut self, broke: &Broke) {
        let dimension = self.dim.id().to_owned();
        self.apply_java120(
            &dimension,
            &broke.set,
            &broke.block_entities,
            &broke.spawned,
        );
        let before = self.held().map(stack_to_120).unwrap_or_default();
        if broke.held == before {
            return;
        }
        let after = if broke.held.is_empty() {
            None
        } else {
            match stack_from_120(&broke.held) {
                Some(stack) => Some(stack),
                // A tool that doesn't read back stays as it was, rather than vanish.
                None => return,
            }
        };
        *self.held_mut() = after;
    }

    /// The player acting now broke the block at `pos` (it was `state`): Fabric's
    /// `PlayerBlockBreakEvents.AFTER` for the 1.20.1 mods.
    pub(in crate::game) fn java120_after_break(&mut self, pos: [i32; 3], state: BlockStateId) {
        if self.remote.is_some() {
            return;
        }
        let request = self.java120_break_request(pos, state, BreakHow::After);
        let broke = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.break_block(&Level(self), &request)
        };
        match broke {
            // What a listener did (a vein miner breaking the rest of the vein) lands too.
            Ok(broke) => {
                let dimension = self.dim.id().to_owned();
                self.apply_java120(
                    &dimension,
                    &broke.set,
                    &broke.block_entities,
                    &broke.spawned,
                );
            }
            Err(e) => log::warn!("1.20.1 mods: after breaking {}: {e}", request.state),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tool_goes_down_with_its_enchantments_and_wear() {
        let mut pick = ItemStack::new("minecraft:diamond_pickaxe", 1);
        pick.damage = 12;
        pick.data_mut().enchantments = vec![("minecraft:silk_touch".into(), 1)];
        let snbt = stack_to_120(&pick);
        // 1.20.1's own shape (ItemStack.save): Count as a byte, the rest under tag.
        assert_eq!(
            snbt,
            r#"{id:"minecraft:diamond_pickaxe",Count:1b,tag:{Damage:12,Enchantments:[{id:"minecraft:silk_touch",lvl:1s}]}}"#
        );
        // And back up again, through 1.20.1 -> 26.3's fixes: the same tool.
        let back = stack_from_120(&snbt).expect("it reads back");
        assert_eq!(back.damage, 12);
        assert_eq!(back.enchantment_level("minecraft:silk_touch"), 1);
    }

    #[test]
    fn a_plain_stack_has_no_tag() {
        let carrots = ItemStack::new("minecraft:carrot", 5);
        assert_eq!(
            stack_to_120(&carrots),
            r#"{id:"minecraft:carrot",Count:5b}"#
        );
    }
}
