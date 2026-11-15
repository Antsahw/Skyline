package dev.horizonlab.platform120.mixin;

import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.AbstractClientPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The client player's own client level (Screens120.clientPlayer), which its constructor sets. */
@Mixin(AbstractClientPlayer.class)
public interface AbstractClientPlayerAccess {
    @Mutable
    @Accessor("clientLevel")
    void horizon$setClientLevel(ClientLevel level);
}
