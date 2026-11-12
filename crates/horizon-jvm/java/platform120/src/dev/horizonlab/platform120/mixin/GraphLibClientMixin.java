package dev.horizonlab.platform120.mixin;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * GraphLib's client graphs, while the headless client has no world (it never has one: Horizon
 * draws the world). GraphLib tells each player's client of every graph merge and split, for its
 * debug rendering; the client half here receives them (Packets120), asks for its world's graph
 * storage, and GraphLib logged a stack trace each time that there was none. With no world, the
 * answer is none, quietly, as GraphLib already handles (it drops the packet).
 */
@Pseudo
@Mixin(targets = "com.kneelawk.graphlib.impl.client.ClientProxy", remap = false)
public abstract class GraphLibClientMixin {
    @Inject(method = "getClientStorage", at = @At("HEAD"), cancellable = true, require = 0)
    private static void horizon$noWorld(CallbackInfoReturnable<Object> cir) {
        if (net.minecraft.client.Minecraft.getInstance().level == null) cir.setReturnValue(null);
    }
}
