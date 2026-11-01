package dev.horizonlab.mixinmod;

import net.minecraft.world.entity.player.Inventory;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** The ninth hotbar slot can't be picked (the head of Inventory.setSelectedSlot). */
@Mixin(Inventory.class)
public abstract class SlotMixin {
    @Inject(method = "setSelectedSlot", at = @At("HEAD"), cancellable = true)
    private void horizonMixins$noNinth(int slot, CallbackInfo ci) {
        if (slot == 8) ci.cancel();
    }
}
