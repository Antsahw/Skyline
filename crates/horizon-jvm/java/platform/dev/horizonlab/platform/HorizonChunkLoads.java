package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CompletableFuture;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.StreamTagVisitor;
import net.minecraft.server.level.ChunkResult;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.ChunkAccess;
import net.minecraft.world.level.chunk.status.ChunkStatus;

/**
 * Loading chunks through vanilla's chunk system on a mirrored level, as a pregenerator does
 * (Chunky: a ticket, then ServerChunkCache.getChunkFutureMainThread, and ChunkMap's readChunk
 * or chunkScanner to see what is generated already). Horizon holds the chunks: a chunk asked
 * for is kept as a ticket keeps it (HorizonTickets sends it with the tickets' chunks) until
 * Horizon has it loaded, then the future completes with a copy of it, on the server thread, as
 * vanilla's completes there. What is saved is read from Horizon's own region files, which are
 * vanilla's format, read-only (Horizon writes them).
 */
public final class HorizonChunkLoads {
    private HorizonChunkLoads() {}

    /** A chunk waited for: its futures and the ticks waited. */
    private static final class Wanted {
        final List<CompletableFuture<ChunkResult<ChunkAccess>>> futures = new ArrayList<>();
        int ticks;
    }

    /** HORIZON_JAVA_DEBUG=chunks: each load asked for and each one done. */
    static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("chunks");

    /** How long a chunk is waited for (Horizon may never load one: past the world's border). */
    private static final int GIVE_UP_TICKS = 20 * 120;

    private static final Map<ServerLevel, Map<Long, Wanted>> WANTED = new IdentityHashMap<>();

    /**
     * ServerChunkCache.getChunkFuture(MainThread) on a mirrored level: Horizon's chunk when it
     * has it loaded (any status asked: Horizon's loaded chunks are full ones); else, when
     * {@code load} (vanilla adds a short ticket for it: so does this) or a ticket keeps it, a
     * future completed once Horizon loaded it; else the unloaded result.
     */
    public static CompletableFuture<ChunkResult<ChunkAccess>> future(ServerLevel level, int x, int z,
                                                                      ChunkStatus status, boolean load) {
        ChunkAccess now = loaded(level, x, z);
        if (DEBUG) Natives.log("chunks: " + x + "," + z + " of " + HorizonServer.dimensionOf(level) + " asked for ("
                + status + (load ? ", load" : "") + "): " + (now != null ? "loaded" : "waits") + " on " + Thread.currentThread().getName());
        if (now != null) return CompletableFuture.completedFuture(ChunkResult.of(now));
        // Not to be loaded: only where a ticket keeps it (vanilla's holder exists then).
        if (!load && !HorizonTickets.keeps(level, x, z)) return net.minecraft.server.level.GenerationChunkHolder.UNLOADED_CHUNK_FUTURE;
        CompletableFuture<ChunkResult<ChunkAccess>> future = new CompletableFuture<>();
        synchronized (WANTED) {
            WANTED.computeIfAbsent(level, l -> new LinkedHashMap<>())
                    .computeIfAbsent(ChunkPos.pack(x, z), k -> new Wanted()).futures.add(future);
        }
        return future;
    }

    /** Horizon's chunk x z as a chunk of the level, when this thread can read it and it is loaded. */
    private static ChunkAccess loaded(ServerLevel level, int x, int z) {
        long revision = Natives.chunkRevision(HorizonServer.dimensionOf(level), x, z);
        if (revision < 0) return null;
        return HorizonChunks.chunk(level, x, z);
    }

    /**
     * Each server tick (before the tickets go to Horizon): the futures of chunks Horizon loaded
     * complete; those waited for too long fail, as vanilla's would at a chunk it can't make.
     */
    static void tick() {
        List<Runnable> done = new ArrayList<>();
        synchronized (WANTED) {
            for (var byLevel : WANTED.entrySet()) {
                ServerLevel level = byLevel.getKey();
                for (Iterator<Map.Entry<Long, Wanted>> it = byLevel.getValue().entrySet().iterator(); it.hasNext(); ) {
                    var entry = it.next();
                    ChunkPos pos = ChunkPos.unpack(entry.getKey());
                    Wanted wanted = entry.getValue();
                    ChunkAccess chunk = loaded(level, pos.x(), pos.z());
                    ChunkResult<ChunkAccess> result = chunk != null ? ChunkResult.of(chunk)
                            : ++wanted.ticks > GIVE_UP_TICKS ? ChunkResult.error("Horizon didn't load chunk " + pos) : null;
                    if (result == null) continue;
                    if (DEBUG) Natives.log("chunks: " + pos + " " + (chunk != null ? "loaded" : "given up") + " after " + wanted.ticks + " ticks");
                    it.remove();
                    for (var future : wanted.futures) done.add(() -> future.complete(result));
                }
            }
        }
        // Outside the lock: what the futures run next may ask for more chunks.
        for (Runnable r : done) {
            try {
                r.run();
            } catch (Throwable t) {
                HorizonFailures.report("a chunk's load", t);
            }
        }
    }

    /** The chunks waited for in a level (kept loaded, as tickets keep theirs). */
    static List<Long> wanted(ServerLevel level) {
        synchronized (WANTED) {
            Map<Long, Wanted> chunks = WANTED.get(level);
            return chunks == null ? List.of() : new ArrayList<>(chunks.keySet());
        }
    }

    /** A new world: what was waited for belongs to the last one. */
    static void reset() {
        synchronized (WANTED) {
            for (Map<Long, Wanted> chunks : WANTED.values()) {
                for (Wanted w : chunks.values()) w.futures.forEach(f -> f.complete(ChunkResult.error("world closed")));
            }
            WANTED.clear();
        }
    }

    // ---- What is saved: Horizon's region files ------------------------------------------

    /** ChunkMap.readChunk on a mirrored level: the chunk's save (loaded: its status alone). */
    public static CompletableFuture<Optional<CompoundTag>> read(ServerLevel level, ChunkPos pos) {
        if (loaded(level, pos.x(), pos.z()) != null) return CompletableFuture.completedFuture(Optional.of(full()));
        try {
            CompoundTag[] tag = {null};
            region(level, pos, in -> tag[0] = NbtIo.read(in));
            return CompletableFuture.completedFuture(Optional.ofNullable(tag[0]));
        } catch (IOException | RuntimeException e) {
            return CompletableFuture.completedFuture(Optional.empty());
        }
    }

    /**
     * Horizon's save of chunk x z (its region file, vanilla's format) as a chunk of the level,
     * for a stand-in (HorizonWorldgen.standIn): its blocks, biomes and heights; null when it
     * isn't saved complete. Its block entities stay Horizon's.
     */
    public static net.minecraft.world.level.chunk.LevelChunk saved(ServerLevel level, int x, int z) {
        CompoundTag[] tag = {null};
        ChunkPos pos = new ChunkPos(x, z);
        try {
            region(level, pos, in -> tag[0] = NbtIo.read(in));
        } catch (IOException | RuntimeException e) {
            return null;
        }
        if (tag[0] == null) return null;
        var data = net.minecraft.world.level.chunk.storage.SerializableChunkData.parse(level, level.palettedContainerFactory(), tag[0]);
        if (data == null || !data.chunkStatus().isOrAfter(ChunkStatus.FULL)) return null;
        var sections = new net.minecraft.world.level.chunk.LevelChunkSection[level.getSectionsCount()];
        for (var section : data.sectionData()) {
            int i = level.getSectionIndexFromSectionY(section.y());
            if (i >= 0 && i < sections.length && section.chunkSection() != null) sections[i] = section.chunkSection();
        }
        for (int i = 0; i < sections.length; i++) {
            if (sections[i] == null) sections[i] = new net.minecraft.world.level.chunk.LevelChunkSection(level.palettedContainerFactory());
        }
        var chunk = new net.minecraft.world.level.chunk.LevelChunk(level, pos, net.minecraft.world.level.chunk.UpgradeData.EMPTY,
                new net.minecraft.world.ticks.LevelChunkTicks<>(), new net.minecraft.world.ticks.LevelChunkTicks<>(), 0L,
                sections, null, null);
        net.minecraft.world.level.levelgen.Heightmap.primeHeightmaps(chunk,
                java.util.EnumSet.allOf(net.minecraft.world.level.levelgen.Heightmap.Types.class));
        return chunk;
    }

    /**
     * ServerChunkCache.chunkScanner().scanChunk on a mirrored level: the visitor reads the
     * chunk's save (a pregenerator asks for its Status), or a loaded chunk's status.
     */
    public static CompletableFuture<Void> scan(ServerLevel level, ChunkPos pos, StreamTagVisitor visitor) {
        try {
            if (loaded(level, pos.x(), pos.z()) != null) {
                full().acceptAsRoot(visitor);
            } else {
                region(level, pos, in -> NbtIo.parse(in, visitor, NbtAccounter.unlimitedHeap()));
            }
        } catch (IOException | RuntimeException e) {
            // Unreadable (being written): as if it weren't saved.
        }
        return CompletableFuture.completedFuture(null);
    }

    /** What a loaded chunk is said to be: full. */
    private static CompoundTag full() {
        CompoundTag tag = new CompoundTag();
        tag.putString("Status", net.minecraft.core.registries.BuiltInRegistries.CHUNK_STATUS
                .getKey(ChunkStatus.FULL).toString());
        return tag;
    }

    private interface Reader {
        void read(DataInputStream in) throws IOException;
    }

    /**
     * Reads chunk {@code pos}'s record from its region file, as RegionFile does (the header's
     * offset and sector count, then the length, the compression and the data), read-only: the
     * file is Horizon's. Nothing is read where none is saved.
     */
    private static void region(ServerLevel level, ChunkPos pos, Reader reader) throws IOException {
        Path java = HorizonSavedData.javaDir();
        if (java == null || java.getParent() == null) return;
        // The world's folder: the Java side's is the Overworld's (dimensions/minecraft/overworld).
        Path root = java.getParent();
        if (root.getParent() != null && root.getParent().getParent() != null
                && root.getParent().getParent().getFileName() != null
                && root.getParent().getParent().getFileName().toString().equals("dimensions")) {
            root = root.getParent().getParent().getParent();
        }
        Path file = net.minecraft.world.level.dimension.DimensionType.getStorageFolder(level.dimension(), root)
                .resolve("region").resolve("r." + pos.getRegionX() + "." + pos.getRegionZ() + ".mca");
        if (!Files.isRegularFile(file)) return;
        byte[] data;
        int compression;
        try (RandomAccessFile raf = new RandomAccessFile(file.toFile(), "r")) {
            if (raf.length() < 8192) return;
            raf.seek(4L * (pos.getRegionLocalX() + pos.getRegionLocalZ() * 32));
            int location = raf.readInt();
            long offset = (long) (location >>> 8) * 4096;
            if (location == 0 || offset < 8192 || offset + 5 > raf.length()) return;
            raf.seek(offset);
            int length = raf.readInt();
            compression = raf.readByte();
            if (length <= 1 || offset + 4 + length > raf.length() || (compression & 128) != 0) return;
            data = new byte[length - 1];
            raf.readFully(data);
        }
        if (!net.minecraft.world.level.chunk.storage.RegionFileVersion.isValidVersion(compression)) return;
        var version = net.minecraft.world.level.chunk.storage.RegionFileVersion.fromId(compression);
        try (DataInputStream in = new DataInputStream(version.wrap(new java.io.ByteArrayInputStream(data)))) {
            reader.read(in);
        }
    }
}
