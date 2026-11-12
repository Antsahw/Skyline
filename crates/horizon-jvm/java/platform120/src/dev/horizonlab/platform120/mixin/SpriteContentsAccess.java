package dev.horizonlab.platform120.mixin;

import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A stand-in sprite's contents: its name and a 16x16 size (Sprites120). */
@Mixin(SpriteContents.class)
public interface SpriteContentsAccess {
    @Mutable @Accessor("name") void horizon$setName(ResourceLocation name);
    @Mutable @Accessor("width") void horizon$setWidth(int width);
    @Mutable @Accessor("height") void horizon$setHeight(int height);
}
