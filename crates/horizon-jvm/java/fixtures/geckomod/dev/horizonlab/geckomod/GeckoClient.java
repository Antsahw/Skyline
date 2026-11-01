package dev.horizonlab.geckomod;

import com.geckolib.model.GeoModel;
import com.geckolib.renderer.GeoEntityRenderer;
import com.geckolib.renderer.base.GeoRenderState;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.rendering.v1.EntityRendererRegistry;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.resources.Identifier;

/** The bobber's GeckoLib model (geckolib/models/bobber.geo.json, animations/bobber/bobber.animation.json). */
public final class GeckoClient implements ClientModInitializer {
    @Override
    public void onInitializeClient() {
        EntityRendererRegistry.register(GeckoMod.BOBBER,
                context -> new GeoEntityRenderer<Bobber, LivingEntityRenderState>(context, new Model()));
    }

    static final class Model extends GeoModel<Bobber> {
        @Override
        public Identifier getModelResource(GeoRenderState state) {
            return Identifier.fromNamespaceAndPath("horizon_gecko", "bobber");
        }

        @Override
        public Identifier getTextureResource(GeoRenderState state) {
            return Identifier.fromNamespaceAndPath("horizon_gecko", "textures/entity/bobber.png");
        }

        @Override
        public Identifier getAnimationResource(Bobber bobber) {
            return Identifier.fromNamespaceAndPath("horizon_gecko", "bobber/bobber");
        }
    }
}
