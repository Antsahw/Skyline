package dev.horizonlab.platform120.mixin;

import java.util.Map;
import net.minecraft.client.renderer.entity.EntityRendererProvider;
import net.minecraft.client.renderer.entity.EntityRenderers;
import net.minecraft.world.entity.EntityType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The entity renderers' providers, vanilla's and those Fabric's registry put there (Render120). */
@Mixin(EntityRenderers.class)
public interface EntityRenderersAccess {
    @Accessor("PROVIDERS")
    static Map<EntityType<?>, EntityRendererProvider<?>> horizon$providers() {
        throw new AssertionError();
    }
}
