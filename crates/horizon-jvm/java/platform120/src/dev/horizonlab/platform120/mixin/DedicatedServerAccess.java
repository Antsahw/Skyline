package dev.horizonlab.platform120.mixin;

import net.minecraft.server.dedicated.DedicatedServer;
import net.minecraft.server.dedicated.DedicatedServerSettings;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The mirror server's server.properties: vanilla's defaults (a file that isn't there reads as
 * them, and nothing is written). Mods read them when the server starts (Supplementaries).
 */
@Mixin(DedicatedServer.class)
public interface DedicatedServerAccess {
    @Mutable
    @Accessor("settings")
    void horizon$setSettings(DedicatedServerSettings settings);
}
