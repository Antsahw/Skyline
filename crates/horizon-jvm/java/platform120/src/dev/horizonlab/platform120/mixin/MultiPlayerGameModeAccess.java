package dev.horizonlab.platform120.mixin;

import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.world.level.GameType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The HUD's stand-in client game mode (Hud120): the player's mode, which its reach asks. */
@Mixin(MultiPlayerGameMode.class)
public interface MultiPlayerGameModeAccess {
    @Accessor("localPlayerMode")
    void horizon$setMode(GameType mode);

    @org.spongepowered.asm.mixin.Mutable
    @Accessor("minecraft")
    void horizon$setMinecraft(net.minecraft.client.Minecraft minecraft);

    @Accessor("destroyBlockPos")
    void horizon$setDestroyBlockPos(net.minecraft.core.BlockPos pos);

    @Accessor("destroyingItem")
    void horizon$setDestroyingItem(net.minecraft.world.item.ItemStack stack);

    @Accessor("previousLocalPlayerMode")
    void horizon$setPreviousMode(GameType mode);
}
