package dev.horizonlab.mixinmod;

import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A player's jumps are counted (the end of LivingEntity.jumpFromGround, on the server). */
@Mixin(LivingEntity.class)
public abstract class JumpMixin {
    private int horizonMixins$jumps;

    @Inject(method = "jumpFromGround", at = @At("TAIL"))
    private void horizonMixins$count(CallbackInfo ci) {
        if ((Object) this instanceof ServerPlayer player) {
            player.sendSystemMessage(Component.literal("[mixinmod] jump " + ++horizonMixins$jumps));
        }
    }
}
