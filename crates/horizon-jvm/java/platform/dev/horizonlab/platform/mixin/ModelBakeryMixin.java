package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonLazyModels;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.util.thread.ParallelMapTransform;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Redirect;

import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import java.util.function.BiFunction;

/**
 * Block state and item models baked on first use when the pack's don't fit in the heap
 * (HorizonLazyModels): bakeModels' two parallel bakes become lazy maps.
 */
@Mixin(ModelBakery.class)
public abstract class ModelBakeryMixin {
    @Redirect(method = "bakeModels", at = @At(value = "INVOKE",
            target = "Lnet/minecraft/util/thread/ParallelMapTransform;schedule(Ljava/util/Map;Ljava/util/function/BiFunction;Ljava/util/concurrent/Executor;)Ljava/util/concurrent/CompletableFuture;"))
    private <K, U, V> CompletableFuture<Map<K, V>> horizon$lazy(Map<K, U> unbaked, BiFunction<K, U, V> bake, Executor executor) {
        if (!HorizonLazyModels.on) return ParallelMapTransform.schedule(unbaked, bake, executor);
        return CompletableFuture.completedFuture(new HorizonLazyModels<>(unbaked, bake));
    }
}
