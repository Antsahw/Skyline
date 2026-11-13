package dev.horizonlab.platform120.mixin;

import java.util.Map;
import net.minecraft.client.renderer.blockentity.BlockEntityRendererProvider;
import net.minecraft.client.renderer.blockentity.BlockEntityRenderers;
import net.minecraft.world.level.block.entity.BlockEntityType;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The block entity renderers' providers, vanilla's and the mods' (Render120). */
@Mixin(BlockEntityRenderers.class)
public interface BlockEntityRenderersAccess {
    @Accessor("PROVIDERS")
    static Map<BlockEntityType<?>, BlockEntityRendererProvider<?>> horizon$providers() {
        throw new AssertionError();
    }
}
