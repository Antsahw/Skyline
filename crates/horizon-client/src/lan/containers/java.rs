//! Windows of Java mods' menus, which the game doesn't run, and containers with no block.
use super::*;

impl Hub {
    /// Open a container with no block in the world (a server mod's `/enderchest`): valid
    /// wherever the guest goes.
    pub fn open_unplaced_container(
        &mut self,
        conn: u32,
        menu_kind: &str,
        title: Tag,
        slots: Slots,
        items: Vec<Option<ItemStack>>,
    ) {
        self.open_container(
            conn,
            menu_kind,
            title,
            None::<(i32, i32, i32)>,
            slots,
            items,
        );
    }

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
