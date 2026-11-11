package dev.horizonlab.platform120;

import com.mojang.authlib.GameProfile;
import dev.horizonlab.platform120.mixin.LevelAccess;
import dev.horizonlab.platform120.mixin.ServerLevelAccess;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.dedicated.DedicatedPlayerList;
import net.minecraft.server.dedicated.DedicatedServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.players.PlayerList;
import net.minecraft.util.RandomSource;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.item.Item;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.dimension.BuiltinDimensionTypes;
import net.minecraft.world.level.dimension.DimensionType;
import net.minecraft.world.phys.BlockHitResult;
import net.minecraft.world.phys.Vec3;

/**
 * Horizon's world as 1.20.1 mods see it (docs/java-bridge-120.md, phase 3): a server, its levels
 * and its players made without their running parts. The mixins in this package send what mods'
 * code reads of a level to Horizon (Host "blockState") and what it sets back to Horizon
 * ("setBlock" events, applied by Horizon after the call), and quiet what a world with no server
 * behind it can't do (sounds, particles and neighbour updates for now). In the 1.20.1 runtime
 * every level and player is one of these: there is no other server.
 *
 * <p>Within one call from Horizon a mod reads its own writes: they are kept until the call ends
 * ({@link #begin}).
 */
public final class Mirror120 {
    private Mirror120() {}

    /**
     * Horizon's side, set by the boot (Boot120.mirror): calls answered by Horizon, and events
     * it applies after the call. Hooks rather than the bridge's Host, which the game's class
     * loader would load a second copy of, with no connection.
     */
    public static volatile BiFunction<String, Object[], Object[]> call;

    public static volatile BiConsumer<String, Object[]> event;

    static DedicatedServer server;
    static PlayerList playerList;
    static RegistryAccess.Frozen registries;
    /** The server's data packs as loaded (recipes, loot, tags, advancements, functions). */
    private static net.minecraft.server.ReloadableServerResources data;
    private static Holder<DimensionType> overworldType;
    /** The world's data a server has from level.dat: a new world's, shared by every level. */
    private static net.minecraft.world.level.storage.PrimaryLevelData worldData;
    private static final Map<String, ServerLevel> LEVELS = new HashMap<>();
    private static final Map<UUID, ServerPlayer> PLAYERS = new HashMap<>();
    /** Horizon's state names (26.3's) as 1.20.1 states, as read so far. */
    private static final Map<String, BlockState> STATES = new HashMap<>();
    /** This call's writes, per level, by packed position. */
    private static final Map<Level, Map<Long, BlockState>> WRITES = new HashMap<>();
    /**
     * What this call has read of Horizon's blocks, per level, by packed position.
     *
     * <p>A read costs a round trip (tens of microseconds) whatever it carries, and the code on
     * this side reads the same position again and again -- a block's own `use` asks for its
     * state, then `setBlock` asks again to see whether it changed. Kept for the length of a
     * call only, which is as long as the answer is good for: within one call Horizon's world
     * changes only where this side writes, and a write is read from WRITES before this.
     */
    private static final Map<Level, Map<Long, BlockState>> READS = new HashMap<>();
    /**
     * This call's block entities, per level, by packed position. One position gives one object
     * for the whole call: mods keep what {@code getBlockEntity} gave them and change it in
     * place, and some compare it by identity.
     */
    private static final Map<Level, Map<Long, BlockEntity>> ENTITIES = new HashMap<>();
    /** Positions this call asked Horizon about and found nothing at (asked once). */
    private static final Map<Level, java.util.Set<Long>> NO_ENTITY = new HashMap<>();
    /**
     * What each of this call's block entities held when it was made, as this side writes it:
     * only one a mod changed goes back to Horizon. A new one gets "", so it always does.
     */
    private static final Map<BlockEntity, String> ENTITY_NBT = new java.util.IdentityHashMap<>();

    static synchronized void setUp() {
        if (server != null) return;
        server = Allocate.make(DedicatedServer.class);
        playerList = Allocate.make(DedicatedPlayerList.class);
        ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setPlayerList(playerList);
        ((dev.horizonlab.platform120.mixin.PlayerListAccess) playerList).horizon$setServer(server);
        // The registries as vanilla's WorldLoader makes them: the built-in ones, then the data
        // packs' worldgen and dimensions (vanilla's and the mods'), then the reloadable data:
        // recipes, loot, tags (bound into the registries: mods ask `state.is(BlockTags.CAMPFIRES)`),
        // advancements and functions.
        var packs = Tags120.dataPacks();
        var layers = net.minecraft.server.RegistryLayer.createRegistryAccess();
        var worldgen = net.minecraft.resources.RegistryDataLoader.load(packs,
                layers.getAccessForLoading(net.minecraft.server.RegistryLayer.WORLDGEN), worldgenRegistries());
        layers = layers.replaceFrom(net.minecraft.server.RegistryLayer.WORLDGEN, worldgen);
        var dimensions = net.minecraft.resources.RegistryDataLoader.load(packs,
                layers.getAccessForLoading(net.minecraft.server.RegistryLayer.DIMENSIONS), net.minecraft.resources.RegistryDataLoader.DIMENSION_REGISTRIES);
        layers = layers.replaceFrom(net.minecraft.server.RegistryLayer.DIMENSIONS, dimensions);
        registries = layers.compositeAccess();
        try {
            data = net.minecraft.server.ReloadableServerResources.loadResources(packs,
                    layers.getAccessForLoading(net.minecraft.server.RegistryLayer.RELOADABLE),
                    net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS, net.minecraft.commands.Commands.CommandSelection.DEDICATED, 2,
                    net.minecraft.Util.backgroundExecutor(), Runnable::run).get();
        } catch (InterruptedException | java.util.concurrent.ExecutionException e) {
            throw new IllegalStateException("the data packs didn't load", e);
        }
        data.updateRegistryTags(registries);
        worldData = new net.minecraft.world.level.storage.PrimaryLevelData(
                new net.minecraft.world.level.LevelSettings("Horizon", GameType.SURVIVAL, false, net.minecraft.world.Difficulty.NORMAL,
                        true, new net.minecraft.world.level.GameRules(), net.minecraft.world.level.WorldDataConfiguration.DEFAULT),
                new net.minecraft.world.level.levelgen.WorldOptions(0L, true, false),
                net.minecraft.world.level.storage.PrimaryLevelData.SpecialWorldProperty.NONE,
                com.mojang.serialization.Lifecycle.stable());
        ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setWorldData(worldData);
        // Statistics award score to objectives: an empty board (Horizon keeps the real one).
        ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setScoreboard(new net.minecraft.server.ServerScoreboard(server));
        overworldType = registries.registryOrThrow(Registries.DIMENSION_TYPE).getHolderOrThrow(BuiltinDimensionTypes.OVERWORLD);
    }

    public static MinecraftServer server() {
        return server;
    }

    /**
     * The data packs' registries a world loads: vanilla's, and those mods add through Fabric
     * API's DynamicRegistries (which keeps its own list; its mixins hand it to a real server).
     */
    @SuppressWarnings("unchecked")
    private static java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>> worldgenRegistries() {
        try {
            Class<?> fabric = Class.forName("net.fabricmc.fabric.impl.registry.sync.DynamicRegistriesImpl");
            return (java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>>) fabric.getMethod("getDynamicRegistries").invoke(null);
        } catch (ReflectiveOperationException e) {
            return net.minecraft.resources.RegistryDataLoader.WORLDGEN_REGISTRIES;
        }
    }

    public static RegistryAccess.Frozen registries() {
        setUp();
        return registries;
    }

    public static net.minecraft.server.ReloadableServerResources data() {
        setUp();
        return data;
    }

    /** A level of Horizon's (`minecraft:overworld`), made once. */
    static synchronized ServerLevel level(String dimension) {
        setUp();
        return LEVELS.computeIfAbsent(dimension, d -> {
            ServerLevel level = Allocate.make(ServerLevel.class);
            LevelAccess access = (LevelAccess) level;
            ResourceKey<Level> key = ResourceKey.create(Registries.DIMENSION, new ResourceLocation(d));
            access.horizon$setDimension(key);
            access.horizon$setRandom(RandomSource.create());
            access.horizon$setDimensionType(overworldType);
            access.horizon$setThreadSafeRandom(RandomSource.createThreadSafe());
            access.horizon$setWorldBorder(new net.minecraft.world.level.border.WorldBorder());
            access.horizon$setLevelData(worldData);
            ((ServerLevelAccess) level).horizon$setServerLevelData(worldData);
            ServerLevelAccess sl = (ServerLevelAccess) level;
            sl.horizon$setServer(server);
            // What the skipped constructor makes that mods' code reaches.
            sl.horizon$setPlayers(new java.util.ArrayList<>());
            sl.horizon$setNavigatingMobs(new it.unimi.dsi.fastutil.objects.ObjectOpenHashSet<>());
            sl.horizon$setBlockEvents(new it.unimi.dsi.fastutil.objects.ObjectLinkedOpenHashSet<>());
            sl.horizon$setBlockEventsToReschedule(new java.util.ArrayList<>(64));
            sl.horizon$setCustomSpawners(java.util.List.of());
            sl.horizon$setDragonParts(new it.unimi.dsi.fastutil.ints.Int2ObjectOpenHashMap<>());
            sl.horizon$setSleepStatus(new net.minecraft.server.players.SleepStatus());
            // Ticks mods schedule: kept, not run (Horizon's ticks: a later step).
            sl.horizon$setBlockTicks(new net.minecraft.world.ticks.LevelTicks<>(pos -> true, () -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE));
            sl.horizon$setFluidTicks(new net.minecraft.world.ticks.LevelTicks<>(pos -> true, () -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE));
            sl.horizon$setRandomSequences(new net.minecraft.world.RandomSequences(0L));
            // A chunk source holding no chunks, with the level's saved data (Fabric API's
            // attachments, mods' SavedData) in the runtime's folder.
            var chunks = Allocate.make(net.minecraft.server.level.ServerChunkCache.class);
            var chunkAccess = (dev.horizonlab.platform120.mixin.ServerChunkCacheAccess) chunks;
            chunkAccess.horizon$setLevel(level);
            java.io.File data = new java.io.File("horizon-mirror/" + d.replace(':', '/') + "/data");
            data.mkdirs();
            chunkAccess.horizon$setDataStorage(new net.minecraft.world.level.storage.DimensionDataStorage(data, net.minecraft.util.datafix.DataFixers.getDataFixer()));
            // An empty chunk storage behind it. Nothing here holds chunks, but mods reach this
            // field through their own mixins to ask who is watching a chunk before sending a
            // block entity's network update (LibNetworkStack does, so Wired Redstone does when
            // it places a cable), and a null one throws inside the mod where nothing can answer.
            // Empty maps make every such lookup find nobody, which is the truth: Horizon has
            // the players, not this runtime.
            var chunkMap = Allocate.make(net.minecraft.server.level.ChunkMap.class);
            var mapAccess = (dev.horizonlab.platform120.mixin.ChunkMapAccess) chunkMap;
            // Every position answers with one holder that holds no chunk, rather than with
            // nothing: a mod asks for a holder and uses what comes back without checking
            // (LibNetworkStack calls getTickingChunk straight on it), so an absent one throws
            // inside the mod. A holder with no chunk sends it down its own empty path instead.
            // The default return value covers a lookup by either route -- the map directly, or
            // getVisibleChunkIfPresent -- since a mod's mixin may use either.
            var holder = Allocate.make(net.minecraft.server.level.ChunkHolder.class);
            var visible = new it.unimi.dsi.fastutil.longs.Long2ObjectLinkedOpenHashMap<
                    net.minecraft.server.level.ChunkHolder>();
            visible.defaultReturnValue(holder);
            var updating = new it.unimi.dsi.fastutil.longs.Long2ObjectLinkedOpenHashMap<
                    net.minecraft.server.level.ChunkHolder>();
            updating.defaultReturnValue(holder);
            mapAccess.horizon$setVisibleChunkMap(visible);
            mapAccess.horizon$setUpdatingChunkMap(updating);
            chunkAccess.horizon$setChunkMap(chunkMap);
            sl.horizon$setChunkSource(chunks);
            // Per-level state mods hang off the chunk storage rather than the level: GraphLib
            // builds the graph world every Wired Redstone cable is a node in at the end of
            // ChunkMap's constructor, and without it the cable's first tick throws before it
            // can run. The save access is what those handlers ask for the folder to keep their
            // own files in; the mirror's is beside the level's.
            var storage = Allocate.make(
                    net.minecraft.world.level.storage.LevelStorageSource.LevelStorageAccess.class);
            ((dev.horizonlab.platform120.mixin.LevelStorageAccessAccess) storage)
                    .horizon$setLevelDirectory(
                            new net.minecraft.world.level.storage.LevelStorageSource.LevelDirectory(
                                    java.nio.file.Path.of("horizon-mirror")));
            Constructed120.run(chunkMap, level, storage, net.minecraft.util.datafix.DataFixers.getDataFixer());
            // What mods do at the end of Level's and ServerLevel's constructors, with the
            // arguments a server gives them (Lithium's height cache, ...).
            java.util.function.Supplier<net.minecraft.util.profiling.ProfilerFiller> profiler = () -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE;
            Constructed120.run(level, worldData, key, registries, overworldType, profiler, server, java.util.List.of());
            return level;
        });
    }

    /** Horizon's player as a ServerPlayer of `level`, made once (vanilla's constructor). */
    static synchronized ServerPlayer player(String uuid, String name, ServerLevel level) {
        UUID id = UUID.fromString(uuid);
        ServerPlayer player = PLAYERS.get(id);
        if (player == null) {
            player = new ServerPlayer(server, level, new GameProfile(id, name));
            // A packet listener that swallows what is sent to it. A mod telling "the player"
            // about something sends through here, and vanilla's send would throw on the
            // connection a mirror player has not got (ServerGamePacketListenerMixin).
            player.connection = Allocate.make(
                    net.minecraft.server.network.ServerGamePacketListenerImpl.class);
            PLAYERS.put(id, player);
        }
        if (player.level() != level) player.setServerLevel(level);
        return player;
    }

    /** The dimension a mirror level stands for. */
    static String dimension(Level level) {
        return level.dimension().location().toString();
    }

    /** A line for Horizon's log. */
    static void log(String line) {
        var e = event;
        if (e != null) e.accept("log", new Object[] {line});
        else System.out.println("[horizon-platform120] " + line);
    }

    /**
     * A new call from Horizon: what the last one wrote is Horizon's now.
     *
     * <p>The block entities are not cleared. They stay alive between calls, as the 26.3
     * bridge's do: a machine ticking twenty times a second would otherwise be read from
     * Horizon and written back on each tick, and anything a mod keeps in a field it never
     * saves would be lost between them. What is cleared is this call's own view: the writes,
     * and the positions found empty (a block can gain a block entity between calls).
     */
    static void begin() {
        WRITES.clear();
        READS.clear();
        NO_ENTITY.clear();
    }

    /**
     * The call is over: every block entity a mod changed goes back to Horizon, which keeps it
     * (a mod's own type as opaque NBT). Compared as this side writes them, so a difference is
     * the mod's change and not another serializer's spelling.
     */
    static void finish() {
        for (Map.Entry<Level, Map<Long, BlockEntity>> level : ENTITIES.entrySet()) {
            String dimension = dimension(level.getKey());
            for (Map.Entry<Long, BlockEntity> made : level.getValue().entrySet()) {
                BlockEntity entity = made.getValue();
                String now;
                try {
                    now = entity.saveWithFullMetadata().toString();
                } catch (RuntimeException e) {
                    log("a block entity didn't save: " + e);
                    continue;
                }
                if (now.equals(ENTITY_NBT.get(entity))) continue;
                BlockPos pos = BlockPos.of(made.getKey());
                event.accept("setBlockEntity",
                        new Object[] {dimension, pos.getX(), pos.getY(), pos.getZ(), now});
                // Horizon has this now: only a later change goes back.
                ENTITY_NBT.put(entity, now);
            }
        }
    }

    /**
     * Level.getBlockEntity of a mirror level: the one this call already made there, else
     * Horizon's, loaded as a chunk loads it (BlockEntity.loadStatic, so a mod's own type is
     * made by its own code from its own NBT). Null where Horizon has none.
     */
    public static BlockEntity blockEntity(Level level, BlockPos pos) {
        long key = pos.asLong();
        Map<Long, BlockEntity> made = ENTITIES.get(level);
        BlockEntity known = made == null ? null : made.get(key);
        if (known != null) {
            // Still the block it belongs to: the same object, as a chunk would keep it. The
            // state can still have moved on under it (a furnace lighting), so it is told.
            BlockState now = blockState(level, pos);
            if (known.getType().isValid(now)) {
                if (known.getBlockState() != now) known.setBlockState(now);
                return known;
            }
            // The block there is another one now: that block entity is gone with it.
            made.remove(key);
            ENTITY_NBT.remove(known);
        }
        java.util.Set<Long> none = NO_ENTITY.get(level);
        if (none != null && none.contains(key)) return null;
        if (level.isOutsideBuildHeight(pos)) return null;
        Object[] answer = call.apply("blockEntity",
                new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ()});
        String snbt = answer.length > 0 && answer[0] instanceof String s ? s : null;
        BlockEntity entity = snbt == null ? null : load(level, pos, snbt);
        if (entity == null) {
            NO_ENTITY.computeIfAbsent(level, l -> new java.util.HashSet<>()).add(key);
            return null;
        }
        keep(level, key, entity, entity.saveWithFullMetadata().toString());
        return entity;
    }

    /** Horizon's block entity NBT as a 1.20.1 block entity; null when it can't be read. */
    private static BlockEntity load(Level level, BlockPos pos, String snbt) {
        try {
            var tag = net.minecraft.nbt.TagParser.parseTag(snbt);
            BlockEntity entity = BlockEntity.loadStatic(pos.immutable(), blockState(level, pos), tag);
            if (entity != null) entity.setLevel(level);
            return entity;
        } catch (Exception e) {
            // A type 1.20.1 hasn't got, or NBT it won't read: no block entity there.
            log("the block entity at " + pos.toShortString() + " didn't load: " + e);
            return null;
        }
    }

    /**
     * Level.addFreshEntity on a mirror level. Horizon owns this world's entities and a mirror
     * level has nowhere to put one, so an item a mod drops or ejects is sent for Horizon to
     * spawn (a chest spilling its contents, a machine putting its output into the world), and
     * anything else is turned away until entities are a phase of their own.
     *
     * <p>The stack crosses as its id, count and damage, as the held one does. What a mod put
     * in its NBT (an enchantment, a mod's own tag) is lost on the way, and will keep being
     * lost until the downgrade tables land.
     *
     * @return whether the entity was taken, which is what vanilla's caller reads.
     */
    public static boolean addEntity(Level level, net.minecraft.world.entity.Entity entity) {
        if (!(entity instanceof net.minecraft.world.entity.item.ItemEntity dropped)) {
            notEntities(entity);
            return false;
        }
        ItemStack stack = dropped.getItem();
        if (stack.isEmpty()) return false;
        var motion = dropped.getDeltaMovement();
        // The whole stack as 1.20.1 saves it, not just its name and count: an enchanted tool a
        // machine spat out keeps its enchantments, a renamed one its name. Horizon runs it
        // through vanilla's own 1.20.1 -> 26.3 fixes on the other side.
        event.accept("spawnItem", new Object[] {
            dimension(level), dropped.getX(), dropped.getY(), dropped.getZ(),
            motion.x, motion.y, motion.z,
            stack.save(new net.minecraft.nbt.CompoundTag()).toString(),
        });
        return true;
    }

    /** Said once per kind: the entities a mirror level turns away (phase 6 is their own). */
    private static void notEntities(net.minecraft.world.entity.Entity entity) {
        String kind = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE
                .getKey(entity.getType()).toString();
        if (TURNED_AWAY.add(kind)) {
            log("a mod put a " + kind + " into the world; only dropped items arrive so far");
        }
    }

    private static final java.util.Set<String> TURNED_AWAY = new java.util.HashSet<>();

    /** Level.setBlockEntity: a mod's own, for this call and for Horizon when it ends. */
    public static void addBlockEntity(Level level, BlockEntity entity) {
        entity.setLevel(level);
        keep(level, entity.getBlockPos().asLong(), entity, "");
    }

    /** Level.removeBlockEntity: gone for the rest of this call. */
    public static void removeBlockEntity(Level level, BlockPos pos) {
        Map<Long, BlockEntity> made = ENTITIES.get(level);
        if (made != null) {
            BlockEntity gone = made.remove(pos.asLong());
            if (gone != null) ENTITY_NBT.remove(gone);
        }
        NO_ENTITY.computeIfAbsent(level, l -> new java.util.HashSet<>()).add(pos.asLong());
    }

    /** A block entity of this call, and what it held when it was made. */
    private static void keep(Level level, long key, BlockEntity entity, String nbt) {
        ENTITIES.computeIfAbsent(level, l -> new HashMap<>()).put(key, entity);
        ENTITY_NBT.put(entity, nbt);
        java.util.Set<Long> none = NO_ENTITY.get(level);
        if (none != null) none.remove(key);
    }

    /** Level.getBlockState of a mirror level: this call's own write, else Horizon's block. */
    public static BlockState blockState(Level level, BlockPos pos) {
        Map<Long, BlockState> writes = WRITES.get(level);
        if (writes != null) {
            BlockState written = writes.get(pos.asLong());
            if (written != null) return written;
        }
        if (level.isOutsideBuildHeight(pos)) return Blocks.VOID_AIR.defaultBlockState();
        Map<Long, BlockState> read = READS.computeIfAbsent(level, l -> new HashMap<>());
        BlockState known = read.get(pos.asLong());
        if (known != null) return known;
        // Not read yet: this position and the ones around it, in one call. Code that looks at a
        // block almost always looks at its neighbours too (what a pipe connects to, whether a
        // crop has soil under it), and one round trip for the lot beats one each.
        fill(level, pos, read);
        known = read.get(pos.asLong());
        // Where Horizon has no chunk loaded: air, as vanilla reads an unloaded position.
        return known == null ? Blocks.AIR.defaultBlockState() : known;
    }

    /** The 3x3x3 around `pos` into `read`, in one call. */
    private static void fill(Level level, BlockPos pos, Map<Long, BlockState> read) {
        int[] wanted = new int[3 * 27];
        long[] keys = new long[27];
        int n = 0;
        for (int dx = -1; dx <= 1; dx++) {
            for (int dy = -1; dy <= 1; dy++) {
                for (int dz = -1; dz <= 1; dz++) {
                    int x = pos.getX() + dx;
                    int y = pos.getY() + dy;
                    int z = pos.getZ() + dz;
                    // Already known, or not a position Horizon has: left out of the ask.
                    if (y < level.getMinBuildHeight() || y >= level.getMaxBuildHeight()) continue;
                    long key = BlockPos.asLong(x, y, z);
                    if (read.containsKey(key)) continue;
                    wanted[n * 3] = x;
                    wanted[n * 3 + 1] = y;
                    wanted[n * 3 + 2] = z;
                    keys[n] = key;
                    n++;
                }
            }
        }
        if (n == 0) return;
        int[] ask = n * 3 == wanted.length ? wanted : java.util.Arrays.copyOf(wanted, n * 3);
        Object[] answer = call.apply("blockStates", new Object[] {dimension(level), ask});
        if (answer.length == 0 || !(answer[0] instanceof String[] names)) return;
        for (int i = 0; i < n && i < names.length; i++) {
            // "" is Horizon saying it has no chunk there, which reads as air.
            read.put(keys[i], names[i].isEmpty() ? Blocks.AIR.defaultBlockState() : state(names[i]));
        }
    }

    /** Level.setBlock of a mirror level: kept for this call and sent to Horizon. */
    public static boolean setBlock(Level level, BlockPos pos, BlockState state, int flags) {
        if (level.isOutsideBuildHeight(pos)) return false;
        BlockState before = blockState(level, pos);
        if (before == state) return false;
        long key = pos.asLong();
        WRITES.computeIfAbsent(level, l -> new HashMap<>()).put(key, state);
        event.accept("setBlock", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ(), BlockStateParser.serialize(state), flags});
        // The block entity the new block has: vanilla's LevelChunk.setBlockState makes it, and
        // a mirror level has no chunks to do it. Made here, so the mod's own code reads it back
        // in this call (a cooking pot it just placed) and Horizon gets it when the call ends.
        if (state.getBlock() instanceof net.minecraft.world.level.block.EntityBlock block) {
            // The one already there, when the new state is still its block: vanilla keeps it,
            // and a mod sets a block on the way through its own block entity's code (a jukebox
            // turns itself `has_record=true` while holding the record it was just given). A new
            // one here would be the empty one, and the change would be lost.
            BlockEntity had = ENTITIES.getOrDefault(level, Map.of()).get(key);
            if (had != null && had.getType().isValid(state)) return true;
            BlockEntity entity = block.newBlockEntity(pos.immutable(), state);
            if (entity != null) {
                entity.setLevel(level);
                // "": never what it saves, so a new one always reaches Horizon.
                keep(level, key, entity, "");
                return true;
            }
        }
        // The block there has none now: forget the one the old block had.
        Map<Long, BlockEntity> made = ENTITIES.get(level);
        if (made != null) {
            BlockEntity gone = made.remove(key);
            if (gone != null) ENTITY_NBT.remove(gone);
        }
        return true;
    }

    /**
     * Horizon's state name as a 1.20.1 state. Names are 26.3's: a property 1.20.1 doesn't have
     * is dropped (the block's other properties kept), and a block 1.20.1 doesn't have reads as
     * stone, a solid block where one is (phase 3's placeholder).
     */
    static BlockState state(String name) {
        BlockState known = STATES.get(name);
        if (known != null) return known;
        BlockState state;
        try {
            state = BlockStateParser.parseForBlock(BuiltInRegistries.BLOCK.asLookup(), name, false).blockState();
        } catch (Exception e) {
            state = closest(name);
        }
        STATES.put(name, state);
        return state;
    }

    private static BlockState closest(String name) {
        int bracket = name.indexOf('[');
        String id = bracket < 0 ? name : name.substring(0, bracket);
        ResourceLocation key = ResourceLocation.tryParse(id);
        if (key == null || !BuiltInRegistries.BLOCK.containsKey(key)) return Blocks.STONE.defaultBlockState();
        BlockState state = BuiltInRegistries.BLOCK.get(key).defaultBlockState();
        if (bracket < 0) return state;
        for (String pair : name.substring(bracket + 1, name.length() - 1).split(",")) {
            int eq = pair.indexOf('=');
            if (eq < 0) continue;
            String prop = pair.substring(0, eq), value = pair.substring(eq + 1);
            var property = state.getBlock().getStateDefinition().getProperty(prop);
            if (property == null) continue;
            state = with(state, property, value);
        }
        return state;
    }

    private static <T extends Comparable<T>> BlockState with(BlockState state, net.minecraft.world.level.block.state.properties.Property<T> property, String value) {
        return property.getValue(value).map(v -> state.setValue(property, v)).orElse(state);
    }

    /**
     * A player of Horizon's uses its held item on a block (right-click): vanilla's
     * ServerPlayerGameMode.useItemOn, so the block's own use, Fabric API's UseBlockCallback and
     * the item's useOn run as they do in 1.20.1. Returns the result (0 pass, 1 consumed, 2
     * consumed with a swing) and the held item after: id, count, damage.
     */
    public static Object[] useBlock(String uuid, String name, String dimension, int x, int y, int z, int face,
            double hx, double hy, double hz, int hand, String item, int count, int damage, boolean creative) {
        begin();
        ServerLevel level = level(dimension);
        ServerPlayer player = player(uuid, name, level);
        player.setPos(hx, hy - 1.62, hz);
        GameType mode = creative ? GameType.CREATIVE : GameType.SURVIVAL;
        if (player.gameMode.getGameModeForPlayer() != mode) {
            ((dev.horizonlab.platform120.mixin.GameModeAccess) player.gameMode).horizon$setGameMode(mode, player.gameMode.getGameModeForPlayer());
        }
        InteractionHand h = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
        ItemStack stack = stack(item, count, damage);
        player.setItemInHand(h, stack);
        BlockHitResult hit = new BlockHitResult(new Vec3(hx, hy, hz), Direction.from3DDataValue(face), new BlockPos(x, y, z), false);
        var menuBefore = player.containerMenu;
        InteractionResult result = player.gameMode.useItemOn(player, level, stack, h, hit);
        ItemStack after = player.getItemInHand(h);
        finish();
        int code = !result.consumesAction() ? 0 : result.shouldSwing() ? 2 : 1;
        // A menu the click opened (a cooking pot, a chest): its type and title, for Horizon to
        // show its screen. Empty when none did.
        String[] menu = Screens120.openedSince(player, menuBefore);
        return new Object[] {code, BuiltInRegistries.ITEM.getKey(after.getItem()).toString(), after.getCount(), after.getDamageValue(),
                menu == null ? "" : menu[0], menu == null ? "" : menu[1]};
    }

    /**
     * One tick of the mods' block entities at `positions` (x, y, z each) of `dimension`: a
     * cooking pot cooks, a machine runs. Each is loaded from Horizon as a chunk would load it
     * ({@link #blockEntity}), its block's own ticker is run once, and what changed goes back
     * when the call ends ({@link #finish}) -- the same path a right-click takes.
     *
     * <p>Loaded and saved per tick rather than kept alive between them: a block entity's state
     * is what it saves (that is how one survives a restart), so a tick reads what the last one
     * wrote. A mod that keeps something only in a field, never in its NBT, loses it between
     * ticks; keeping them live is the next step, and the one that makes this cheap.
     */
    public static Object[] tickBlockEntities(String dimension, int[] positions) {
        begin();
        ServerLevel level = level(dimension);
        int ticked = 0;
        for (int i = 0; i + 2 < positions.length; i += 3) {
            BlockPos pos = new BlockPos(positions[i], positions[i + 1], positions[i + 2]);
            if (tickOne(level, pos)) ticked++;
        }
        finish();
        return new Object[] {ticked};
    }

    /**
     * One frame of the screen of the menu `uuid`'s player has open (a cooking pot's), drawn at
     * `width` by `height` with the mouse at (`mouseX`, `mouseY`): the operations Horizon's
     * screen drawing reads, as JSON. Empty when they have no menu open, or its mod registered
     * no screen for it.
     */
    public static Object[] screenFrame(String uuid, String inventory, double x, double y, double z,
            int width, int height, int mouseX, int mouseY) {
        ServerPlayer player = PLAYERS.get(UUID.fromString(uuid));
        if (player == null) return new Object[] {""};
        begin();
        try {
            // The inventory slots the screen shows are Horizon's, as they are now, and the
            // player is where Horizon has them: a menu asks that to stay open.
            takeInventory(player, inventory);
            player.setPos(x, y, z);
            String frame = Screens120.frame(player, width, height, mouseX, mouseY);
            return new Object[] {frame == null ? "" : frame};
        } finally {
            finish();
        }
    }

    /**
     * One input of `uuid`'s player on their open screen (Screens120.input), with Horizon's
     * inventory as it is now ({@link #takeInventory}). Gives whether the screen is still open,
     * and the inventory slots the input changed as JSON ({@code {"<slot>": "<1.20.1 item NBT>"}},
     * an empty string for a slot emptied): a stack taken into the pot, one shift-clicked out,
     * what was on the cursor put back as the screen closed. What it did to the block entities
     * goes back as every call's does ({@link #finish}).
     */
    public static Object[] screenInput(String uuid, String inventory, String input) {
        ServerPlayer player = PLAYERS.get(UUID.fromString(uuid));
        if (player == null) return new Object[] {false, "{}"};
        begin();
        String[] given = takeInventory(player, inventory);
        boolean open;
        try {
            open = Screens120.input(player, input);
        } finally {
            finish();
        }
        com.google.gson.JsonObject changed = new com.google.gson.JsonObject();
        var items = player.getInventory().items;
        for (int i = 0; i < given.length && i < items.size(); i++) {
            ItemStack now = items.get(i);
            if (!key(now).equals(given[i])) {
                changed.addProperty(Integer.toString(i), now.isEmpty() ? "" : now.save(new net.minecraft.nbt.CompoundTag()).toString());
            }
        }
        return new Object[] {open, changed.toString()};
    }

    /**
     * Horizon's inventory onto the player here: `json` is its 36 main slots, hotbar first, each
     * {@code [id, count, damage]} or null. A slot already holding that stack keeps the one it
     * has, with whatever NBT it carries; the others take Horizon's. Gives each slot's key as
     * given, for saying afterwards which ones changed.
     */
    private static String[] takeInventory(ServerPlayer player, String json) {
        var items = player.getInventory().items;
        var slots = com.google.gson.JsonParser.parseString(json == null || json.isEmpty() ? "[]" : json).getAsJsonArray();
        String[] given = new String[Math.min(slots.size(), items.size())];
        for (int i = 0; i < given.length; i++) {
            var slot = slots.get(i);
            ItemStack stack = ItemStack.EMPTY;
            if (slot != null && slot.isJsonArray()) {
                var a = slot.getAsJsonArray();
                stack = stack(a.get(0).getAsString(), a.get(1).getAsInt(), a.get(2).getAsInt());
            }
            given[i] = key(stack);
            if (!key(items.get(i)).equals(given[i])) items.set(i, stack);
        }
        return given;
    }

    /** A stack as Horizon tells one apart: its item, count and damage. */
    private static String key(ItemStack stack) {
        if (stack.isEmpty()) return "";
        return BuiltInRegistries.ITEM.getKey(stack.getItem()) + "," + stack.getCount() + "," + stack.getDamageValue();
    }

    /** What a screen's drawing records: {@link Screens120#probe}. */
    public static Object[] screenProbe() {
        return Screens120.probe();
    }

    /**
     * The shapes of the blocks at `positions` (x, y, z each), as the mods give them *here*: a
     * block whose shape is its block entity's has none worth exporting (LibMultiPart's container
     * answers an empty shape with no parts in it, which is every one of its states in the
     * registry), so Horizon asks per position instead.
     *
     * <p>Flat, one position after another: a count of boxes, then that many boxes of six
     * doubles (x0 y0 z0 x1 y1 z1, inside the block). The outline shape, the one a click is
     * aimed at; a block whose outline is empty cannot be looked at or mined.
     */
    public static Object[] blockShapes(String dimension, int[] positions) {
        begin();
        ServerLevel level = level(dimension);
        java.util.List<Double> out = new java.util.ArrayList<>();
        for (int i = 0; i + 2 < positions.length; i += 3) {
            BlockPos pos = new BlockPos(positions[i], positions[i + 1], positions[i + 2]);
            int at = out.size();
            out.add(0.0);
            int boxes = 0;
            try {
                BlockState state = blockState(level, pos);
                var shape = state.getShape(level, pos);
                for (net.minecraft.world.phys.AABB box : shape.toAabbs()) {
                    out.add(box.minX);
                    out.add(box.minY);
                    out.add(box.minZ);
                    out.add(box.maxX);
                    out.add(box.maxY);
                    out.add(box.maxZ);
                    boxes++;
                }
            } catch (RuntimeException | LinkageError e) {
                // A mod that cannot say what shape it is keeps the one the registry has.
                shapeFailed(pos, e);
                while (out.size() > at + 1) out.remove(out.size() - 1);
                boxes = 0;
            }
            out.set(at, (double) boxes);
        }
        finish();
        double[] flat = new double[out.size()];
        for (int i = 0; i < flat.length; i++) flat[i] = out.get(i);
        return new Object[] {flat};
    }

    /** A shape that threw: said once, with where it was. */
    private static void shapeFailed(BlockPos pos, Throwable e) {
        if (SHAPE_FAILED.add(pos.toShortString())) {
            log("the shape of the block at " + pos.toShortString() + " could not be asked for: " + e);
        }
    }

    private static final java.util.Set<String> SHAPE_FAILED = new java.util.HashSet<>();

    /** One block entity's tick; false when there is nothing there to tick. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static boolean tickOne(ServerLevel level, BlockPos pos) {
        BlockState state = blockState(level, pos);
        if (!(state.getBlock() instanceof net.minecraft.world.level.block.EntityBlock block)) {
            return false;
        }
        BlockEntity entity = blockEntity(level, pos);
        if (entity == null) return false;
        if (TICK_FAILED.contains(kind(entity))) return false;
        net.minecraft.world.level.block.entity.BlockEntityTicker ticker =
                block.getTicker(level, state, entity.getType());
        if (ticker == null) return false;
        try {
            ticker.tick(level, pos, state, entity);
        } catch (RuntimeException | LinkageError e) {
            // One mod's bad tick must not stop the rest of them.
            tickFailed(entity, e);
            return false;
        }
        return true;
    }

    /** A block entity type's id, for saying which one misbehaved. */
    private static String kind(BlockEntity entity) {
        return BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(entity.getType()) + "";
    }

    /**
     * A tick that threw: said once, and that type is not ticked again. A mod whose tick fails
     * once fails every tick, and the world would fill with the same line twenty times a second.
     */
    private static void tickFailed(BlockEntity entity, Throwable e) {
        if (TICK_FAILED.add(kind(entity))) {
            log("the tick of " + kind(entity) + " failed, and is left out from now on: " + e);
        }
    }

    private static final java.util.Set<String> TICK_FAILED = new java.util.HashSet<>();

    private static ItemStack stack(String item, int count, int damage) {
        ResourceLocation id = ResourceLocation.tryParse(item);
        if (id == null || count <= 0) return ItemStack.EMPTY;
        Item it = BuiltInRegistries.ITEM.get(id);
        // The registry answers an id it hasn't got with air rather than nothing, so an item
        // 1.20.1 doesn't know would quietly become an empty hand and the click would pass with
        // no sign of why. Said once per id instead.
        if (it == net.minecraft.world.item.Items.AIR && !"minecraft:air".equals(item)) {
            if (UNKNOWN_ITEMS.add(item)) {
                log("this runtime has no item " + item + "; a click holding it is empty-handed");
            }
            return ItemStack.EMPTY;
        }
        ItemStack stack = new ItemStack(it, count);
        if (damage > 0) stack.setDamageValue(damage);
        return stack;
    }

    /** Item ids Horizon sent that 1.20.1 hasn't got, said once each. */
    private static final java.util.Set<String> UNKNOWN_ITEMS = new java.util.HashSet<>();
}
