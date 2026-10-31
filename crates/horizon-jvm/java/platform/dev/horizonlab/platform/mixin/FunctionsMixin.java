package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonFunctions;
import net.minecraft.commands.execution.CommandQueueEntry;
import net.minecraft.commands.execution.ExecutionContext;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * A mods' function run stopped by a forced chunk Horizon hasn't loaded yet
 * (HorizonFunctions.forced): the commands left in its queue are dropped, so nothing after the
 * /forceload runs on a chunk that isn't there; the run is retried whole.
 */
@Mixin(CommandQueueEntry.class)
public abstract class FunctionsMixin {
    @Inject(method = "execute", at = @At("HEAD"), cancellable = true)
    private void horizon$stopped(ExecutionContext<?> context, CallbackInfo ci) {
        if (HorizonFunctions.TRACE && context.tracer() == null) context.tracer(HorizonFunctions.tracer());
        if (!HorizonFunctions.aborting()) return;
        ci.cancel();
        context.discardAtDepthOrHigher(0);
    }
}
