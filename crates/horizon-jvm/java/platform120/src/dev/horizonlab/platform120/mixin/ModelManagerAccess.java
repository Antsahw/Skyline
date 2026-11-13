package dev.horizonlab.platform120.mixin;

import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.Executor;
import net.minecraft.client.renderer.block.model.BlockModel;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.client.resources.model.ModelManager;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.packs.resources.ResourceManager;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

/** ModelManager's own loaders of the block models and blockstates (Models120). */
@Mixin(ModelManager.class)
public interface ModelManagerAccess {
    @Invoker("loadBlockModels")
    static CompletableFuture<Map<ResourceLocation, BlockModel>> horizon$loadBlockModels(ResourceManager resources,
            Executor executor) {
        throw new AssertionError();
    }

    @Invoker("loadBlockStates")
    static CompletableFuture<Map<ResourceLocation, List<ModelBakery.LoadedJson>>> horizon$loadBlockStates(
            ResourceManager resources, Executor executor) {
        throw new AssertionError();
    }

    /** A ModelManager made without its constructor gets its models here (Render120's items). */
    @org.spongepowered.asm.mixin.gen.Accessor("bakedRegistry")
    void horizon$setBakedRegistry(Map<ResourceLocation, net.minecraft.client.resources.model.BakedModel> models);

    @org.spongepowered.asm.mixin.gen.Accessor("missingModel")
    void horizon$setMissingModel(net.minecraft.client.resources.model.BakedModel model);
}
