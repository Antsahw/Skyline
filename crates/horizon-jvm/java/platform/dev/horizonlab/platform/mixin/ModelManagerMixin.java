package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonLazyModels;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.resources.model.ModelManager;
import net.minecraft.world.level.block.state.BlockState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

import java.util.Map;

/**
 * Lazily baked block models (HorizonLazyModels) stay lazy in the model set: vanilla copies
 * them into a map of every state (baking them all), this keeps the lazy map, the missing
 * model its default.
 */
@Mixin(ModelManager.class)
public abstract class ModelManagerMixin {
    @Inject(method = "createBlockStateToModelDispatch", at = @At("HEAD"), cancellable = true)
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static void horizon$lazy(Map<BlockState, BlockStateModel> models, BlockStateModel missing,
                                     CallbackInfoReturnable<Map<BlockState, BlockStateModel>> cir) {
        if (models instanceof HorizonLazyModels lazy) cir.setReturnValue(lazy.missing(missing));
    }
}
