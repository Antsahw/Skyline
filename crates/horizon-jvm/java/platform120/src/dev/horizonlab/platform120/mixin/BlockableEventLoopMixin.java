package dev.horizonlab.platform120.mixin;

import net.minecraft.util.thread.BlockableEventLoop;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Work handed to the headless client or the mirror server to do "on its thread"
 * (Minecraft.execute, MinecraftServer.execute): neither runs a loop of its own -- they are
 * called, never ticked by Mojang's code -- so the work runs now, on the call that asked. Nearly
 * every packet receiver hands its work on this way.
 */
@Mixin(BlockableEventLoop.class)
public abstract class BlockableEventLoopMixin {
    @Inject(method = "execute(Ljava/lang/Runnable;)V", at = @At("HEAD"), cancellable = true)
    private void horizon$now(Runnable task, CallbackInfo ci) {
        Object self = this;
        if (self == net.minecraft.client.Minecraft.getInstance()
                || self == dev.horizonlab.platform120.Mirror120.server()) {
            task.run();
            ci.cancel();
        }
    }
}
