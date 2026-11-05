package dev.horizonlab.kineticmod.mixin;

import com.llamalad7.mixinextras.sugar.Local;
import net.minecraft.client.Minecraft;
import net.minecraft.client.MouseHandler;
import net.minecraft.world.item.Items;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Create's MouseHandlerMixin.onMouseScroll: the wheel while playing, before the hotbar takes
 * it (at its getInventory call, the scaled offset as a local); a held stick (Create: a
 * schematic tool, a train's throttle) takes the turn.
 */
@Mixin(MouseHandler.class)
public class ScrollMixin {
    @Shadow
    @Final
    private Minecraft minecraft;

    @Inject(method = "onScroll(JDD)V", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/client/player/LocalPlayer;getInventory()Lnet/minecraft/world/entity/player/Inventory;"),
            cancellable = true)
    private void horizonKinetic$scroll(long handle, double x, double y, CallbackInfo ci, @Local(ordinal = 4) double scaled) {
        boolean stick = minecraft.player != null && minecraft.player.getMainHandItem().is(Items.STICK);
        System.out.println("horizon_kinetic: scroll " + scaled + (stick ? " taken" : " passed"));
        if (stick) ci.cancel();
    }
}
