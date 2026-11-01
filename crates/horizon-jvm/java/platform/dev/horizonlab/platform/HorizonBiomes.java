package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeResolver;

/**
 * A mirrored level's biomes as Level.getBiome reads them (its BiomeManager's resolver): those
 * Horizon's chunks hold (Natives.chunkBiomes), kept per chunk, as vanilla's level reads the
 * biomes its chunks store. Where Horizon has no chunk (or outside a call's view), the biome
 * source's own noise. The noise was asked on every read before: Promenade's movement mixin
 * asks every living entity's biome each tick, a climate sample each (a tenth of a whale's tick).
 */
final class HorizonBiomes implements BiomeResolver {
    private final ServerLevel level;
    private final BiomeResolver noise;
    private final String dimension;
    private final int minQuart;
    private final int quarts;
    /** Each chunk's biomes by quart ((y - min) * 16 + z * 4 + x), the last 4096 chunks. */
    private final ConcurrentHashMap<Long, Holder<Biome>[]> chunks = new ConcurrentHashMap<>();

    HorizonBiomes(ServerLevel level, BiomeResolver noise) {
        this.level = level;
        this.noise = noise;
        this.dimension = level.dimension().identifier().toString();
        this.minQuart = level.getMinY() >> 2;
        this.quarts = level.getHeight() >> 2;
    }

    @Override
    public Holder<Biome> getNoiseBiome(int qx, int qy, int qz) {
        int y = qy - minQuart;
        if (y >= 0 && y < quarts) {
            Holder<Biome>[] biomes = chunk(qx >> 2, qz >> 2);
            if (biomes != null) {
                Holder<Biome> biome = biomes[(y * 4 + (qz & 3)) * 4 + (qx & 3)];
                if (biome != null) return biome;
            }
        }
        return noise.getNoiseBiome(qx, qy, qz);
    }

    private Holder<Biome>[] chunk(int cx, int cz) {
        long key = net.minecraft.world.level.ChunkPos.pack(cx, cz);
        Holder<Biome>[] had = chunks.get(key);
        if (had != null) return had;
        int[] out = new int[quarts * 16];
        String[] palette;
        try {
            palette = Natives.chunkBiomes(dimension, cx, cz, level.getMinY(), out);
        } catch (Throwable t) {
            return null;
        }
        if (palette == null) return null;
        var registry = level.registryAccess().lookupOrThrow(Registries.BIOME);
        @SuppressWarnings("unchecked")
        Holder<Biome>[] byIndex = new Holder[palette.length];
        for (int i = 0; i < palette.length; i++) {
            byIndex[i] = registry.get(ResourceKey.create(Registries.BIOME, Identifier.parse(palette[i])))
                    .map(h -> (Holder<Biome>) h).orElse(null);
        }
        @SuppressWarnings("unchecked")
        Holder<Biome>[] biomes = new Holder[out.length];
        for (int i = 0; i < out.length; i++) {
            int b = out[i];
            biomes[i] = b >= 0 && b < byIndex.length ? byIndex[b] : null;
        }
        if (chunks.size() > 4096) chunks.clear();
        chunks.put(key, biomes);
        return biomes;
    }
}
