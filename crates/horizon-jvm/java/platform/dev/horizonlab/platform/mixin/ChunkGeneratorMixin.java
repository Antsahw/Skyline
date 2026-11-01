package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonWorldgen;
import net.minecraft.world.level.chunk.ChunkGenerator;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

/**
 * ChunkGenerator.createBiomes's hop to vanilla's background executor: on the Horizon worker
 * that generates the chunk instead, when HorizonWorldgen.executor says so (its threads are
 * behind the server's and the window's, the executor's aren't).
 */
@Mixin(ChunkGenerator.class)
public abstract class ChunkGeneratorMixin {
    @Redirect(method = "createBiomes", require = 0,
            at = @At(value = "INVOKE", target = "Lnet/minecraft/TracingExecutor;forName(Ljava/lang/String;)Ljava/util/concurrent/Executor;"))
    private java.util.concurrent.Executor horizon$biomesExecutor(net.minecraft.TracingExecutor executor, String name) {
        return HorizonWorldgen.executor(executor, name);
    }
}
