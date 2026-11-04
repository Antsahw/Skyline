package dev.horizonlab.platform;

import com.google.gson.JsonObject;
import dev.horizonlab.bridge.Natives;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.material.FlowingFluid;

/**
 * Mods' fluids as the client draws them: once the client's models are loaded, each mod fluid's
 * baked FluidModel (FluidStateModelSet: vanilla's, with what mods add to its bake: Fabric's
 * fluid rendering registry, Create's honey and chocolate) gives its still and flowing sprites,
 * its tint and its layer. Horizon's fluid mesher draws a mod's liquid block with them; without,
 * mods' liquids were carried as lava (their template) and drawn as lava.
 */
final class HorizonFluids {
    private HorizonFluids() {}

    /** After the client's models load (HorizonClient.finishModels). */
    static void export() {
        try {
            var set = net.minecraft.client.Minecraft.getInstance().getModelManager().getFluidStateModelSet();
            JsonObject out = new JsonObject();
            for (var fluid : BuiltInRegistries.FLUID) {
                var id = BuiltInRegistries.FLUID.getKey(fluid);
                if (id.getNamespace().equals("minecraft") || !(fluid instanceof FlowingFluid flowing)) continue;
                if (fluid != flowing.getSource()) continue;
                var state = fluid.defaultFluidState();
                var block = state.createLegacyBlock();
                if (block.isAir()) continue;
                var model = set.get(state);
                if (model == null || model.stillMaterial() == null) continue;
                JsonObject o = new JsonObject();
                o.addProperty("still", model.stillMaterial().sprite().contents().name().toString());
                o.addProperty("flow", model.flowingMaterial().sprite().contents().name().toString());
                int tint = -1;
                if (model.tintSource() != null) {
                    try {
                        tint = model.tintSource().color(block);
                    } catch (Throwable t) {
                        // A tint that needs a world: drawn untinted.
                    }
                }
                o.addProperty("tint", tint);
                o.addProperty("translucent", String.valueOf(model.layer()).contains("TRANSLUCENT"));
                out.add(BuiltInRegistries.BLOCK.getKey(block.getBlock()).toString(), o);
            }
            Natives.log("Java mods: " + out.size() + " mod fluid(s) drawn with their own sprites: " + out);
            if (!out.isEmpty()) Natives.modFluids(out.toString());
        } catch (Throwable t) {
            Natives.log("Java mods: the mods' fluids' looks couldn't be read: " + t);
        }
    }
}
