//! Java mods' crafted hooks for a guest's crafting.
use super::*;

impl Game<'_> {
    /// Java mods' crafted hooks (ResultSlot.onTake) for guest `gi`'s `takes`; what they give
    /// is sent now (the click's answer takes the inventory as the client already has it).
    pub(super) fn lan_java_crafted(
        &mut self,
        gi: usize,
        takes: &[horizon_client::menu::CraftedTake],
    ) {
        if takes.is_empty() {
            return;
        }
        let conn = self.hub_host().hub.guests[gi].conn;
        let before: Vec<_> = (5..46).map(|s| self.hub_host().hub.guests[gi].menu_slot(s)).collect();
        self.as_guest(conn, |g| g.java_crafted(takes));
        let changed: Vec<usize> = (5..46)
            .filter(|&s| self.hub_host().hub.guests[gi].menu_slot(s) != before[s - 5])
            .collect();
        self.hub_host().hub.send_inventory_slots(gi, &changed);
    }
}
