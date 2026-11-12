package dev.horizonlab.platform120;

import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;

/**
 * A mirror level's biome manager (LevelMixin.getBiomeManager): every biome is Horizon's, at
 * the block asked ({@code LevelReader.getBiome} and the rest go through here), so Horizon's own
 * zoom from its biome cells is the one used.
 */
final class Biomes120 extends BiomeManager {
    private final Level level;

    Biomes120(Level level) {
        super((x, y, z) -> Mirror120.biome(level, new BlockPos(x << 2, y << 2, z << 2)), 0L);
        this.level = level;
    }

    @Override
    public Holder<Biome> getBiome(BlockPos pos) {
        return Mirror120.biome(level, pos);
    }
}
