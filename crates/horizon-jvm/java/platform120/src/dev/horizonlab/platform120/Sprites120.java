package dev.horizonlab.platform120;

import java.util.IdentityHashMap;
import java.util.HashMap;
import java.util.Map;
import java.util.function.Function;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.resources.ResourceLocation;

/**
 * Sprites of the texture atlases, for a client that has none. A real atlas is built when the
 * client loads its resources, which this runtime does not do: Horizon has its own atlases and
 * draws a sprite by its name. So a sprite here is a stand-in that only knows its name, and the
 * drawing of one is recorded as that name (GuiGraphicsMixin) for Horizon to find in its own.
 *
 * <p>An empty slot's outline is one: Farmer's Delight's cooking pot shows a bowl where the
 * bowl goes, and vanilla fetches it from the blocks atlas as it lays the slot out.
 */
public final class Sprites120 {
    private Sprites120() {}

    private static final Map<TextureAtlasSprite, ResourceLocation> NAMES = new IdentityHashMap<>();
    private static final Map<ResourceLocation, TextureAtlasSprite> MADE = new HashMap<>();
    private static final java.util.List<ResourceLocation> SLOTS = new java.util.ArrayList<>();

    /** The sprite of a slot (a U's whole part), or null. */
    static synchronized ResourceLocation slot(int slot) {
        return slot >= 0 && slot < SLOTS.size() ? SLOTS.get(slot) : null;
    }

    /** A material's stand-in sprite (`ModelBakery.bakeModels`' getter). */
    static TextureAtlasSprite sprite(net.minecraft.client.resources.model.Material material) {
        return atlas(material.atlasLocation()).apply(material.texture());
    }

    /** An atlas: a sprite for each name asked of it. `Minecraft.getTextureAtlas`. */
    public static synchronized Function<ResourceLocation, TextureAtlasSprite> atlas(ResourceLocation atlas) {
        return name -> {
            synchronized (Sprites120.class) {
                return MADE.computeIfAbsent(name, n -> {
                    TextureAtlasSprite sprite = Allocate.make(TextureAtlasSprite.class);
                    // Its own slot of a wide atlas: U from its index to the next, V 0 to 1, so
                    // a quad's UVs say which sprite it is drawn from and where in it (Models120).
                    int slot = SLOTS.size();
                    SLOTS.add(n);
                    var access = (dev.horizonlab.platform120.mixin.TextureAtlasSpriteAccess) sprite;
                    access.horizon$setU0(slot);
                    access.horizon$setU1(slot + 1);
                    access.horizon$setV0(0);
                    access.horizon$setV1(1);
                    access.horizon$setAtlas(atlas);
                    var contents = Allocate.make(net.minecraft.client.renderer.texture.SpriteContents.class);
                    var contentsAccess = (dev.horizonlab.platform120.mixin.SpriteContentsAccess) contents;
                    contentsAccess.horizon$setName(n);
                    contentsAccess.horizon$setWidth(16);
                    contentsAccess.horizon$setHeight(16);
                    access.horizon$setContents(contents);
                    NAMES.put(sprite, n);
                    return sprite;
                });
            }
        };
    }

    /**
     * The texture a sprite is drawn from, as Horizon names textures (`ns:textures/path.png`),
     * or null for a sprite this class did not make.
     */
    public static synchronized String texture(TextureAtlasSprite sprite) {
        ResourceLocation name = NAMES.get(sprite);
        return name == null ? null : name.getNamespace() + ":textures/" + name.getPath() + ".png";
    }
}
