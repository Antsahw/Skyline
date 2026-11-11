package dev.horizonlab.platform120.mixin;

import java.util.function.Function;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/** The headless client's texture atlases: names only, for Horizon to draw from its own. */
@Mixin(Minecraft.class)
public abstract class MinecraftMixin {
    @Inject(method = "getTextureAtlas(Lnet/minecraft/resources/ResourceLocation;)Ljava/util/function/Function;",
            at = @At("HEAD"), cancellable = true)
    private void horizon$atlas(ResourceLocation atlas,
            CallbackInfoReturnable<Function<ResourceLocation, TextureAtlasSprite>> cir) {
        cir.setReturnValue(dev.horizonlab.platform120.Sprites120.atlas(atlas));
    }
}
