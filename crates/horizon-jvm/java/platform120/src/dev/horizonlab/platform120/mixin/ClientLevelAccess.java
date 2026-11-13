package dev.horizonlab.platform120.mixin;

import net.minecraft.client.multiplayer.ClientLevel;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The HUD's stand-in client level (Hud120): its level data, which the hearts ask (hardcore). */
@Mixin(ClientLevel.class)
public interface ClientLevelAccess {
    @Mutable
    @Accessor("clientLevelData")
    void horizon$setLevelData(ClientLevel.ClientLevelData data);
}
