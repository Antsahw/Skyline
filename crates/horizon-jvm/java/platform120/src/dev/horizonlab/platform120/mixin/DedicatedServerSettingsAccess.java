package dev.horizonlab.platform120.mixin;

import net.minecraft.server.dedicated.DedicatedServerProperties;
import net.minecraft.server.dedicated.DedicatedServerSettings;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The mirror server's settings, made from no file at all (DedicatedServerAccess). */
@Mixin(DedicatedServerSettings.class)
public interface DedicatedServerSettingsAccess {
    @Accessor("properties")
    void horizon$setProperties(DedicatedServerProperties properties);
}
