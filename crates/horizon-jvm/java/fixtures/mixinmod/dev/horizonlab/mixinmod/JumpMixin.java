package dev.horizonlab.mixinmod;

import net.minecraft.network.chat.Component;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.item.Items;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A player's jumps are counted (the end of LivingEntity.jumpFromGround, on the server); a
 * player holding a slime ball may not jump (its head cancels).
 */
@Mixin(LivingEntity.class)
public abstract class JumpMixin {
    private int horizonMixins$jumps, horizonMixins$refused;

    @Inject(method = "jumpFromGround", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$refuse(CallbackInfo ci) {
        if ((Object) this instanceof ServerPlayer player && player.getMainHandItem().is(Items.SLIME_BALL)) {
            horizonMixins$refused++;
            player.sendSystemMessage(Component.literal("[mixinmod] no jumping"));
            ci.cancel();
        }
    }

    @Inject(method = "jumpFromGround", at = @At("TAIL"))
    private void horizonMixins$count(CallbackInfo ci) {
        if ((Object) this instanceof ServerPlayer player) {
            player.sendSystemMessage(Component.literal("[mixinmod] jump " + ++horizonMixins$jumps
                    + (horizonMixins$refused > 0 ? " after " + horizonMixins$refused + " refused" : "")));
        }
    }
}
