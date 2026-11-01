package dev.horizonlab.mixinmod;

import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.Items;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A player holding a stick can't get off (the head of Entity.removeVehicle). */
@Mixin(Entity.class)
public abstract class DismountMixin {
    @Inject(method = "removeVehicle", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$stickToIt(CallbackInfo ci) {
        if ((Object) this instanceof Player player && player.getVehicle() != null && player.getMainHandItem().is(Items.STICK)) ci.cancel();
    }
}
