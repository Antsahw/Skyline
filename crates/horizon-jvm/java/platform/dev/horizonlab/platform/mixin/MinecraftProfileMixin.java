package dev.horizonlab.platform.mixin;

import com.mojang.authlib.GameProfile;
import dev.horizonlab.platform.HorizonClient;
import net.minecraft.client.Minecraft;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The headless client's signed-in profile (Minecraft.getGameProfile: the user's, fetched at
 * its start) is the profile of the player whose client runs now: one client serves every
 * player (Flashback names the recording's player by it).
 */
@Mixin(Minecraft.class)
public abstract class MinecraftProfileMixin {
    @Inject(method = "getGameProfile", at = @At("HEAD"), cancellable = true)
    private void horizon$profile(CallbackInfoReturnable<GameProfile> cir) {
        var self = (Minecraft) (Object) this;
        if (HorizonClient.headless() && self.player != null) cir.setReturnValue(self.player.getGameProfile());
    }
}
