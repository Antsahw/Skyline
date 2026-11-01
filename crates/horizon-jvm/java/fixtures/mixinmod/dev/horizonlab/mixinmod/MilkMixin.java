package dev.horizonlab.mixinmod;

import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.animal.cow.Cow;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** Cows can't be used (milked): the head of Player.interactOn. */
@Mixin(Player.class)
public abstract class MilkMixin {
    @Inject(method = "interactOn", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$noMilk(Entity entity, InteractionHand hand, net.minecraft.world.phys.Vec3 location,
                                      CallbackInfoReturnable<InteractionResult> cir) {
        if (entity instanceof Cow) cir.setReturnValue(InteractionResult.FAIL);
    }
}
