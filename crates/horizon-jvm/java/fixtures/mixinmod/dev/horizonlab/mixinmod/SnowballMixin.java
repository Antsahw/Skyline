package dev.horizonlab.mixinmod;

import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.SnowballItem;
import net.minecraft.world.level.Level;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Snowballs can't be thrown (the head of SnowballItem.use). */
@Mixin(SnowballItem.class)
public abstract class SnowballMixin {
    @Inject(method = "use", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$noThrow(Level level, Player player, InteractionHand hand, CallbackInfoReturnable<InteractionResult> cir) {
        cir.setReturnValue(InteractionResult.FAIL);
    }
}
