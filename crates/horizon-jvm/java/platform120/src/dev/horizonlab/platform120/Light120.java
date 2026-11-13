package dev.horizonlab.platform120;

import net.minecraft.core.BlockPos;
import net.minecraft.core.SectionPos;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LightLayer;
import net.minecraft.world.level.chunk.DataLayer;
import net.minecraft.world.level.lighting.LayerLightEventListener;
import net.minecraft.world.level.lighting.LevelLightEngine;

/**
 * A mirror level's light: Horizon's (Mirror120.light), which lights its own chunks. A mob asks
 * it whether it's dark enough to spawn or bright enough to burn, a moobloom where to grow its
 * flowers; vanilla's engine would need the chunks a mirror level doesn't have.
 */
final class Light120 extends LevelLightEngine {
    private final Level level;
    private final LayerLightEventListener sky;
    private final LayerLightEventListener block;

    Light120(Level level) {
        super(chunks(level), false, false);
        this.level = level;
        this.sky = new Layer(true);
        this.block = new Layer(false);
    }

    /** What vanilla's engine lights from: here only the level (its height), and no chunks. */
    private static net.minecraft.world.level.chunk.LightChunkGetter chunks(Level level) {
        return new net.minecraft.world.level.chunk.LightChunkGetter() {
            @Override
            public net.minecraft.world.level.chunk.LightChunk getChunkForLighting(int x, int z) {
                return null;
            }

            @Override
            public net.minecraft.world.level.BlockGetter getLevel() {
                return level;
            }
        };
    }

    @Override
    public LayerLightEventListener getLayerListener(LightLayer layer) {
        return layer == LightLayer.SKY ? sky : block;
    }

    /** LevelLightEngine.getRawBrightness: the brighter of sky (less the darkening) and block. */
    @Override
    public int getRawBrightness(BlockPos pos, int darken) {
        int light = Mirror120.light(level, pos);
        return Math.max((light >> 4) - darken, light & 15);
    }

    /** Horizon lights its chunks; nothing to do here. */
    @Override
    public void checkBlock(BlockPos pos) {}

    @Override
    public boolean hasLightWork() {
        return false;
    }

    @Override
    public int runLightUpdates() {
        return 0;
    }

    @Override
    public void updateSectionStatus(SectionPos pos, boolean empty) {}

    @Override
    public void setLightEnabled(ChunkPos pos, boolean enabled) {}

    @Override
    public void propagateLightSources(ChunkPos pos) {}

    private final class Layer implements LayerLightEventListener {
        private final boolean sky;

        Layer(boolean sky) {
            this.sky = sky;
        }

        @Override
        public DataLayer getDataLayerData(SectionPos pos) {
            return null;
        }

        @Override
        public int getLightValue(BlockPos pos) {
            int light = Mirror120.light(level, pos);
            return sky ? light >> 4 : light & 15;
        }

        @Override
        public void checkBlock(BlockPos pos) {}

        @Override
        public boolean hasLightWork() {
            return false;
        }

        @Override
        public int runLightUpdates() {
            return 0;
        }

        @Override
        public void updateSectionStatus(SectionPos pos, boolean empty) {}

        @Override
        public void setLightEnabled(ChunkPos pos, boolean enabled) {}

        @Override
        public void propagateLightSources(ChunkPos pos) {}
    }
}
