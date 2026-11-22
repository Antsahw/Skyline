package dev.horizonlab.platform120.mixin;

import net.minecraft.network.Connection;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * A mirror player's connection has a netty channel with nothing on the other end (Mirror120's
 * `player`): a loader asking it whether the client has a mod's channel (Forge's
 * `SimpleChannel.isRemotePresent` reads the channel's attributes) gets "no" rather than a null.
 */
@Mixin(Connection.class)
public interface ConnectionAccess {
    @Accessor("channel")
    void horizon$setChannel(io.netty.channel.Channel channel);
}
