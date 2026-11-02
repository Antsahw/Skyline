package dev.horizonlab.platform;

/**
 * The parent a mod's settings screen gets (Mod Menu gives its mod list): Horizon's Mods page,
 * which Horizon draws itself. Mods' screens read their parent (FCAP's and cloth-config's
 * screens keep it, some read its title or size) and go back to it on Done: shown again, it
 * closes the settings screen and the player's window is back on its Mods page.
 */
public final class HorizonModsPage extends net.minecraft.client.gui.screens.Screen {
    HorizonModsPage() {
        super(net.minecraft.network.chat.Component.empty()); // Never shown.
    }
}
