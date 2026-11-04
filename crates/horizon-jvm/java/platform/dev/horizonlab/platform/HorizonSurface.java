package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.invoke.MethodHandle;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.util.EnumSet;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.atomic.AtomicLong;
import java.util.function.Function;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.world.level.biome.Biome;
import net.minecraft.world.level.biome.BiomeManager;
import net.minecraft.world.level.biome.BiomeResolver;
import net.minecraft.world.level.biome.Biomes;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.chunk.BlockColumn;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.LevelChunkSection;
import net.minecraft.world.level.chunk.ProtoChunk;
import net.minecraft.world.level.chunk.status.ChunkStatus;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.level.levelgen.Heightmap;
import net.minecraft.world.level.levelgen.NoiseChunk;
import net.minecraft.world.level.levelgen.RandomState;
import net.minecraft.world.level.levelgen.WorldGenerationContext;
import net.minecraft.world.level.levelgen.densityfunction.DensitySamplerSet;
import net.minecraft.world.level.levelgen.densityfunction.DensityVolume;
import net.minecraft.world.level.levelgen.material.MaterialRuleContext;
import net.minecraft.world.level.levelgen.material.MaterialSystem;
import net.minecraft.world.level.levelgen.material.rule.MaterialRule;
import net.minecraft.world.level.levelgen.material.rule.RuleEvaluator;

/**
 * MaterialSystem.buildSurface (the surface rules' walk over every stone block of a chunk's
 * columns) for the mods' worldgen, the same blocks for less work:
 * <ul>
 * <li>blocks are read from and written to the chunk's sections directly (ProtoChunk's
 * getBlockState and setBlockState checked the height, found the section and, for a write,
 * took the section's lock and updated both world-generation heightmaps every time);</li>
 * <li>a write of the state already there is skipped (its heightmaps are already right);</li>
 * <li>a block's biome (BiomeManager.getBiome: eight corners' seeded hashes, then the nearest's
 * noise biome) is the corners' biome without hashing when all eight corners have the same,
 * and stays while the corners stay (the column's next block down).</li>
 * </ul>
 * The rules themselves are the mods' and vanilla's, compiled and run as vanilla does.
 * HORIZON_JAVA_SURFACE=vanilla turns it off; HORIZON_JAVA_SURFACE_CHECK=1 runs both on every
 * chunk and compares blocks, heightmaps and post-processing marks (vanilla's result kept).
 */
public final class HorizonSurface {
    private HorizonSurface() {}

    private static final boolean ON = !"vanilla".equals(System.getenv("HORIZON_JAVA_SURFACE"));
    private static final boolean CHECK = "1".equals(System.getenv("HORIZON_JAVA_SURFACE_CHECK"));
    /** Set while vanilla's own buildSurface runs (the check's second run). */
    private static final ThreadLocal<Boolean> VANILLA = ThreadLocal.withInitial(() -> false);

    private static final MethodHandle CONTEXT;
    private static final MethodHandle UPDATE_XZ;
    private static final MethodHandle UPDATE_Y;
    private static final MethodHandle ERODED_BADLANDS;
    private static final MethodHandle FROZEN_OCEAN;
    private static final java.lang.invoke.VarHandle ZOOM_SOURCE;
    private static final java.lang.invoke.VarHandle ZOOM_SEED;
    private static final boolean READY;

    static {
        MethodHandle context = null, xz = null, y = null, badlands = null, ocean = null;
        java.lang.invoke.VarHandle source = null, seed = null;
        boolean ready = false;
        try {
            var contexts = MethodHandles.privateLookupIn(MaterialRuleContext.class, MethodHandles.lookup());
            context = contexts.findConstructor(MaterialRuleContext.class, MethodType.methodType(void.class,
                    MaterialSystem.class, RandomState.class, DensityVolume.class, DensitySamplerSet.class, Function.class,
                    WorldGenerationContext.class, Set.class));
            xz = contexts.findVirtual(MaterialRuleContext.class, "updateXZ",
                    MethodType.methodType(void.class, int.class, int.class, int.class, int.class));
            y = contexts.findVirtual(MaterialRuleContext.class, "updateY",
                    MethodType.methodType(void.class, int.class, int.class, int.class, int.class));
            var systems = MethodHandles.privateLookupIn(MaterialSystem.class, MethodHandles.lookup());
            badlands = systems.findVirtual(MaterialSystem.class, "erodedBadlandsExtension", MethodType.methodType(void.class,
                    BlockColumn.class, int.class, int.class, int.class, net.minecraft.world.level.LevelHeightAccessor.class));
            ocean = systems.findVirtual(MaterialSystem.class, "frozenOceanExtension", MethodType.methodType(void.class,
                    int.class, Biome.class, BlockColumn.class, BlockPos.MutableBlockPos.class, int.class, int.class, int.class));
            var managers = MethodHandles.privateLookupIn(BiomeManager.class, MethodHandles.lookup());
            source = managers.findVarHandle(BiomeManager.class, "noiseBiomeSource", BiomeResolver.class);
            seed = managers.findVarHandle(BiomeManager.class, "biomeZoomSeed", long.class);
            ready = true;
            // Another mod's injection into the walk would be skipped: vanilla's runs then.
            Set<String> others = new java.util.TreeSet<>();
            for (var method : MaterialSystem.class.getDeclaredMethods()) {
                var merged = method.getAnnotation(org.spongepowered.asm.mixin.transformer.meta.MixinMerged.class);
                if (merged != null && !merged.mixin().startsWith("dev.horizonlab.")) others.add(merged.mixin());
            }
            if (!others.isEmpty()) {
                Natives.log("Java mods' worldgen: " + String.join(", ", others) + " change MaterialSystem: vanilla's surface walk runs");
                ready = false;
            }
        } catch (ReflectiveOperationException | RuntimeException e) {
            Natives.log("Java mods' worldgen: the faster surface walk is off (" + e + "): vanilla's runs");
        }
        CONTEXT = context;
        UPDATE_XZ = xz;
        UPDATE_Y = y;
        ERODED_BADLANDS = badlands;
        FROZEN_OCEAN = ocean;
        ZOOM_SOURCE = source;
        ZOOM_SEED = seed;
        READY = ready;
    }

    private static final AtomicLong CHECKED = new AtomicLong();
    private static final AtomicLong DIFFERENT = new AtomicLong();
    private static final AtomicLong FAST_NANOS = new AtomicLong();
    private static final AtomicLong VANILLA_NANOS = new AtomicLong();

    /**
     * MaterialSystem.buildSurface's head (MaterialSystemMixin): true when the chunk's surface
     * was built here (vanilla's is skipped).
     */
    public static boolean build(MaterialSystem system, RandomState randomState, BiomeManager biomeManager,
                                WorldGenerationContext generation, ChunkAccess chunk, NoiseChunk noiseChunk,
                                MaterialRule rule, Set<Holder<Biome>> possibleBiomes) {
        if (!ON || !READY || VANILLA.get() || !(chunk instanceof ProtoChunk proto)) return false;
        // Light and every heightmap ProtoChunk.setBlockState keeps up: as vanilla's, or vanilla's.
        if (proto.getPersistedStatus().isOrAfter(ChunkStatus.INITIALIZE_LIGHT)) return false;
        Heightmap[] heightmaps = heightmaps(proto);
        if (heightmaps == null) return false;
        if (!CHECK) {
            fast(system, randomState, biomeManager, generation, proto, noiseChunk, rule, possibleBiomes, heightmaps);
            return true;
        }
        // Both from the same start; the first to run fills the noise chunk's caches the
        // second reuses, so the order alternates from chunk to chunk. Vanilla's result stays.
        Snapshot before = Snapshot.of(proto);
        boolean fastFirst = (CHECKED.get() & 1) == 0;
        long fastTook = 0, vanillaTook = 0;
        Snapshot mine = null, theirs = null;
        for (int run = 0; run < 2; run++) {
            if (run == 1) before.restore(proto);
            long started = System.nanoTime();
            if ((run == 0) == fastFirst) {
                fast(system, randomState, biomeManager, generation, proto, noiseChunk, rule, possibleBiomes, heightmaps);
                fastTook = System.nanoTime() - started;
                mine = Snapshot.of(proto);
            } else {
                VANILLA.set(true);
                try {
                    system.buildSurface(randomState, biomeManager, generation, chunk, noiseChunk, rule, possibleBiomes);
                } finally {
                    VANILLA.set(false);
                }
                vanillaTook = System.nanoTime() - started;
                theirs = Snapshot.of(proto);
            }
        }
        if (fastFirst == false) theirs.restore(proto);
        String difference = mine.differs(theirs, proto);
        long n = CHECKED.incrementAndGet();
        FAST_NANOS.addAndGet(fastTook);
        VANILLA_NANOS.addAndGet(vanillaTook);
        if (difference != null) DIFFERENT.incrementAndGet();
        if (difference != null || n % 50 == 0) {
            Natives.log("Java mods' worldgen: surface check: " + n + " chunks, " + DIFFERENT.get() + " differ; "
                    + String.format("%.2f", FAST_NANOS.get() / 1e6 / n) + " ms a chunk against vanilla's "
                    + String.format("%.2f", VANILLA_NANOS.get() / 1e6 / n)
                    + (difference != null ? "; " + proto.getPos() + ": " + difference : ""));
        }
        return true;
    }

    /**
     * Far terrain's top blocks (HorizonWorldgen.lodTops): the rule's block at each x y z
     * (the land's top block, solid below it, water up to {@code waterHeights[i]} above it or
     * Integer.MIN_VALUE for none), as buildSurface would give it with the column's surface
     * flat around. One context for all (the rule compiled once): positions outside its 1x1x1
     * volume sample their densities one by one, as vanilla's own do. Null entries where the
     * rule leaves the block; null if the walk isn't available.
     */
    public static BlockState[] tops(MaterialSystem system, MaterialRule ruleSource, RandomState randomState,
                                    WorldGenerationContext generation, Function<BlockPos, Holder<Biome>> biomes,
                                    DensitySamplerSet samplers, int[] xyz, int[] waterHeights) {
        if (!READY || xyz.length < 3) return null;
        try {
            DensityVolume volume = new DensityVolume(1, 1, 1, xyz[0], xyz[1], xyz[2]);
            MaterialRuleContext context = (MaterialRuleContext) CONTEXT.invoke(system, randomState, volume, samplers,
                    biomes, generation, (Set<Holder<Biome>>) null);
            RuleEvaluator rule = ruleSource.compile(context);
            BlockState[] out = new BlockState[xyz.length / 3];
            for (int i = 0; i < out.length; i++) {
                int x = xyz[3 * i], y = xyz[3 * i + 1], z = xyz[3 * i + 2];
                UPDATE_XZ.invoke(context, x, z, 0, 0);
                // The top of the land: one block of stone above it, the land going on below.
                UPDATE_Y.invoke(context, 1, 1000, waterHeights[i], y);
                out[i] = rule.tryApply(x, y, z);
            }
            return out;
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new IllegalStateException(t);
        }
    }

    /**
     * Every block a material rule can give (its block rules, ore veins, the badlands' bands),
     * through sequences, conditions and references, and the rules a mod's own kinds of rule hold.
     */
    public static Set<BlockState> materials(MaterialRule rule) {
        Set<BlockState> out = new java.util.HashSet<>();
        collect(rule, out, java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>()));
        return out;
    }

    private static void collect(MaterialRule rule, Set<BlockState> out, Set<Object> seen) {
        if (rule == null || !seen.add(rule)) return;
        switch (rule) {
            case net.minecraft.world.level.levelgen.material.rule.SequenceRule sequence ->
                    sequence.sequence().forEach(r -> collect(r, out, seen));
            case net.minecraft.world.level.levelgen.material.rule.ConditionRule condition -> collect(condition.thenRun(), out, seen);
            case net.minecraft.world.level.levelgen.material.rule.BlockRule block -> out.add(block.resultState());
            case net.minecraft.world.level.levelgen.material.rule.OreVeinRule vein -> {
                out.add(vein.oreBlock());
                out.add(vein.rawOreBlock());
                out.add(vein.fillerBlock());
            }
            case MaterialRule.HolderHolder reference -> collect(reference.holder().value(), out, seen);
            default -> {
                if (rule instanceof net.minecraft.world.level.levelgen.material.rule.BandlandsRule) {
                    for (var block : new net.minecraft.world.level.block.Block[] {Blocks.TERRACOTTA,
                            Blocks.DYED_TERRACOTTA.white(), Blocks.DYED_TERRACOTTA.orange(), Blocks.DYED_TERRACOTTA.yellow(),
                            Blocks.DYED_TERRACOTTA.brown(), Blocks.DYED_TERRACOTTA.red(), Blocks.DYED_TERRACOTTA.lightGray()}) {
                        out.add(block.defaultBlockState());
                    }
                    return;
                }
                // A mod's own kind of rule (one combining others: a base and each mod's
                // rules): the rules it holds, in its fields, lists and maps.
                for (Class<?> c = rule.getClass(); c != null && c != Object.class; c = c.getSuperclass()) {
                    for (java.lang.reflect.Field f : c.getDeclaredFields()) {
                        if (java.lang.reflect.Modifier.isStatic(f.getModifiers()) || f.getType().isPrimitive()) continue;
                        try {
                            f.setAccessible(true);
                            held(f.get(rule), out, seen);
                        } catch (RuntimeException | ReflectiveOperationException e) {
                            // Not readable: its blocks aren't known.
                        }
                    }
                }
            }
        }
    }

    /** The material rules a value holds: itself, a holder's, a list's or a map's. */
    private static void held(Object value, Set<BlockState> out, Set<Object> seen) {
        switch (value) {
            case MaterialRule m -> collect(m, out, seen);
            case Holder<?> h when h.isBound() -> held(h.value(), out, seen);
            case Iterable<?> list -> {
                for (Object o : list) if (o instanceof MaterialRule || o instanceof Holder<?>) held(o, out, seen);
            }
            case Map<?, ?> map -> {
                for (Object o : map.values()) if (o instanceof MaterialRule || o instanceof Holder<?>) held(o, out, seen);
            }
            case null, default -> {}
        }
    }

    /** The heightmaps ProtoChunk.setBlockState updates at this status, all there; null if one isn't. */
    private static Heightmap[] heightmaps(ProtoChunk chunk) {
        EnumSet<Heightmap.Types> kinds = chunk.getPersistedStatus().heightmapsAfter();
        Heightmap[] out = new Heightmap[kinds.size()];
        int i = 0;
        for (Heightmap.Types kind : kinds) {
            Heightmap found = null;
            for (Map.Entry<Heightmap.Types, Heightmap> entry : chunk.getHeightmaps()) {
                if (entry.getKey() == kind) found = entry.getValue();
            }
            if (found == null) return null;
            out[i++] = found;
        }
        return out;
    }

    private static void fast(MaterialSystem system, RandomState randomState, BiomeManager biomeManager,
                             WorldGenerationContext generation, ProtoChunk chunk, NoiseChunk noiseChunk,
                             MaterialRule ruleSource, Set<Holder<Biome>> possibleBiomes, Heightmap[] heightmaps) {
        try {
            walk(system, randomState, biomeManager, generation, chunk, noiseChunk, ruleSource, possibleBiomes, heightmaps);
        } catch (RuntimeException | Error e) {
            throw e;
        } catch (Throwable t) {
            throw new IllegalStateException(t);
        }
    }

    /** MaterialSystem.buildSurface, step for step. */
    private static void walk(MaterialSystem system, RandomState randomState, BiomeManager biomeManager,
                             WorldGenerationContext generation, ProtoChunk chunk, NoiseChunk noiseChunk,
                             MaterialRule ruleSource, Set<Holder<Biome>> possibleBiomes, Heightmap[] heightmaps) throws Throwable {
        int minBlockX = chunk.getPos().getMinBlockX();
        int minBlockZ = chunk.getPos().getMinBlockZ();
        var heightAccessor = chunk.getHeightAccessorForGeneration();
        int minY = heightAccessor.getMinY();
        int maxY = heightAccessor.getMaxY();
        Column column = new Column(chunk, minY, maxY, heightmaps);
        int highestFilledSectionIndex = chunk.getHighestFilledSectionIndex();
        int maxBlockY = chunk.getSectionYFromSectionIndex(highestFilledSectionIndex) * 16 + 15;
        DensityVolume fullVolume = noiseChunk.volume();
        DensityVolume narrowedVolume = new DensityVolume(fullVolume.sizeX(), Math.max(maxBlockY - fullVolume.minBlockY() + 1, 1),
                fullVolume.sizeZ(), fullVolume.minBlockX(), fullVolume.minBlockY(), fullVolume.minBlockZ());
        Biomes2 biomes = new Biomes2(biomeManager);
        MaterialRuleContext context = (MaterialRuleContext) CONTEXT.invoke(system, randomState, narrowedVolume,
                noiseChunk.cachingSamplers(), (Function<BlockPos, Holder<Biome>>) biomes, generation, possibleBiomes);
        RuleEvaluator rule = ruleSource.compile(context);
        BlockPos.MutableBlockPos blockPos = new BlockPos.MutableBlockPos();
        Heightmap.Types surface = Heightmap.Types.WORLD_SURFACE_WG;
        BlockState lastState = null;
        boolean lastFluid = false;
        for (int x = 0; x < 16; x++) {
            for (int z = 0; z < 16; z++) {
                int blockX = minBlockX + x;
                int blockZ = minBlockZ + z;
                int startingHeight = chunk.getHeight(surface, x, z) + 1;
                column.at(blockX, blockZ);
                Holder<Biome> surfaceBiome = biomes.at(blockX, startingHeight, blockZ);
                if (surfaceBiome.is(Biomes.ERODED_BADLANDS)) {
                    ERODED_BADLANDS.invoke(system, (BlockColumn) column, blockX, blockZ, startingHeight,
                            (net.minecraft.world.level.LevelHeightAccessor) chunk);
                }
                int height = chunk.getHeight(surface, x, z) + 1;
                int gradientX = chunk.getHeight(surface, Math.min(x + 1, 15), z) - chunk.getHeight(surface, Math.max(x - 1, 0), z);
                int gradientZ = chunk.getHeight(surface, x, Math.min(z + 1, 15)) - chunk.getHeight(surface, x, Math.max(z - 1, 0));
                UPDATE_XZ.invoke(context, blockX, blockZ, gradientX, gradientZ);
                int stoneAboveDepth = 0;
                int waterHeight = Integer.MIN_VALUE;
                int nextCeilingStoneY = Integer.MAX_VALUE;
                int endY = chunk.getMinY();
                for (int y = height; y >= endY; y--) {
                    BlockState old = column.getBlock(y);
                    // The column is mostly one state (stone): its fluid asked once.
                    if (old != lastState) {
                        lastState = old;
                        lastFluid = !old.getFluidState().isEmpty();
                    }
                    if (old.isAir()) {
                        stoneAboveDepth = 0;
                        waterHeight = Integer.MIN_VALUE;
                    } else if (lastFluid) {
                        if (waterHeight == Integer.MIN_VALUE) waterHeight = y + 1;
                    } else {
                        if (nextCeilingStoneY >= y) {
                            nextCeilingStoneY = DimensionType.WAY_BELOW_MIN_Y;
                            for (int lookaheadY = y - 1; lookaheadY >= endY - 1; lookaheadY--) {
                                BlockState next = column.getBlock(lookaheadY);
                                if (next.isAir() || !next.getFluidState().isEmpty()) {
                                    nextCeilingStoneY = lookaheadY + 1;
                                    break;
                                }
                            }
                        }
                        stoneAboveDepth++;
                        int stoneBelowDepth = y - nextCeilingStoneY + 1;
                        UPDATE_Y.invoke(context, stoneAboveDepth, stoneBelowDepth, waterHeight, y);
                        if (y >= minY && y <= maxY) {
                            BlockState state = rule.tryApply(blockX, y, blockZ);
                            if (state != null) column.setBlock(y, old, state);
                        }
                    }
                }
                if (surfaceBiome.is(Biomes.FROZEN_OCEAN) || surfaceBiome.is(Biomes.DEEP_FROZEN_OCEAN)) {
                    FROZEN_OCEAN.invoke(system, context.getMinSurfaceLevel(), surfaceBiome.value(), (BlockColumn) column,
                            blockPos, blockX, blockZ, startingHeight);
                }
            }
        }
    }

    /**
     * MaterialSystem's chunk column (its anonymous BlockColumn) reading and writing the
     * sections directly, as ProtoChunk.getBlockState and setBlockState would at this status.
     */
    private static final class Column implements BlockColumn {
        private final ProtoChunk chunk;
        private final LevelChunkSection[] sections;
        private final int minY;
        private final int maxY;
        private final int chunkMinY;
        private final int chunkMaxY;
        private final int minSection;
        private final Heightmap[] heightmaps;
        private final BlockPos.MutableBlockPos pos = new BlockPos.MutableBlockPos();
        private int localX;
        private int localZ;

        Column(ProtoChunk chunk, int minY, int maxY, Heightmap[] heightmaps) {
            this.chunk = chunk;
            this.sections = chunk.getSections();
            this.minY = minY;
            this.maxY = maxY;
            this.chunkMinY = chunk.getMinY();
            this.chunkMaxY = chunk.getMaxY();
            this.minSection = chunk.getMinSectionY();
            this.heightmaps = heightmaps;
        }

        void at(int blockX, int blockZ) {
            pos.setX(blockX).setZ(blockZ);
            localX = blockX & 15;
            localZ = blockZ & 15;
        }

        @Override
        public BlockState getBlock(int y) {
            // ProtoChunk.getBlockState: void air outside the chunk, air in an empty section.
            if (y < chunkMinY || y > chunkMaxY) return Blocks.VOID_AIR.defaultBlockState();
            LevelChunkSection section = sections[(y >> 4) - minSection];
            return section.hasOnlyAir() ? Blocks.AIR.defaultBlockState() : section.getBlockState(localX, y & 15, localZ);
        }

        @Override
        public void setBlock(int y, BlockState state) {
            setBlock(y, getBlock(y), state);
        }

        /** The anonymous column's setBlock, knowing the state there ({@code old}). */
        void setBlock(int y, BlockState old, BlockState state) {
            if (y < minY || y > maxY) return;
            if (state != old) set(y, state);
            if (state != lastSet) {
                lastSet = state;
                lastSetFluid = !state.getFluidState().isEmpty();
            }
            if (lastSetFluid) chunk.markPosForPostProcessing(pos.setY(y));
        }

        private BlockState lastSet;
        private boolean lastSetFluid;

        /** ProtoChunk.setBlockState before INITIALIZE_LIGHT: the section, then each heightmap. */
        private void set(int y, BlockState state) {
            if (y < chunkMinY || y > chunkMaxY) return;
            LevelChunkSection section = sections[(y >> 4) - minSection];
            if (section.hasOnlyAir() && state.is(Blocks.AIR)) return;
            section.setBlockState(localX, y & 15, localZ, state, false);
            for (Heightmap heightmap : heightmaps) heightmap.update(localX, y, localZ, state);
        }
    }

    /**
     * BiomeManager.getBiome, with its answer taken straight from the noise biomes when the
     * eight corners it picks among have the same one (then any pick gives it), kept while
     * the corners stay.
     */
    private static final class Biomes2 implements Function<BlockPos, Holder<Biome>> {
        private final BiomeManager manager;
        private final BiomeResolver source;
        private int cornerX = Integer.MIN_VALUE, cornerY, cornerZ;
        private Holder<Biome> uniform;

        Biomes2(BiomeManager manager) {
            this.manager = manager;
            this.source = (BiomeResolver) ZOOM_SOURCE.get(manager);
        }

        @Override
        public Holder<Biome> apply(BlockPos pos) {
            return at(pos.getX(), pos.getY(), pos.getZ());
        }

        Holder<Biome> at(int x, int y, int z) {
            int parentX = (x - 2) >> 2, parentY = (y - 2) >> 2, parentZ = (z - 2) >> 2;
            if (parentX != cornerX || parentY != cornerY || parentZ != cornerZ) {
                cornerX = parentX;
                cornerY = parentY;
                cornerZ = parentZ;
                uniform = uniform(parentX, parentY, parentZ);
            }
            return uniform != null ? uniform : manager.getBiome(x, y, z);
        }

        /** The corners' biome if all eight have it, else null. */
        private Holder<Biome> uniform(int x, int y, int z) {
            Holder<Biome> first = source.getNoiseBiome(x, y, z);
            for (int i = 1; i < 8; i++) {
                if (source.getNoiseBiome(x + ((i >> 2) & 1), y + ((i >> 1) & 1), z + (i & 1)) != first) return null;
            }
            return first;
        }
    }

    /** A chunk's blocks, world-generation heightmaps and post-processing marks, copied. */
    private record Snapshot(LevelChunkSection[] sections, Map<Heightmap.Types, long[]> heightmaps, short[][] marks) {
        static Snapshot of(ProtoChunk chunk) {
            LevelChunkSection[] live = chunk.getSections();
            LevelChunkSection[] sections = new LevelChunkSection[live.length];
            for (int i = 0; i < live.length; i++) sections[i] = live[i].copy();
            Map<Heightmap.Types, long[]> heightmaps = new java.util.EnumMap<>(Heightmap.Types.class);
            for (Map.Entry<Heightmap.Types, Heightmap> entry : chunk.getHeightmaps()) {
                heightmaps.put(entry.getKey(), entry.getValue().getRawData().clone());
            }
            var post = chunk.getPostProcessing();
            short[][] marks = new short[post.length][];
            for (int i = 0; i < post.length; i++) marks[i] = post[i] == null ? null : post[i].toShortArray();
            return new Snapshot(sections, heightmaps, marks);
        }

        void restore(ProtoChunk chunk) {
            LevelChunkSection[] live = chunk.getSections();
            for (int i = 0; i < live.length; i++) live[i] = sections[i].copy();
            for (Map.Entry<Heightmap.Types, long[]> entry : heightmaps.entrySet()) {
                chunk.setHeightmap(entry.getKey(), entry.getValue().clone());
            }
            var post = chunk.getPostProcessing();
            for (int i = 0; i < post.length; i++) {
                if (post[i] == null) continue;
                post[i].clear();
                if (marks[i] != null) post[i].addAll(it.unimi.dsi.fastutil.shorts.ShortArrayList.wrap(marks[i]));
            }
        }

        /** Where {@code other} differs from this, or null. */
        String differs(Snapshot other, ProtoChunk chunk) {
            long blocks = 0;
            String first = null;
            for (int i = 0; i < sections.length; i++) {
                for (int y = 0; y < 16; y++) {
                    for (int z = 0; z < 16; z++) {
                        for (int x = 0; x < 16; x++) {
                            BlockState mine = sections[i].getBlockState(x, y, z);
                            BlockState theirs = other.sections[i].getBlockState(x, y, z);
                            if (mine != theirs) {
                                if (first == null) {
                                    first = (chunk.getPos().getMinBlockX() + x) + " " + ((chunk.getMinSectionY() + i) * 16 + y) + " "
                                            + (chunk.getPos().getMinBlockZ() + z) + ": vanilla " + theirs + ", Horizon's " + mine;
                                }
                                blocks++;
                            }
                        }
                    }
                }
            }
            if (blocks > 0) return blocks + " blocks (first at " + first + ")";
            for (var entry : heightmaps.entrySet()) {
                long[] theirs = other.heightmaps.get(entry.getKey());
                if (theirs == null || !java.util.Arrays.equals(entry.getValue(), theirs)) return "heightmap " + entry.getKey();
            }
            for (int i = 0; i < marks.length; i++) {
                short[] a = marks[i], b = other.marks[i];
                if (!java.util.Arrays.equals(a == null ? new short[0] : a, b == null ? new short[0] : b)) {
                    return "post-processing marks of section " + i;
                }
            }
            return null;
        }
    }
}
