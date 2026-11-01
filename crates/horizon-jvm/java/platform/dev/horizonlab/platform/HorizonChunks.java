package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.LevelChunk;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.levelgen.Heightmap;

/**
 * Chunks of a mirrored level, for code that reads a chunk rather than the level: 26.3's block
 * scans (BlockScanUtils, behind a sapling's "flowers nearby", a vine's spread) walk a chunk's
 * sections, and heightmaps are a chunk's. A copy of Horizon's chunk is made the first time one
 * is asked for in a call (its blocks, biomes and heightmaps at once: Natives.chunkBlocks), kept
 * for that call only (Natives.viewEpoch), and follows what the call sets. What is set on it
 * goes to the level (Horizon), as a mod's Level.setBlock does.
 */
public final class HorizonChunks {
    private HorizonChunks() {}

    private static final Map<net.minecraft.world.level.Level, Map<Long, LevelChunk>> COPIES = new IdentityHashMap<>();
    private static long epoch = Long.MIN_VALUE;

    /** Horizon's chunk cx cz of a mirrored level as a LevelChunk; null where none is loaded. */
    public static LevelChunk chunk(ServerLevel level, int cx, int cz) {
        return chunk(level, cx, cz, level);
    }

    /**
     * The same, as a chunk of {@code view} (a headless client's level of {@code level}): its
     * getLevel() is the client's, as a client's chunks are (JourneyMap maps only chunks of the
     * player's level). Its blocks are Horizon's.
     */
    public static LevelChunk chunk(ServerLevel level, int cx, int cz, net.minecraft.world.level.Level view) {
        if (level.palettedContainerFactory() == null) return null;
        synchronized (COPIES) {
            if (view != level) return held(level, view, cx, cz);
            long now = Natives.viewEpoch();
            if (now != epoch) {
                COPIES.clear();
                epoch = now;
            }
            Map<Long, LevelChunk> copies = COPIES.computeIfAbsent(view, l -> new HashMap<>());
            long key = ChunkPos.pack(cx, cz);
            LevelChunk copy = copies.get(key);
            if (copy == null && !copies.containsKey(key)) {
                copy = kept(level, cx, cz);
                copies.put(key, copy);
            }
            return copy;
        }
    }

    /** A copy made in an earlier view, by the revision of Horizon's chunk it copied. */
    private record Kept(long revision, Copy copy) {}

    /** Copies kept across views (ticks), the last 512 a level. */
    private static final Map<ServerLevel, Map<Long, Kept>> KEPT = new IdentityHashMap<>();

    /**
     * The copy of chunk cx cz: the one an earlier view made while Horizon's chunk is the same
     * (Natives.chunkRevision: a mob reads the same chunks tick after tick, and a filled copy
     * is 98k blocks), else a new one. The calls' writes Horizon hasn't taken yet are applied.
     */
    private static LevelChunk kept(ServerLevel level, int cx, int cz) {
        String dimension = HorizonServer.dimensionOf(level);
        long revision = Natives.chunkRevision(dimension, cx, cz);
        if (revision == -2) return null;
        long key = ChunkPos.pack(cx, cz);
        Map<Long, Kept> kept = KEPT.computeIfAbsent(level, l -> new java.util.LinkedHashMap<>(64, 0.75F, true) {
            @Override
            protected boolean removeEldestEntry(Map.Entry<Long, Kept> eldest) {
                return size() > 512;
            }
        });
        Kept had = revision >= 0 ? kept.get(key) : null;
        if (had != null && had.revision() == revision) {
            Map<Long, BlockState> written = HorizonServer.written(level);
            if (written != null) {
                written.forEach((at, state) -> {
                    BlockPos pos = BlockPos.of(at);
                    if (pos.getX() >> 4 == cx && pos.getZ() >> 4 == cz) had.copy().follow(pos, state);
                });
            }
            return had.copy();
        }
        LevelChunk copy = copy(level, level, cx, cz);
        if (copy instanceof Copy c && revision >= 0) kept.put(key, new Kept(revision, c));
        return copy;
    }

    /**
     * A client's chunks are kept, as a client keeps the chunks it was sent: mods hold them by
     * weak references between frames (JourneyMap's ChunkMD) and read them on their own threads,
     * where Horizon's world can't be read (no view: Natives.chunkRevision -1). One is made, and
     * filled, on a thread that can read it, and made again when Horizon's chunk changes (its
     * revision). One asked for where none was made is wanted: the client's tick makes it
     * (serveWanted).
     */
    private static final Map<net.minecraft.world.level.Level, Map<Long, Object[]>> HELD = new java.util.WeakHashMap<>();
    private static final Map<net.minecraft.world.level.Level, java.util.Set<Long>> WANTED = new java.util.WeakHashMap<>();

    private static LevelChunk held(ServerLevel level, net.minecraft.world.level.Level view, int cx, int cz) {
        Map<Long, Object[]> chunks = HELD.computeIfAbsent(view, l -> new HashMap<>());
        if (chunks.size() > 8192) chunks.clear();
        long key = ChunkPos.pack(cx, cz);
        Object[] held = chunks.get(key);
        long revision = Natives.chunkRevision(level.dimension().identifier().toString(), cx, cz);
        if (revision == -1) {
            if (held != null) return (LevelChunk) held[0];
            WANTED.computeIfAbsent(view, l -> new java.util.LinkedHashSet<>()).add(key);
            return null;
        }
        if (held != null && revision >= 0 && (long) held[1] == revision) return (LevelChunk) held[0];
        if (revision == -2) {
            chunks.remove(key);
            var byServer = HELD_BY_SERVER.get(level);
            if (byServer != null) byServer.remove(key);
            return null;
        }
        LevelChunk copy = copy(level, view, cx, cz);
        if (copy instanceof Copy c) c.ensure();
        var byServer = HELD_BY_SERVER.computeIfAbsent(level, l -> new java.util.concurrent.ConcurrentHashMap<>());
        VIEW_OF.put(level, view);
        if (copy != null) {
            chunks.put(key, new Object[] {copy, revision});
            if (byServer.size() > 8192) byServer.clear();
            byServer.put(key, copy);
        } else {
            byServer.remove(key);
        }
        return copy;
    }

    /**
     * Whether a client of {@code level} has chunk cx cz: whether Horizon has it loaded (its
     * revision), or on a thread that can't read Horizon's world, whether one is held. No copy
     * is made (VoxelMap asks this of hundreds of chunks a tick).
     */
    public static boolean has(ServerLevel level, int cx, int cz, net.minecraft.world.level.Level view) {
        long revision = Natives.chunkRevision(level.dimension().identifier().toString(), cx, cz);
        if (revision != -1) return revision != -2;
        synchronized (COPIES) {
            var chunks = HELD.get(view);
            return chunks != null && chunks.get(ChunkPos.pack(cx, cz)) instanceof Object[] held && held[0] != null;
        }
    }

    /** The held chunks by Horizon's level (what threads without a view read) and a client level of each. */
    private static final Map<ServerLevel, java.util.concurrent.ConcurrentHashMap<Long, LevelChunk>> HELD_BY_SERVER =
            new java.util.concurrent.ConcurrentHashMap<>();
    private static final Map<ServerLevel, net.minecraft.world.level.Level> VIEW_OF = new java.util.concurrent.ConcurrentHashMap<>();

    /**
     * A block as a client of {@code level} was given it (a held chunk), for a thread that can't
     * read Horizon's world; null where none is held (then wanted, made at the next client tick).
     */
    public static BlockState heldBlock(ServerLevel level, BlockPos pos) {
        var chunks = HELD_BY_SERVER.get(level);
        long key = ChunkPos.pack(pos.getX() >> 4, pos.getZ() >> 4);
        LevelChunk chunk = chunks == null ? null : chunks.get(key);
        if (chunk != null) return chunk.getBlockState(pos);
        var view = VIEW_OF.get(level);
        if (view != null) {
            synchronized (COPIES) {
                WANTED.computeIfAbsent(view, l -> new java.util.LinkedHashSet<>()).add(key);
            }
        }
        return null;
    }

    /** Makes up to {@code most} of the chunks of client levels mods asked for off the game's threads. */
    public static void serveWanted(int most) {
        java.util.List<Object[]> todo = new java.util.ArrayList<>();
        synchronized (COPIES) {
            for (var entry : WANTED.entrySet()) {
                var it = entry.getValue().iterator();
                while (it.hasNext() && todo.size() < most) {
                    todo.add(new Object[] {entry.getKey(), it.next()});
                    it.remove();
                }
            }
        }
        for (Object[] want : todo) {
            var view = (net.minecraft.world.level.Level) want[0];
            long key = (long) want[1];
            if (HorizonServer.serverSide(view) instanceof ServerLevel server) {
                chunk(server, ChunkPos.getX(key), ChunkPos.getZ(key), view);
            }
        }
    }

    /** A block a call set on a mirrored level: the copy of its chunk (if made) follows. */
    static void set(ServerLevel level, BlockPos pos, BlockState state) {
        synchronized (COPIES) {
            long key = ChunkPos.pack(pos.getX() >> 4, pos.getZ() >> 4);
            if (Natives.viewEpoch() != epoch) return;
            for (var copies : COPIES.values()) {
                if (copies.get(key) instanceof Copy c && c.server == level) c.follow(pos, state);
            }
        }
    }

    /**
     * Horizon's chunk as a LevelChunk, filled only when code reads its sections or heightmaps
     * (fill): reads of a block or a fluid go to the level (HorizonServer.blockState, whole
     * sections at a time). Mobs ask for their chunk every tick (their fluids, their paths, their
     * shadows) and only read a few blocks of it; a full copy is 98k blocks.
     */
    private static LevelChunk copy(ServerLevel level, net.minecraft.world.level.Level view, int cx, int cz) {
        if (!HorizonServer.loaded(level, cx << 4, level.getMinY(), cz << 4)) return null;
        return new Copy(level, view, new ChunkPos(cx, cz));
    }

    private static void fill(Copy chunk) {
        ServerLevel level = chunk.server;
        int cx = chunk.getPos().x(), cz = chunk.getPos().z();
        String dimension = level.dimension().identifier().toString();
        int minY = level.getMinY();
        int height = level.getHeight();
        int[] blocks = new int[height * 256];
        String[] palette = Natives.chunkBlocks(dimension, cx, cz, minY, blocks);
        if (palette == null) return;
        BlockState[] states = new BlockState[palette.length];
        for (int i = 0; i < palette.length; i++) {
            BlockState state = HorizonServer.parseState(palette[i]);
            states[i] = state != null ? state : net.minecraft.world.level.block.Blocks.AIR.defaultBlockState();
        }
        LevelChunkSection[] sections = chunk.rawSections();
        for (int i = 0; i < blocks.length; i++) {
            BlockState state = states[blocks[i]];
            if (state.isAir()) continue;
            int y = i >> 8;
            sections[y >> 4].setBlockState(i & 15, y & 15, (i >> 4) & 15, state, false);
        }
        int[] quarts = new int[height / 4 * 16];
        String[] biomes = Natives.chunkBiomes(dimension, cx, cz, minY, quarts);
        if (biomes != null) {
            var registry = level.registryAccess().lookupOrThrow(Registries.BIOME);
            @SuppressWarnings("unchecked")
            Holder<Biome>[] holders = new Holder[biomes.length];
            for (int i = 0; i < biomes.length; i++) {
                holders[i] = registry.get(ResourceKey.create(Registries.BIOME, Identifier.parse(biomes[i])))
                        .map(h -> (Holder<Biome>) h).orElse(null);
            }
            for (int s = 0; s < sections.length; s++) {
                final LevelChunkSection section = sections[s];
                section.fillBiomesFromNoise((qx, qy, qz) -> {
                    int i = (((qy - (minY >> 2)) * 4 + (qz & 3)) * 4) + (qx & 3);
                    Holder<Biome> h = i >= 0 && i < quarts.length ? holders[quarts[i]] : null;
                    return h != null ? h : section.getNoiseBiome(qx & 3, qy & 3, qz & 3);
                }, cx << 2, (minY >> 2) + s * 4, cz << 2);
            }
        }
        // The calls this copy takes from the level's own, made after the copy (writes so far).
        Map<Long, BlockState> written = HorizonServer.written(level);
        if (written != null) {
            written.forEach((at, state) -> {
                BlockPos pos = BlockPos.of(at);
                if (pos.getX() >> 4 == cx && pos.getZ() >> 4 == cz) chunk.follow(pos, state);
            });
        }
        Heightmap.primeHeightmaps(chunk, java.util.EnumSet.allOf(Heightmap.Types.class));
    }

    /**
     * A copied chunk: blocks and fluids read from the level until something reads its sections,
     * heightmaps or biomes, then filled; what is set on it goes to the level, as Level.setBlock.
     */
    private static final class Copy extends LevelChunk {
        private boolean filled;
        /**
         * Set when the constructor is done. Mixins into LevelChunk's constructor read its
         * sections (Polymer's polymer$generatePolymerBlockSet): they get the empty ones the
         * constructor made, not a fill (which a copy for one block read doesn't need).
         */
        private final boolean made;
        /** The level whose chunk this is (Horizon's); getLevel() may be a client's view of it. */
        final ServerLevel server;

        Copy(ServerLevel level, net.minecraft.world.level.Level view, ChunkPos pos) {
            super(view, pos);
            this.server = level;
            made = true;
        }

        private void ensure() {
            if (filled || !made) return;
            filled = true;
            fill(this);
        }

        LevelChunkSection[] rawSections() {
            return super.getSections();
        }

        void follow(BlockPos pos, BlockState state) {
            if (!filled) return;
            LevelChunkSection section = super.getSection(getSectionIndex(pos.getY()));
            section.setBlockState(pos.getX() & 15, pos.getY() & 15, pos.getZ() & 15, state, false);
            for (var heightmap : super.getHeightmaps()) {
                heightmap.getValue().update(pos.getX() & 15, pos.getY(), pos.getZ() & 15, state);
            }
        }

        @Override
        public BlockState getBlockState(BlockPos pos) {
            if (filled) return super.getBlockState(pos);
            return HorizonServer.blockState(server, pos);
        }

        @Override
        public net.minecraft.world.level.material.FluidState getFluidState(BlockPos pos) {
            if (filled) return super.getFluidState(pos);
            return HorizonServer.blockState(server, pos).getFluidState();
        }

        @Override
        public net.minecraft.world.level.material.FluidState getFluidState(int x, int y, int z) {
            if (filled) return super.getFluidState(x, y, z);
            return HorizonServer.blockState(server, new BlockPos(x, y, z)).getFluidState();
        }

        @Override
        public LevelChunkSection[] getSections() {
            ensure();
            return super.getSections();
        }

        @Override
        public LevelChunkSection getSection(int index) {
            ensure();
            return super.getSection(index);
        }

        @Override
        public int getHeight(Heightmap.Types type, int x, int z) {
            ensure();
            return super.getHeight(type, x, z);
        }

        @Override
        public java.util.Collection<Map.Entry<Heightmap.Types, Heightmap>> getHeightmaps() {
            ensure();
            return super.getHeightmaps();
        }

        @Override
        public Heightmap getOrCreateHeightmapUnprimed(Heightmap.Types type) {
            ensure();
            return super.getOrCreateHeightmapUnprimed(type);
        }

        /** Not empty (PathNavigationRegion's quick check): the slower, exact way follows. */
        @Override
        public boolean isYSpaceEmpty(int from, int to) {
            if (!filled) return false;
            return super.isYSpaceEmpty(from, to);
        }

        @Override
        public int getHighestSectionPosition() {
            ensure();
            return super.getHighestSectionPosition();
        }

        @Override
        public Holder<Biome> getNoiseBiome(int x, int y, int z) {
            ensure();
            return super.getNoiseBiome(x, y, z);
        }

        @Override
        public BlockState setBlockState(BlockPos pos, BlockState state, int flags) {
            BlockState old = getBlockState(pos);
            server.setBlock(pos, state, flags);
            return old == state ? null : old;
        }
    }
}
