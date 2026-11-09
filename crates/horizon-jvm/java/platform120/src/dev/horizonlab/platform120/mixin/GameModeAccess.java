package dev.horizonlab.platform120.mixin;

import net.minecraft.server.level.ServerPlayerGameMode;
import net.minecraft.world.level.GameType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** A mirror player's game mode set without telling clients (there are none). */
@Mixin(ServerPlayerGameMode.class)
public interface GameModeAccess {
    @Invoker("setGameModeForPlayer")
    void horizon$setGameMode(GameType mode, GameType previous);
}
