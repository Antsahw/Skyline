//! Windows of Java mods' menus, which the game doesn't run.
use super::*;

impl Hub {
    /// The slots of a window the game doesn't run (a Java mod's menu, its items from Java):
    /// ClientboundContainerSetContentPacket, the whole window and the carried item.
    pub fn send_window_content(
        &mut self,
        conn: u32,
        window: i32,
        items: Vec<Option<ItemStack>>,
        carried: Option<ItemStack>,
    ) {
        let net: Vec<_> = items.iter().map(|s| self.stack(s)).collect();
        let carried = self.stack(&carried);
        let Some(g) = self.guests.iter_mut().find(|g| g.conn == conn) else {
            return;
        };
        g.java_window_state = (g.java_window_state + 1) & 0x7FFF;
        g.send(&C::ContainerSetContent(play::ContainerSetContent {
            container: window,
            state: g.java_window_state,
            items: net,
            carried,
        }));
    }

    /// Closes a window the game doesn't run (ClientboundContainerClosePacket).
    pub fn close_window(&mut self, conn: u32, window: i32) {
        self.send_to(
            conn,
            &C::ContainerClose(play::ContainerClose { container: window }),
        );
    }
}
