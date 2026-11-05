//! Hoppers, droppers and Java mods' blocks: their block entities live in Java, and Fabric's
//! Transfer API moves items in and out of them (game/java_server.rs, `java_hopper_insert`).
use super::*;

impl Game<'_> {
    /// `ejectItems` into a Java mod's block in front: one item of the first slot it takes.
    /// None when that block holds no items (not a Java mod's, or no storage on `face`).
    pub(super) fn java_eject(
        &mut self,
        hopper: &View,
        front: Pos,
        face: Direction,
    ) -> Option<bool> {
        let c = self.containers();
        let stacks: Vec<Option<ItemStack>> = (0..hopper.len())
            .map(|i| c.get(hopper, i).cloned())
            .collect();
        if stacks.iter().all(Option::is_none) {
            return None;
        }
        let refs: Vec<Option<&ItemStack>> = stacks.iter().map(Option::as_ref).collect();
        let slot = self.java_hopper_insert([front.0, front.1, front.2], face, &refs)?;
        let Some(slot) = slot else {
            return Some(false);
        };
        // The mod took one of the slot's items: it leaves the hopper.
        Some(self.take_one(hopper, slot).is_some())
    }
    /// `suckInItems` from a Java mod's block above, through its bottom face: one item that
    /// fits. None when that block holds no items.
    pub(super) fn java_suck(&mut self, above: Pos, hopper: &View) -> Option<bool> {
        let c = self.containers();
        let slots: Vec<Option<ItemStack>> = (0..hopper.len())
            .map(|i| c.get(hopper, i).cloned())
            .collect();
        let any_room = slots.iter().any(Option::is_none);
        let partial: Vec<&ItemStack> = slots
            .iter()
            .flatten()
            .filter(|s| s.count < c.max_stack(s))
            .collect();
        if !any_room && partial.is_empty() {
            return Some(false);
        }
        let one = self.java_hopper_extract(
            [above.0, above.1, above.2],
            Direction::Down,
            any_room,
            &partial,
        )?;
        let Some(one) = one else {
            return Some(false);
        };
        if let Some(rest) = self.add_item(None, hopper, one, None) {
            // Room was checked first; should the hopper still refuse it, it goes back.
            let back = self.java_hopper_insert(
                [above.0, above.1, above.2],
                Direction::Down,
                &[Some(&rest)],
            );
            if back != Some(Some(0)) {
                log::warn!(
                    "Java mods: a hopper under {above:?} lost {} it had no room for",
                    rest.item
                );
            }
            return Some(false);
        }
        Some(true)
    }
    /// A Java mod's block in front (its block entity lives in Java), or a mod's container
    /// entity there: Fabric Transfer API's DropperBlockMixin, at the default dispense (vanilla
    /// found no container), moves one item of a slot it draws again into the block's item
    /// storage, and nothing is dispensed.
    ///
    /// The rest of Fabric's `DropperBlockMixin.hookDispense`: `getRandomSlot` once more, and
    /// `StorageUtil.move` of one item of that slot into the Java mod's storage at `front`.
    pub(super) fn java_drop(
        &mut self,
        key: Pos,
        view: &View,
        front: Pos,
        face: Direction,
    ) -> Result<()> {
        let mut slot = None;
        let mut odds = 1;
        for i in 0..view.len() {
            if self.containers().get(view, i).is_some() {
                if self.world.random.next_int_bound(odds).unwrap_or(1) == 0 {
                    slot = Some(i);
                }
                odds += 1;
            }
        }
        let Some(slot) = slot else {
            return Ok(());
        };
        let Some(stack) = self.containers().get(view, slot).cloned() else {
            return Ok(());
        };
        let moved = self.java_hopper_insert([front.0, front.1, front.2], face, &[Some(&stack)]);
        if moved == Some(Some(0)) {
            if let Some(entry) = self.slot_mut_at(view, slot) {
                *entry = Some(ItemStack {
                    count: stack.count - 1,
                    ..stack
                })
                .filter(|s| s.count > 0);
            }
            self.container_changed(key);
        }
        Ok(())
    }
}
