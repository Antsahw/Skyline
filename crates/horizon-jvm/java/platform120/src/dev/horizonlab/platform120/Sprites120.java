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

    /** An atlas: a sprite for each name asked of it. `Minecraft.getTextureAtlas`. */
    public static synchronized Function<ResourceLocation, TextureAtlasSprite> atlas(ResourceLocation atlas) {
        return name -> {
            synchronized (Sprites120.class) {
                return MADE.computeIfAbsent(name, n -> {
                    TextureAtlasSprite sprite = Allocate.make(TextureAtlasSprite.class);
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
