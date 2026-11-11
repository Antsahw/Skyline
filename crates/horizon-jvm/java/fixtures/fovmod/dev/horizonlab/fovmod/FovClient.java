package dev.horizonlab.fovmod;

import com.mojang.blaze3d.platform.InputConstants;
import net.fabricmc.api.ClientModInitializer;
import net.minecraft.client.KeyMapping;
import net.minecraft.resources.Identifier;

/**
 * The zoom key: Z, in a category of its own, made as a plain KeyMapping (KeyMapping.ALL only,
 * not Fabric's KeyMappingHelper list, as some mods do).
 */
public class FovClient implements ClientModInitializer {
    public static KeyMapping zoom;

    @Override
    public void onInitializeClient() {
        var category = KeyMapping.Category.register(Identifier.fromNamespaceAndPath("horizon_fov", "main"));
        zoom = new KeyMapping("key.horizon_fov.zoom", InputConstants.KEY_Z, category);
    }
}
