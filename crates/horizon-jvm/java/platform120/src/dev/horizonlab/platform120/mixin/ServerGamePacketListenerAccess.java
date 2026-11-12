package dev.horizonlab.platform120.mixin;

import net.minecraft.network.Connection;
import net.minecraft.server.network.ServerGamePacketListenerImpl;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * A mirror player's network connection: one with no channel behind it, for the mods that ask
 * it questions (Porting Lib asks whether it is an in-memory one, as single player's is: no).
 * Nothing is sent through it; packets go the mods' own way (Packets120).
 */
@Mixin(ServerGamePacketListenerImpl.class)
public interface ServerGamePacketListenerAccess {
    @Mutable
    @Accessor("connection")
    void horizon$setConnection(Connection connection);
}
