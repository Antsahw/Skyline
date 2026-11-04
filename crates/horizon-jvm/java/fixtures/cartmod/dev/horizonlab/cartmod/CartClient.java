package dev.horizonlab.cartmod;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;
import net.minecraft.client.model.geom.ModelLayers;
import net.minecraft.client.renderer.entity.MinecartRenderer;

/** Vanilla's minecart renderer for the barrel cart. */
public final class CartClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        EntityRendererRegistry.register(CartMod.BARREL_CART, ctx -> new MinecartRenderer(ctx, ModelLayers.CHEST_MINECART));
    }
}
