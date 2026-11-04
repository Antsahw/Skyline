package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonEntityRender;
import net.minecraft.client.renderer.item.ItemModelResolver;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.item.ItemStack;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The headless client's item models: it has none (no model manager; HorizonEntityRender's
 * context makes this resolver without one). The items an entity renderer asks for (held,
 * worn) are kept on their render state instead, whose submit hands them to
 * HorizonEntityRender (ItemStackRenderStateMixin) for Horizon's item renderer to draw.
 */
@Mixin(ItemModelResolver.class)
public abstract class ItemModelResolverMixin {
    @Shadow
    @Final
    private net.minecraft.client.resources.model.ModelManager modelManager;

    @Inject(method = "updateForLiving", at = @At("HEAD"), cancellable = true)
    private void horizon$living(ItemStackRenderState state, ItemStack stack, ItemDisplayContext context,
                                net.minecraft.world.entity.LivingEntity owner, CallbackInfo ci) {
        var real = horizon$real(stack);
        if (real != null) {
            real.updateForLiving(state, stack, context, owner);
            ci.cancel();
            return;
        }
        horizon$keep(state, stack, context, ci);
    }

    @Inject(method = "updateForNonLiving", at = @At("HEAD"), cancellable = true)
    private void horizon$nonLiving(ItemStackRenderState state, ItemStack stack, ItemDisplayContext context,
                                   net.minecraft.world.entity.Entity owner, CallbackInfo ci) {
        var real = horizon$real(stack);
        if (real != null) {
            real.updateForNonLiving(state, stack, context, owner);
            ci.cancel();
            return;
        }
        horizon$keep(state, stack, context, ci);
    }

    @Inject(method = "updateForTopItem", at = @At("HEAD"), cancellable = true)
    private void horizon$top(ItemStackRenderState state, ItemStack stack, ItemDisplayContext context,
                             net.minecraft.world.level.Level level, net.minecraft.world.entity.ItemOwner owner, int seed,
                             CallbackInfo ci) {
        var real = horizon$real(stack);
        if (real != null) {
            real.updateForTopItem(state, stack, context, level, owner, seed);
            ci.cancel();
            return;
        }
        horizon$keep(state, stack, context, ci);
    }

    /**
     * An item only its mod can draw (HorizonEntityRender.modDrawn: TACZ's guns), for this
     * resolver without models: the client's real resolver, whose state's submit runs the mod's
     * renderer into the recording collector. Null for the rest (kept for Horizon to draw).
     */
    private ItemModelResolver horizon$real(ItemStack stack) {
        if (modelManager != null || !HorizonEntityRender.modDrawn(stack)) return null;
        var real = HorizonEntityRender.realItemResolver();
        return real == (Object) this ? null : real;
    }

    @Inject(method = "appendItemLayers", at = @At("HEAD"), cancellable = true)
    private void horizon$noLayers(CallbackInfo ci) {
        if (modelManager == null) ci.cancel();
    }

    private void horizon$keep(ItemStackRenderState state, ItemStack stack, ItemDisplayContext context, CallbackInfo ci) {
        if (modelManager != null) return;
        state.clear();
        HorizonEntityRender.keepItem(state, stack, context);
        ci.cancel();
    }
}
