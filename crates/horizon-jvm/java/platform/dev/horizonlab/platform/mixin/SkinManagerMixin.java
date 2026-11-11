package dev.horizonlab.platform.mixin;

import com.mojang.authlib.GameProfile;
import dev.horizonlab.platform.HorizonClient;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.resources.SkinManager;
import net.minecraft.world.entity.player.PlayerSkin;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The headless client's skins (a mod's screen draws a player's head: Chunk Loaders' owners; 3D
 * skin layers build voxels from a skin's pixels): the player's real skin (HorizonSkins: the
 * profile's {@code textures}, or the game's own offline skin), else the profile's default.
 */
@Mixin(SkinManager.class)
public abstract class SkinManagerMixin {
    @Inject(method = "get", at = @At("HEAD"), cancellable = true)
    private void horizon$skin(GameProfile profile, CallbackInfoReturnable<CompletableFuture<Optional<PlayerSkin>>> cir) {
        if (HorizonClient.headless()) cir.setReturnValue(dev.horizonlab.platform.HorizonSkins.get(profile));
    }
}
