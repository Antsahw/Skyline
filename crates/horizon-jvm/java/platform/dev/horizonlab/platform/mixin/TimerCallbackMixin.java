package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonFunctions;
import net.minecraft.server.MinecraftServer;
import net.minecraft.world.level.timers.FunctionCallback;
import net.minecraft.world.level.timers.FunctionTagCallback;
import net.minecraft.world.level.timers.TimerCallback;
import net.minecraft.world.level.timers.TimerQueue;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** A scheduled function stopped for a chunk (HorizonFunctions.forced) runs again next tick. */
@Mixin({FunctionCallback.class, FunctionTagCallback.class})
public abstract class TimerCallbackMixin {
    @Inject(method = "handle(Lnet/minecraft/server/MinecraftServer;Lnet/minecraft/world/level/timers/TimerQueue;J)V",
            at = @At("TAIL"))
    @SuppressWarnings("unchecked")
    private void horizon$retry(MinecraftServer server, TimerQueue<MinecraftServer> queue, long time, CallbackInfo ci) {
        if (!HorizonFunctions.takeAborted()) return;
        queue.schedule("horizon-retry-" + System.identityHashCode(this) + "-" + time, time + 1,
                (TimerCallback<MinecraftServer>) (Object) this);
    }
}
