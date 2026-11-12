package dev.horizonlab.platform120.mixin;

import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.ResourceLocation;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** A stand-in sprite's place in its atlas (Sprites120): its UVs, its contents, its atlas. */
@Mixin(TextureAtlasSprite.class)
public interface TextureAtlasSpriteAccess {
    @Mutable @Accessor("u0") void horizon$setU0(float u);
    @Mutable @Accessor("u1") void horizon$setU1(float u);
    @Mutable @Accessor("v0") void horizon$setV0(float v);
    @Mutable @Accessor("v1") void horizon$setV1(float v);
    @Mutable @Accessor("contents") void horizon$setContents(SpriteContents contents);
    @Mutable @Accessor("atlasLocation") void horizon$setAtlas(ResourceLocation atlas);
}
