package dev.horizonlab.platform120;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import net.minecraft.client.renderer.BiomeColors;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.world.level.BlockAndTintGetter;
import net.minecraft.world.level.ColorResolver;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.lighting.LevelLightEngine;
import net.minecraft.world.level.material.FluidState;

/**
 * The mods' block tints (the colours they register with the client's BlockColors: Fabric's
 * ColorProviderRegistry, Forge's RegisterColorHandlersEvent), for Horizon's mesher, which knows
 * vanilla's. Each block's colour for tint indices 0 and 1 is asked in a world whose every biome
 * colour is a marker, so what it reads (grass, foliage, water) is known; one that reads none is
 * a constant. As the 26.3 bridge's HorizonTints writes them.
 */
final class Tints120 {
    private Tints120() {}

    private static final int GRASS = 0x010203, FOLIAGE = 0x040506, WATER = 0x0A0B0C;

    /** {block id: [a tint index's "grass", "foliage", "water", "none" or "#rrggbb"]}. */
    static String export() {
        // The client's models (and on Fabric its BlockColors, the mods' providers in them).
        Loader120.get().baked();
        var colors = net.minecraft.client.Minecraft.getInstance().getBlockColors();
        JsonObject out = new JsonObject();
        if (colors == null) return out.toString();
        for (var block : BuiltInRegistries.BLOCK) {
            var id = BuiltInRegistries.BLOCK.getKey(block);
            if ("minecraft".equals(id.getNamespace())) continue;
            BlockState state = block.defaultBlockState();
            String[] kinds = new String[2];
            int last = -1;
            for (int index = 0; index < kinds.length; index++) {
                kinds[index] = kind(colors, state, index);
                if (!kinds[index].equals("none")) last = index;
            }
            if (last < 0) continue;
            JsonArray layers = new JsonArray();
            for (int index = 0; index <= last; index++) layers.add(kinds[index]);
            out.add(id.toString(), layers);
        }
        return out.toString();
    }

    /**
     * The mods' item tints (their ItemColors: a leaves item's foliage green): {item id: [a tint
     * index's ARGB, -1 for none]}, those with any. Spawn eggs are left to Mirror120.spawnEggs.
     */
    static String items() {
        Loader120.get().baked();
        var client = net.minecraft.client.Minecraft.getInstance();
        var colors = ((dev.horizonlab.platform120.mixin.MinecraftAccess) client).horizon$itemColors();
        if (colors == null && client.getBlockColors() != null) {
            colors = net.minecraft.client.color.item.ItemColors.createDefault(client.getBlockColors());
        }
        JsonObject out = new JsonObject();
        if (colors == null) return out.toString();
        for (var item : BuiltInRegistries.ITEM) {
            var id = BuiltInRegistries.ITEM.getKey(item);
            if ("minecraft".equals(id.getNamespace()) || item instanceof net.minecraft.world.item.SpawnEggItem) continue;
            var stack = new net.minecraft.world.item.ItemStack(item);
            int[] tints = new int[2];
            int last = -1;
            for (int index = 0; index < tints.length; index++) {
                try {
                    tints[index] = colors.getColor(stack, index);
                } catch (RuntimeException | LinkageError e) {
                    tints[index] = -1;
                }
                if (tints[index] != -1) last = index;
            }
            if (last < 0) continue;
            JsonArray layers = new JsonArray();
            for (int index = 0; index <= last; index++) layers.add(tints[index]);
            out.add(id.toString(), layers);
        }
        return out.toString();
    }

    private static String kind(net.minecraft.client.color.block.BlockColors colors, BlockState state, int index) {
        int color;
        try {
            color = colors.getColor(state, new Markers(state), BlockPos.ZERO, index);
        } catch (RuntimeException | LinkageError e) {
            return "none";
        }
        if (color == -1) return "none";
        return switch (color & 0xFFFFFF) {
            case GRASS -> "grass";
            case FOLIAGE -> "foliage";
            case WATER -> "water";
            default -> String.format("#%06x", color & 0xFFFFFF);
        };
    }

    /** A world of `state` everywhere whose biome colours are the markers. */
    private record Markers(BlockState state) implements BlockAndTintGetter {
        @Override public float getShade(Direction side, boolean shade) { return 1.0f; }
        @Override public LevelLightEngine getLightEngine() { return null; }
        @Override public int getBrightness(net.minecraft.world.level.LightLayer layer, BlockPos pos) { return 15; }
        @Override public int getRawBrightness(BlockPos pos, int dark) { return 15; }
        @Override public boolean canSeeSky(BlockPos pos) { return true; }
        @Override public int getBlockTint(BlockPos pos, ColorResolver resolver) {
            if (resolver == BiomeColors.GRASS_COLOR_RESOLVER) return GRASS;
            if (resolver == BiomeColors.FOLIAGE_COLOR_RESOLVER) return FOLIAGE;
            if (resolver == BiomeColors.WATER_COLOR_RESOLVER) return WATER;
            return 0xFFFFFF;
        }
        @Override public BlockEntity getBlockEntity(BlockPos pos) { return null; }
        @Override public BlockState getBlockState(BlockPos pos) { return state; }
        @Override public FluidState getFluidState(BlockPos pos) { return state.getFluidState(); }
        @Override public int getHeight() { return 384; }
        @Override public int getMinBuildHeight() { return -64; }
    }
}
