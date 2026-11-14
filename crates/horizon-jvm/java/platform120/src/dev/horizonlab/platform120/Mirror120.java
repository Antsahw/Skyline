package dev.horizonlab.platform120;

import com.mojang.authlib.GameProfile;
import dev.horizonlab.platform120.mixin.LevelAccess;
import dev.horizonlab.platform120.mixin.ServerLevelAccess;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
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
        ((dev.horizonlab.platform120.mixin.PlayerListAccess) playerList).horizon$setPlayers(new java.util.concurrent.CopyOnWriteArrayList<>());
        ((dev.horizonlab.platform120.mixin.PlayerListAccess) playerList).horizon$setPlayersByUUID(new java.util.concurrent.ConcurrentHashMap<>());
        // Empty, as a new server's, never read from or written to a file: nobody is an op, and
        // a mod asking (a written book resolving its text asks the reader's permission level)
        // gets that answer.
        var access = (dev.horizonlab.platform120.mixin.PlayerListAccess) playerList;
        var folder = new java.io.File("horizon-mirror");
        access.horizon$setOps(new net.minecraft.server.players.ServerOpList(new java.io.File(folder, "ops.json")));
        access.horizon$setBans(new net.minecraft.server.players.UserBanList(new java.io.File(folder, "banned-players.json")));
        access.horizon$setIpBans(new net.minecraft.server.players.IpBanList(new java.io.File(folder, "banned-ips.json")));
        access.horizon$setWhitelist(new net.minecraft.server.players.UserWhiteList(new java.io.File(folder, "whitelist.json")));
        ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setLevels(SERVER_LEVELS);
        ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setStorageSource(storage());
        // Made from no file: the constructor reads server.properties, and logs an error when
        // there is none. Vanilla's defaults are what an empty Properties gives.
        var settings = Allocate.make(net.minecraft.server.dedicated.DedicatedServerSettings.class);
        ((dev.horizonlab.platform120.mixin.DedicatedServerSettingsAccess) settings).horizon$setProperties(
                new net.minecraft.server.dedicated.DedicatedServerProperties(new java.util.Properties()));
        ((dev.horizonlab.platform120.mixin.DedicatedServerAccess) server).horizon$setSettings(settings);
        // The registries as vanilla's WorldLoader makes them: the built-in ones, then the data
        // packs' worldgen and dimensions (vanilla's and the mods'), then the reloadable data:
        // recipes, loot, tags (bound into the registries: mods ask `state.is(BlockTags.CAMPFIRES)`),
        // advancements and functions.
        var packs = Tags120.dataPacks();
        dataPacks = packs;
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

    private static net.minecraft.server.packs.resources.ResourceManager dataPacks;

    /** The data packs the registries were loaded from (vanilla's and the mods'). */
    static net.minecraft.server.packs.resources.ResourceManager dataPacks() {
        setUp();
        return dataPacks;
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

    /**
     * The dimension type of the level `dimension`: its level stem's (a mod's dimension: its
     * build range, skylight, ceiling), vanilla's Nether and End theirs, else the Overworld's.
     */
    static Holder<DimensionType> typeOf(String dimension) {
        var types = registries.registryOrThrow(Registries.DIMENSION_TYPE);
        switch (dimension) {
            case "minecraft:the_nether":
                return types.getHolderOrThrow(BuiltinDimensionTypes.NETHER);
            case "minecraft:the_end":
                return types.getHolderOrThrow(BuiltinDimensionTypes.END);
            default:
                break;
        }
        var stem = registries.registry(Registries.LEVEL_STEM)
                .map(r -> r.get(new ResourceLocation(dimension))).orElse(null);
        return stem != null ? stem.type() : overworldType;
    }

    /** A level of Horizon's (`minecraft:overworld`), made once. */
    private static synchronized ServerLevel makeLevel(String dimension) {
        setUp();
        return LEVELS.computeIfAbsent(dimension, d -> {
            ServerLevel level = Allocate.make(ServerLevel.class);
            LevelAccess access = (LevelAccess) level;
            ResourceKey<Level> key = ResourceKey.create(Registries.DIMENSION, new ResourceLocation(d));
            access.horizon$setDimension(key);
            access.horizon$setRandom(RandomSource.create());
            access.horizon$setDimensionType(typeOf(d));
            access.horizon$setThreadSafeRandom(RandomSource.createThreadSafe());
            access.horizon$setWorldBorder(new net.minecraft.world.level.border.WorldBorder());
            access.horizon$setLevelData(worldData);
            // A mob's goals and paths profile themselves, as a quiet server's do: into nothing.
            access.horizon$setProfiler(() -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE);
            // What hurts a mob (a fall, a player): the damage types of this world's registries.
            access.horizon$setDamageSources(new net.minecraft.world.damagesource.DamageSources(registries()));
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
            // The mods' fluids' ticks are kept and run (FluidTicks120).
            sl.horizon$setFluidTicks(new FluidTicks120());
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
            Constructed120.run(chunkMap, level, storage(), net.minecraft.util.datafix.DataFixers.getDataFixer());
            // What mods do at the end of Level's and ServerLevel's constructors, with the
            // arguments a server gives them (Lithium's height cache, ...).
            java.util.function.Supplier<net.minecraft.util.profiling.ProfilerFiller> profiler = () -> net.minecraft.util.profiling.InactiveProfiler.INSTANCE;
            Constructed120.run(level, worldData, key, registries, typeOf(d), profiler, server, java.util.List.of());
            return level;
        });
    }

    /**
     * {@link #level(String)} with the level on the server's own list (overworld(), getLevel,
     * getAllLevels find it) and, once the server has started, loaded as a server loads one
     * (Fabric's ServerWorldEvents.LOAD).
     */
    static synchronized ServerLevel level(String dimension) {
        boolean known = LEVELS.containsKey(dimension);
        ServerLevel level = makeLevel(dimension);
        if (!known) {
            serverLevels().put(level.dimension(), level);
            if (Events120.started()) Events120.loaded(server, level);
        }
        return level;
    }

    private static net.minecraft.world.level.storage.LevelStorageSource.LevelStorageAccess storage;

    /**
     * The save access a level's handlers, and the server's (getWorldPath: Porting Lib's
     * server configs), ask for the folder to keep their own files in: the runtime's own
     * `horizon-mirror`, never Horizon's world.
     */
    static synchronized net.minecraft.world.level.storage.LevelStorageSource.LevelStorageAccess storage() {
        if (storage == null) {
            storage = Allocate.make(net.minecraft.world.level.storage.LevelStorageSource.LevelStorageAccess.class);
            ((dev.horizonlab.platform120.mixin.LevelStorageAccessAccess) storage)
                    .horizon$setLevelDirectory(
                            new net.minecraft.world.level.storage.LevelStorageSource.LevelDirectory(
                                    java.nio.file.Path.of("horizon-mirror")));
            ((dev.horizonlab.platform120.mixin.LevelStorageAccessAccess) storage)
                    .horizon$setResources(new java.util.concurrent.ConcurrentHashMap<>());
        }
        return storage;
    }

    /** The server's own list of its levels (MinecraftServer.levels), the same map. */
    private static final Map<ResourceKey<Level>, ServerLevel> SERVER_LEVELS = new java.util.LinkedHashMap<>();

    private static Map<ResourceKey<Level>, ServerLevel> serverLevels() {
        return SERVER_LEVELS;
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
            // Whose connection it is: a packet sent through it goes to this player's client
            // half (ServerGamePacketListenerMixin, Packets120).
            player.connection.player = player;
            ((dev.horizonlab.platform120.mixin.ServerGamePacketListenerAccess) player.connection)
                    .horizon$setConnection(Allocate.make(net.minecraft.network.Connection.class));
            PLAYERS.put(id, player);
            // On the server's player list, as a player who joined is: a mod's tick looking for
            // the players (getPlayers, getPlayer by UUID) finds Horizon's.
            playerList.getPlayers().add(player);
            ((dev.horizonlab.platform120.mixin.PlayerListAccess) playerList).horizon$setPlayersByUUID(
                    playersByUuid());
            Events120.joined(server, player);
        }
        if (player.level() != level) player.setServerLevel(level);
        return player;
    }

    /** A server's view distance in chunks, as server.properties has it by default. */
    private static final int VIEW_DISTANCE = 10;

    /** The mirror's players in `level` who see the chunk at `pos` (PlayerLookupMixin). */
    public static java.util.Collection<ServerPlayer> tracking(net.minecraft.server.level.ServerLevel level,
            net.minecraft.world.level.ChunkPos pos) {
        List<ServerPlayer> out = new ArrayList<>();
        for (ServerPlayer player : PLAYERS.values()) {
            if (player.level() != level) continue;
            var at = player.chunkPosition();
            if (Math.abs(at.x - pos.x) <= VIEW_DISTANCE && Math.abs(at.z - pos.z) <= VIEW_DISTANCE) out.add(player);
        }
        return out;
    }

    private static Map<UUID, ServerPlayer> playersByUuid() {
        var map = new java.util.concurrent.ConcurrentHashMap<UUID, ServerPlayer>(PLAYERS);
        return map;
    }

    private static volatile int probeClientGot = -1;
    private static volatile int probeServerGot = -1;
    private static boolean probeRegistered;

    /**
     * A round trip through Fabric's networking, for the tests: the server sends `uuid`'s player
     * 41 on `horizon:probe_down`; the client half's receiver (on Minecraft's execute, as mods
     * hand their work on) sends back one more on `horizon:probe_up`, which the server's receiver
     * (on the server's execute) keeps. Gives what each side got (-1: nothing) and whether each
     * side could say it can send (canSend). The receivers are registered on the first call.
     */
    public static Object[] packetProbe(String uuid, String name) {
        begin();
        try {
            ServerLevel level = level("minecraft:overworld");
            ServerPlayer player = player(uuid, name, level);
            ResourceLocation down = new ResourceLocation("horizon", "probe_down");
            ResourceLocation up = new ResourceLocation("horizon", "probe_up");
            if (!probeRegistered) {
                probeRegistered = true;
                net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.registerGlobalReceiver(down,
                        (client, handler, buf, sender) -> {
                            int got = buf.readVarInt();
                            client.execute(() -> {
                                probeClientGot = got;
                                var reply = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
                                reply.writeVarInt(got + 1);
                                net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.send(up, reply);
                            });
                        });
                net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.registerGlobalReceiver(up,
                        (server, from, handler, buf, sender) -> {
                            int got = buf.readVarInt();
                            server.execute(() -> probeServerGot = got);
                        });
            }
            probeClientGot = -1;
            probeServerGot = -1;
            boolean canDown = net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.canSend(player, down);
            var buf = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
            buf.writeVarInt(41);
            net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking.send(player, down, buf);
            boolean canUp = net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.canSend(up);
            return new Object[] {probeClientGot, probeServerGot, canDown, canUp};
        } finally {
            finish();
        }
    }

    /**
     * One server tick of `dimension` at Horizon's game time `gameTime` (phase 5): the 1.20.1
     * mods hear it through Fabric API's tick events (Events120), the server started first if it
     * hasn't. The server's tick count is Horizon's, so a mod doing something every 20 ticks
     * does it every second. Gives whether any mod listens to ticks: when none does, Horizon
     * asks less often. What the mods' listeners set and dropped goes back as every call's does.
     */
    public static Object[] serverTick(String dimension, long gameTime) {
        begin();
        try {
            ServerLevel level = level(dimension);
            worldData.setGameTime(gameTime);
            ((dev.horizonlab.platform120.mixin.MinecraftServerAccess) server).horizon$setTickCount((int) gameTime);
            Events120.start(server, LEVELS.values());
            // The players' client ticks, where the mods read the keys pressed since the last.
            Keys120.tick(PLAYERS);
            if (!Events120.anyTickListeners()) return new Object[] {false};
            Events120.tick(server, level, gameTime);
            return new Object[] {true};
        } finally {
            finish();
        }
    }

    /** The dimension a mirror level stands for. */
    static String dimension(Level level) {
        return level.dimension().location().toString();
    }

    /** A line for Horizon's log. */
    public static void log(String line) {
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
        BIOMES.clear();
        WRITES.clear();
        READS.clear();
        LIGHT_READS.clear();
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
        if (known != null && key == removing) return known;
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
        if (entity == null && snbt == null) {
            // A mod's block that has one, where Horizon has none (placed by a command, or by a
            // structure): a chunk makes it with the block (LevelChunk.setBlockState), so it is
            // made here, and goes to Horizon with what the call changed. Vanilla's blocks'
            // are Horizon's to make.
            BlockState state = blockState(level, pos);
            if (state.getBlock() instanceof net.minecraft.world.level.block.EntityBlock block
                    && !"minecraft".equals(BuiltInRegistries.BLOCK.getKey(state.getBlock()).getNamespace())) {
                BlockEntity fresh = block.newBlockEntity(pos.immutable(), state);
                if (fresh != null) {
                    addBlockEntity(level, fresh);
                    return fresh;
                }
            }
        }
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
        // A mod's own entity lives in the mirror, with its mod's code (Entities120).
        if (Entities120.modded(entity)) return Entities120.add(level, entity);
        // A vanilla mob the mods' code made (the cow a sheared moobloom turns into): Horizon's,
        // as vanilla's mobs are, from the entity as 1.20.1 saves it.
        if (entity instanceof net.minecraft.world.entity.Mob) {
            var tag = new net.minecraft.nbt.CompoundTag();
            if (entity.save(tag)) {
                event.accept("spawnEntity", new Object[] {dimension(level), tag.toString()});
                return true;
            }
        }
        // Experience a kill or a furnace gives: Horizon's own orbs, of the same value.
        if (entity instanceof net.minecraft.world.entity.ExperienceOrb orb) {
            event.accept("spawnXp", new Object[] {dimension(level), orb.getX(), orb.getY(), orb.getZ(), orb.getValue()});
            return true;
        }
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

    /** Each mirror level's light engine (Light120), made when first asked for. */
    private static final Map<Level, Light120> LIGHTS = new HashMap<>();

    /** A mirror level's light engine: Horizon's light. */
    public static synchronized net.minecraft.world.level.lighting.LevelLightEngine lightEngine(Level level) {
        return LIGHTS.computeIfAbsent(level, Light120::new);
    }

    /** Light Horizon has at `pos` (sky << 4 | block), read once per call; 15 sky where unloaded. */
    static int light(Level level, BlockPos pos) {
        Map<Long, Integer> read = LIGHT_READS.computeIfAbsent(level, l -> new HashMap<>());
        Integer known = read.get(pos.asLong());
        if (known != null) return known;
        Object[] answer = call.apply("light", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ()});
        int light = answer.length > 0 && answer[0] instanceof Integer i ? i : 0xF0;
        read.put(pos.asLong(), light);
        return light;
    }

    /** Light read this call, by level and position (as READS keeps blocks). */
    private static final Map<Level, Map<Long, Integer>> LIGHT_READS = new HashMap<>();

    /**
     * Which of `chunks` (ChunkPos.asLong) Horizon has loaded in `level`, in one call: a
     * position in a chunk it doesn't have reads "" (blockStates).
     */
    static Set<Long> loadedChunks(Level level, long[] chunks) {
        Set<Long> out = new HashSet<>();
        if (chunks.length == 0) return out;
        int[] ask = new int[chunks.length * 3];
        for (int i = 0; i < chunks.length; i++) {
            ask[i * 3] = net.minecraft.world.level.ChunkPos.getX(chunks[i]) << 4;
            ask[i * 3 + 1] = level.getMinBuildHeight();
            ask[i * 3 + 2] = net.minecraft.world.level.ChunkPos.getZ(chunks[i]) << 4;
        }
        Object[] answer = call.apply("blockStates", new Object[] {dimension(level), ask});
        if (answer.length == 0 || !(answer[0] instanceof String[] names)) return out;
        for (int i = 0; i < chunks.length && i < names.length; i++) if (!names[i].isEmpty()) out.add(chunks[i]);
        return out;
    }

    /** The mods' entities of `dimension`, as a save keeps them (Entities120.save). */
    public static Object[] saveEntities(String dimension) {
        begin();
        try {
            return new Object[] {Entities120.save(level(dimension))};
        } finally {
            finish();
        }
    }

    /** Entities a save kept, back into `dimension`; how many came. */
    public static Object[] loadEntities(String dimension, String[] saved) {
        begin();
        try {
            return new Object[] {Entities120.load(level(dimension), saved)};
        } finally {
            finish();
        }
    }

    /** The world closed: the mods' entities go with it. */
    public static Object[] clearEntities() {
        Entities120.clear();
        return new Object[0];
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

    /**
     * The position whose old block is being told it goes (setBlock's onRemove): its block
     * entity is still there for it, whatever block the position holds now, as a chunk keeps it
     * until the block's own onRemove takes it away.
     */
    private static long removing = Long.MIN_VALUE;

    /** Level.setBlock of a mirror level: kept for this call and sent to Horizon. */
    public static boolean setBlock(Level level, BlockPos pos, BlockState state, int flags) {
        if (level.isOutsideBuildHeight(pos)) return false;
        BlockState before = blockState(level, pos);
        if (before == state) return false;
        long key = pos.asLong();
        WRITES.computeIfAbsent(level, l -> new HashMap<>()).put(key, state);
        event.accept("setBlock", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ(), BlockStateParser.serialize(state), flags});
        // The old block hears it is going, as vanilla's LevelChunk.setBlockState tells it
        // (BlockState.onRemove, after the new state is in): a mod's container block drops what
        // it holds there (a cooking pot's ingredients) and its block entity goes. Only a mod's
        // block: Horizon runs a vanilla block's removal itself, and a chest told twice would
        // drop its contents twice.
        if (!before.isAir() && !"minecraft".equals(BuiltInRegistries.BLOCK.getKey(before.getBlock()).getNamespace())) {
            removing = key;
            try {
                before.onRemove(level, pos, state, (flags & 64) != 0);
            } catch (RuntimeException | LinkageError e) {
                log("the block at " + pos.toShortString() + " (" + before + ") failed as it went: " + e);
            } finally {
                removing = Long.MIN_VALUE;
            }
        }
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
            double hx, double hy, double hz, int hand, String item, int count, int damage, boolean creative,
            double px, double py, double pz, double yaw, double pitch) {
        begin();
        ServerLevel level = level(dimension);
        ServerPlayer player = player(uuid, name, level);
        stand(player, px, py, pz, yaw, pitch, hx, hy, hz);
        mode(player, creative);
        Events120.around(level, new BlockPos(x, y, z));
        InteractionHand h = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
        ItemStack stack = stack(item, count, damage);
        player.setItemInHand(h, stack);
        BlockHitResult hit = new BlockHitResult(new Vec3(hx, hy, hz), Direction.from3DDataValue(face), new BlockPos(x, y, z), false);
        var menuBefore = player.containerMenu;
        Screens120.forgetTitle(player);
        InteractionResult result = player.gameMode.useItemOn(player, level, stack, h, hit);
        ItemStack after = player.getItemInHand(h);
        finish();
        int code = !result.consumesAction() ? 0 : result.shouldSwing() ? 2 : 1;
        // A menu the click opened (a cooking pot, a chest): its type and title, for Horizon to
        // show its screen. Empty when none did.
        Object clicked;
        try {
            clicked = level.getBlockEntity(hit.getBlockPos());
        } catch (RuntimeException e) {
            clicked = null;
        }
        String[] menu = opened(player, menuBefore, clicked);
        return new Object[] {code, BuiltInRegistries.ITEM.getKey(after.getItem()).toString(), after.getCount(), after.getDamageValue(),
                menu == null ? "" : menu[0], menu == null ? "" : menu[1]};
    }

    /**
     * What a call opened for `player`: a menu ({kind, title JSON}), or else a screen the
     * client half opened itself ({"screen:" + its class, ""}), or null.
     */
    private static String[] opened(ServerPlayer player, net.minecraft.world.inventory.AbstractContainerMenu before, Object clicked) {
        String[] menu = Screens120.openedSince(player, before, clicked);
        if (menu != null) return menu;
        String screen = Screens120.clientOpenedSince(player);
        return screen == null ? null : new String[] {"screen:" + screen, ""};
    }

    private static volatile int keyProbeClicks;
    private static boolean keyProbeRegistered;

    /**
     * A key binding and a client-tick listener, for the tests, through Fabric's own API as a
     * mod registers them: `key.horizon.probe` on K, and a listener counting its clicks
     * (consumeClick) in each client tick. Gives the clicks counted so far.
     */
    /**
     * Horizon updated the neighbours of these positions (x, y, z each), and a mod's block is
     * among them: each mod's block beside one hears it as vanilla's neighbour updater tells it,
     * its shape updated for the side that changed (BlockState.updateShape, the block replaced or
     * broken when its shape says so) and then its neighborChanged. LibMultipart passes it on to
     * the parts in a container, so a cable joins the cable put beside it. How many heard.
     */
    public static Object[] neighbours(String dimension, int[] positions) {
        begin();
        try {
            ServerLevel level = level(dimension);
            int heard = 0;
            for (int i = 0; i + 2 < positions.length; i += 3) {
                BlockPos source = new BlockPos(positions[i], positions[i + 1], positions[i + 2]);
                Events120.around(level, source);
                BlockState from = level.getBlockState(source);
                for (net.minecraft.core.Direction side : net.minecraft.core.Direction.values()) {
                    BlockPos at = source.relative(side);
                    BlockState state = level.getBlockState(at);
                    if (state.isAir() || "minecraft".equals(BuiltInRegistries.BLOCK.getKey(state.getBlock()).getNamespace())) continue;
                    try {
                        BlockState shaped = state.updateShape(side.getOpposite(), from, level, at, source);
                        if (shaped != state) {
                            net.minecraft.world.level.block.Block.updateOrDestroy(state, shaped, level, at, 3);
                        }
                        level.getBlockState(at).neighborChanged(level, at, from.getBlock(), source, false);
                        heard++;
                    } catch (RuntimeException | LinkageError e) {
                        log("the block at " + at.toShortString() + " (" + state + ") failed hearing its neighbour: " + e);
                    }
                }
            }
            return new Object[] {heard};
        } finally {
            finish();
        }
    }

    /** The mods' block states drawn by code (Models120), baking the client's models first. */
    public static Object[] drawnByCode() {
        setUp();
        return new Object[] {Models120.drawnByCode()};
    }

    /** The quads of the block at a position, as its own model draws it there (Models120.render). */
    public static Object[] renderBlock(String dimension, int x, int y, int z) {
        begin();
        try {
            return Models120.render(level(dimension), new BlockPos(x, y, z));
        } finally {
            finish();
        }
    }

    /**
     * A mod's entity made at (x, y, z), as `/summon` makes one (SummonCommand.createEntity):
     * from `nbt` (SNBT, "" for none) when given, else made new and finalized as a command's
     * spawn. Its id, or -1 when there is no such type or it can't be made here.
     */
    public static Object[] summon(String dimension, String type, double x, double y, double z, String nbt) {
        begin();
        try {
            ServerLevel level = level(dimension);
            var entityType = net.minecraft.world.entity.EntityType.byString(type).orElse(null);
            if (entityType == null) return new Object[] {-1};
            net.minecraft.world.entity.Entity entity;
            if (nbt.isEmpty()) {
                entity = entityType.create(level);
            } else {
                var tag = net.minecraft.nbt.TagParser.parseTag(nbt);
                tag.putString("id", type);
                entity = net.minecraft.world.entity.EntityType.loadEntityRecursive(tag, level, e -> {
                    e.moveTo(x, y, z, e.getYRot(), e.getXRot());
                    return e;
                });
            }
            if (entity == null) return new Object[] {-1};
            entity.moveTo(x, y, z, entity.getYRot(), entity.getXRot());
            if (nbt.isEmpty() && entity instanceof net.minecraft.world.entity.Mob mob) {
                mob.finalizeSpawn(level, level.getCurrentDifficultyAt(entity.blockPosition()),
                        net.minecraft.world.entity.MobSpawnType.COMMAND, null, null);
            }
            if (!level.addFreshEntity(entity)) return new Object[] {-1};
            for (var passenger : entity.getPassengers()) level.addFreshEntity(passenger);
            return new Object[] {entity.getId()};
        } catch (Exception | LinkageError e) {
            log("a " + type + " couldn't be made: " + e);
            return new Object[] {-1};
        } finally {
            finish();
        }
    }

    /** The mods' entity types (not vanilla's), for Horizon's commands to know. */
    public static Object[] modEntityTypes() {
        return new Object[] {net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.keySet().stream()
                .filter(id -> !"minecraft".equals(id.getNamespace()))
                .map(Object::toString).sorted().toArray(String[]::new)};
    }

    /**
     * A tick of the mods' entities in `dimension` (Entities120.tick), and where they are after:
     * {numbers, types} as Entities120.snapshot gives them.
     */
    public static Object[] tickEntities(String dimension, String[] uuids, String[] names, double[] at) {
        begin();
        try {
            ServerLevel level = level(dimension);
            // Horizon's players in this level, where they stand: what a mob's checkDespawn and
            // its goals (a golem looking for a player, a cow following wheat) ask the level for.
            List<ServerPlayer> here = new ArrayList<>();
            for (int i = 0; i < uuids.length && i * 3 + 2 < at.length; i++) {
                ServerPlayer player = player(uuids[i], names[i], level);
                player.setPos(at[i * 3], at[i * 3 + 1], at[i * 3 + 2]);
                here.add(player);
            }
            synchronized (PRESENT) {
                PRESENT.put(level, here);
            }
            Entities120.tick(level);
            return Entities120.snapshot(level);
        } finally {
            finish();
        }
    }

    /**
     * NaturalSpawner.spawnMobsForChunkGeneration's attempt at column (x, z) of a chunk being
     * generated, for one of the mods' types Horizon picked from the biome's creatures: the
     * type's surface there, its placement and rules, room for its box, then the mob's own
     * checks and finalizeSpawn as a chunk generation spawn. {true} when one spawned.
     */
    public static Object[] spawnGenerated(String dimension, String type, int x, int z, int chunkX, int chunkZ, double yaw) {
        begin();
        try {
            ServerLevel level = level(dimension);
            var entityType = net.minecraft.world.entity.EntityType.byString(type).orElse(null);
            if (entityType == null || !entityType.canSummon()) return new Object[] {false};
            BlockPos pos = dev.horizonlab.platform120.mixin.NaturalSpawnerAccess.horizon$topNonColliding(level, entityType, x, z);
            if (!net.minecraft.world.level.NaturalSpawner.isSpawnPositionOk(
                    net.minecraft.world.entity.SpawnPlacements.getPlacementType(entityType), level, pos, entityType)) {
                if (SPAWN_TRACE) log("no " + type + " at " + pos.toShortString() + ": not a place for it (on " + level.getBlockState(pos.below()) + ")");
                return new Object[] {false};
            }
            float width = entityType.getWidth();
            int xo = chunkX << 4;
            int zo = chunkZ << 4;
            double fx = net.minecraft.util.Mth.clamp(x, xo + width, xo + 16.0 - width);
            double fz = net.minecraft.util.Mth.clamp(z, zo + width, zo + 16.0 - width);
            var reason = net.minecraft.world.entity.MobSpawnType.CHUNK_GENERATION;
            if (!level.noCollision(entityType.getAABB(fx, pos.getY(), fz))
                    || !net.minecraft.world.entity.SpawnPlacements.checkSpawnRules(entityType, level, reason,
                            BlockPos.containing(fx, pos.getY(), fz), level.getRandom())) {
                if (SPAWN_TRACE) log("no " + type + " at " + pos.toShortString() + ": its rules (light "
                        + level.getRawBrightness(pos, 0) + ", on " + level.getBlockState(pos.below()) + ")");
                return new Object[] {false};
            }
            if (!(entityType.create(level) instanceof net.minecraft.world.entity.Mob mob)) return new Object[] {false};
            mob.moveTo(fx, pos.getY(), fz, (float) yaw, 0.0F);
            if (!mob.checkSpawnRules(level, reason) || !mob.checkSpawnObstruction(level)) return new Object[] {false};
            mob.finalizeSpawn(level, level.getCurrentDifficultyAt(mob.blockPosition()), reason, null, null);
            level.addFreshEntityWithPassengers(mob);
            if (SPAWN_TRACE) log("a " + type + " spawned with its chunk at " + mob.blockPosition().toShortString());
            return new Object[] {true};
        } catch (RuntimeException | LinkageError e) {
            log("a " + type + " didn't spawn with its chunk: " + e);
            return new Object[] {false};
        } finally {
            finish();
        }
    }

    /**
     * A sound the mods' code played in a level (ServerLevelMixin): to Horizon, which plays it
     * for its players near, by the sound's name (the mod's own sounds are in its content).
     * Every player hears it, the one whose action caused it too: vanilla leaves that one out
     * because their own client played it already, and here no client of theirs did.
     */
    public static void sound(Level level, double x, double y, double z,
            net.minecraft.core.Holder<net.minecraft.sounds.SoundEvent> sound,
            net.minecraft.sounds.SoundSource source, float volume, float pitch) {
        if (event == null) return;
        String id = sound.unwrapKey().map(k -> k.location().toString())
                .orElseGet(() -> sound.value().getLocation().toString());
        event.accept("sound", new Object[] {dimension(level), x, y, z, id, source.ordinal(), (double) volume, (double) pitch});
    }

    /**
     * Particles the mods' code sent its players (ServerLevelMixin): to Horizon, as vanilla's
     * own string of them (`minecraft:block minecraft:stone`, `minecraft:dust 1.0 0.0 0.0 1.0`),
     * with where, how many, the spread and the speed, as the packet carries them.
     */
    public static void particles(Level level, net.minecraft.core.particles.ParticleOptions options,
            double x, double y, double z, int count, double dx, double dy, double dz, double speed) {
        if (event == null) return;
        event.accept("particles", new Object[] {dimension(level), options.writeToString(), x, y, z, count, dx, dy, dz, speed});
    }

    private static final net.minecraft.util.RandomSource ANIMATE = net.minecraft.util.RandomSource.create();
    private static final java.util.Set<String> ANIMATE_FAILED = new java.util.HashSet<>();

    /**
     * Block.animateTick of the blocks at `positions` (x, y, z each), as a client calls it on the
     * blocks around its player: the particles and sounds the blocks' client code makes go to
     * Horizon (LevelMixin). Horizon picks which, as often as ClientLevel.animateTick would.
     */
    public static Object[] animateTicks(String dimension, int[] positions) {
        begin();
        try {
            ServerLevel level = level(dimension);
            for (int i = 0; i + 2 < positions.length; i += 3) {
                BlockPos pos = new BlockPos(positions[i], positions[i + 1], positions[i + 2]);
                BlockState state = blockState(level, pos);
                try {
                    state.getBlock().animateTick(state, level, pos, ANIMATE);
                } catch (RuntimeException | LinkageError e) {
                    String block = BuiltInRegistries.BLOCK.getKey(state.getBlock()).toString();
                    if (ANIMATE_FAILED.add(block)) log("a " + block + "'s animateTick failed: " + e);
                }
            }
            return new Object[0];
        } finally {
            finish();
        }
    }

    /**
     * The mods' blocks that have a block entity (EntityBlock.newBlockEntity of their default
     * state), and its type: {blocks, types}. Horizon makes one for such a block however it's
     * placed (a command, a structure), as a chunk does with the block.
     */
    public static Object[] modBlockEntityBlocks() {
        List<String> blocks = new ArrayList<>();
        List<String> types = new ArrayList<>();
        for (var block : BuiltInRegistries.BLOCK) {
            var id = BuiltInRegistries.BLOCK.getKey(block);
            if ("minecraft".equals(id.getNamespace())) continue;
            if (!(block instanceof net.minecraft.world.level.block.EntityBlock entityBlock)) continue;
            try {
                var made = entityBlock.newBlockEntity(BlockPos.ZERO, block.defaultBlockState());
                if (made == null) continue;
                var type = BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(made.getType());
                if (type == null) continue;
                blocks.add(id.toString());
                types.add(type.toString());
            } catch (RuntimeException | LinkageError e) {
                // One that can't be made without a level: Horizon leaves it to the mod's code.
            }
        }
        return new Object[] {blocks.toArray(new String[0]), types.toArray(new String[0])};
    }

    /** {ids, names} of the mods with a settings screen (Config120). */
    public static Object[] configMods() {
        return Config120.mods();
    }

    /**
     * A mod's settings screen, opened for a player of Horizon's (the client half standing in
     * for them): {kind} of what opened (`screen:<class>`), or {""}.
     */
    public static Object[] openConfig(String uuid, String name, String mod) {
        begin();
        try {
            ServerPlayer player = player(uuid, name, level("minecraft:overworld"));
            Screens120.clientPlayer(player);
            if (!Config120.open(mod)) return new Object[] {""};
            String[] menu = opened(player, player.containerMenu, null);
            return new Object[] {menu == null ? "" : menu[0]};
        } finally {
            finish();
        }
    }

    /**
     * What the mods draw on a player's HUD (Hud120): {frame}, the operations as JSON. `state`
     * is the player as Horizon has them: `dimension`, `health`, `max_health`, `absorption`,
     * `food`, `saturation`, `exhaustion`, `air`, `xp_level`, `xp_progress`, `hurtable`.
     */
    public static Object[] hud(String uuid, String name, String state, int width, int height) {
        begin();
        try {
            String dimension = com.google.gson.JsonParser.parseString(state).getAsJsonObject()
                    .get("dimension").getAsString();
            ServerPlayer player = player(uuid, name, level(dimension));
            return new Object[] {Hud120.frame(player, state, width, height)};
        } finally {
            finish();
        }
    }

    /**
     * The mods' effects on a player acting this tick (phase 5k): `MobEffect.applyEffectTick`
     * where `isDurationEffectTick` says, as `MobEffectInstance.tick` runs it, on the runtime's
     * player with Horizon's numbers. Farmer's Delight's Nourishment takes back exhaustion, its
     * Comfort heals. `state` is the HUD's (Hud120: health, food...) with `effects`, all the
     * player's (`{id, duration, amplifier}`: Comfort asks for Regeneration), and `tick`, the
     * mods' ones due (`{id, ticks, amplifier}`, ticks as `tickServer` hands them). {state}: the
     * player's health, absorption, food, saturation, exhaustion and air after them.
     */
    public static Object[] effectTicks(String uuid, String name, String state) {
        begin();
        try {
            var s = com.google.gson.JsonParser.parseString(state).getAsJsonObject();
            ServerPlayer player = player(uuid, name, level(s.get("dimension").getAsString()));
            Hud120.stats(player, s);
            var active = new java.util.HashMap<net.minecraft.world.effect.MobEffect, net.minecraft.world.effect.MobEffectInstance>();
            for (var e : s.getAsJsonArray("effects")) {
                var o = e.getAsJsonObject();
                var effect = BuiltInRegistries.MOB_EFFECT.get(new ResourceLocation(o.get("id").getAsString()));
                if (effect == null) continue;
                active.put(effect, new net.minecraft.world.effect.MobEffectInstance(effect,
                        o.get("duration").getAsInt(), o.get("amplifier").getAsInt()));
            }
            ((dev.horizonlab.platform120.mixin.LivingEntityAccess) (Object) player).horizon$setActiveEffects(active);
            for (var e : s.getAsJsonArray("tick")) {
                var o = e.getAsJsonObject();
                var effect = BuiltInRegistries.MOB_EFFECT.get(new ResourceLocation(o.get("id").getAsString()));
                if (effect == null) continue;
                int amplifier = o.get("amplifier").getAsInt();
                try {
                    if (effect.isDurationEffectTick(o.get("ticks").getAsInt(), amplifier)) {
                        effect.applyEffectTick(player, amplifier);
                    }
                } catch (Throwable t) {
                    log("effect " + o.get("id").getAsString() + " failed: " + t);
                }
            }
            var food = player.getFoodData();
            var after = new com.google.gson.JsonObject();
            after.addProperty("health", player.getHealth());
            after.addProperty("absorption", player.getAbsorptionAmount());
            after.addProperty("food", food.getFoodLevel());
            after.addProperty("saturation", food.getSaturationLevel());
            after.addProperty("exhaustion", ((dev.horizonlab.platform120.mixin.FoodDataAccess) food).horizon$exhaustion());
            after.addProperty("air", player.getAirSupply());
            return new Object[] {after.toString()};
        } finally {
            finish();
        }
    }

    /**
     * What the mods add to a stack's tooltip (phase 5n), as two lists of JSON components:
     * {hover, after}. `hover` is a mod item's own lines (`Item.appendHoverText`, which
     * getTooltipLines puts under the name), `after` what the tooltip mods add to any stack
     * (Fabric's ItemTooltipCallback, fired at the end of getTooltipLines with the whole list:
     * the lines it appended). Horizon draws the rest of the tooltip itself. `snbt` is the
     * stack as 1.20.1 saves it; `advanced` the F3+H tooltips.
     */
    public static Object[] tooltip(String snbt, int advanced) {
        begin();
        try {
            ItemStack stack = ItemStack.of(net.minecraft.nbt.TagParser.parseTag(snbt));
            var flag = advanced != 0 ? net.minecraft.world.item.TooltipFlag.ADVANCED
                    : net.minecraft.world.item.TooltipFlag.NORMAL;
            var hover = new java.util.ArrayList<net.minecraft.network.chat.Component>();
            ResourceLocation id = BuiltInRegistries.ITEM.getKey(stack.getItem());
            if (!stack.isEmpty() && !id.getNamespace().equals("minecraft")) {
                try {
                    stack.getItem().appendHoverText(stack, null, hover, flag);
                } catch (Throwable t) {
                    log(id + "'s tooltip failed: " + t);
                }
            }
            var all = new java.util.ArrayList<net.minecraft.network.chat.Component>();
            all.add(stack.getHoverName());
            all.addAll(hover);
            int before = all.size();
            try {
                Class<?> callback = Class.forName("net.fabricmc.fabric.api.client.item.v1.ItemTooltipCallback");
                Object eventObject = callback.getField("EVENT").get(null);
                Object invoker = eventObject.getClass().getMethod("invoker").invoke(eventObject);
                var getTooltip = callback.getMethod("getTooltip", ItemStack.class,
                        net.minecraft.world.item.TooltipFlag.class, java.util.List.class);
                getTooltip.invoke(invoker, stack, flag, all);
            } catch (ClassNotFoundException e) {
                // No Fabric client item API: no tooltip mods.
            } catch (Throwable t) {
                Throwable cause = t instanceof java.lang.reflect.InvocationTargetException i && i.getCause() != null ? i.getCause() : t;
                log("a tooltip mod failed on " + id + ": " + cause);
            }
            var hoverJson = new com.google.gson.JsonArray();
            for (var line : hover) hoverJson.add(net.minecraft.network.chat.Component.Serializer.toJsonTree(line));
            var afterJson = new com.google.gson.JsonArray();
            for (int i = before; i < all.size(); i++) {
                afterJson.add(net.minecraft.network.chat.Component.Serializer.toJsonTree(all.get(i)));
            }
            return new Object[] {hoverJson.toString(), afterJson.toString()};
        } catch (com.mojang.brigadier.exceptions.CommandSyntaxException e) {
            return new Object[] {"[]", "[]"};
        } finally {
            finish();
        }
    }

    /** Whether a fluid is a mod's (its own code runs it: phase 6e). */
    static boolean ourFluid(net.minecraft.world.level.material.Fluid fluid) {
        ResourceLocation id = BuiltInRegistries.FLUID.getKey(fluid);
        return id != null && !id.getNamespace().equals("minecraft");
    }

    /**
     * The mods' fluid blocks (their LiquidBlocks: Tech Reborn's oil): {ids}. Horizon leaves
     * them to the mods' code (`LevelAccess::foreign_fluid`): its water and lava don't run them.
     */
    public static Object[] modFluidBlocks() {
        var ids = new java.util.ArrayList<String>();
        for (var block : BuiltInRegistries.BLOCK) {
            if (block instanceof net.minecraft.world.level.block.LiquidBlock liquid
                    && ourFluid(liquid.getFluidState(liquid.defaultBlockState()).getType())) {
                ids.add(BuiltInRegistries.BLOCK.getKey(block).toString());
            }
        }
        return new Object[] {ids.toArray(new String[0])};
    }

    /**
     * How the mods' fluids are drawn (phase 6e): {json}, by the fluid block's id, `{"still",
     * "flow", "tint", "translucent"}` as the 26.3 bridge's HorizonFluids gives them, the
     * sprites as texture ids (`techreborn:block/fluids/oil_still`). Read where they are kept,
     * not from the block atlas (this client stitches none): Fabric's SimpleFluidRenderHandler's
     * fields, a fluid's own settings (Reborn Core's `getFluidSettings`), else the handler's
     * sprites. A fluid none of these say is left out, drawn as its template.
     */
    public static Object[] fluidLooks() {
        var out = new com.google.gson.JsonObject();
        Object registry = null;
        java.lang.reflect.Method get = null;
        try {
            Class<?> r = Class.forName("net.fabricmc.fabric.api.client.render.fluid.v1.FluidRenderHandlerRegistry");
            registry = r.getField("INSTANCE").get(null);
            get = r.getMethod("get", net.minecraft.world.level.material.Fluid.class);
        } catch (Throwable t) {
            // No Fabric fluid rendering: only the fluids' own settings.
        }
        for (var fluid : BuiltInRegistries.FLUID) {
            if (!ourFluid(fluid) || !(fluid instanceof net.minecraft.world.level.material.FlowingFluid flowing)
                    || fluid != flowing.getSource()) {
                continue;
            }
            var state = fluid.defaultFluidState();
            var block = state.createLegacyBlock();
            if (block.isAir()) continue;
            String still = null, flow = null;
            int tint = -1;
            Object handler = null;
            try {
                if (registry != null) handler = get.invoke(registry, fluid);
            } catch (Throwable t) {
                handler = null;
            }
            if (handler != null) {
                still = idField(handler, "stillTexture");
                flow = idField(handler, "flowingTexture");
                try {
                    Object t = field(handler, "tint");
                    if (t instanceof Integer i) tint = i;
                } catch (Throwable t) {
                    // No tint of its own.
                }
            }
            if (still == null) {
                try {
                    Object settings = fluid.getClass().getMethod("getFluidSettings").invoke(fluid);
                    still = String.valueOf(settings.getClass().getMethod("getStillTexture").invoke(settings));
                    flow = String.valueOf(settings.getClass().getMethod("getFlowingTexture").invoke(settings));
                } catch (Throwable t) {
                    still = null;
                }
            }
            if (still == null && handler != null) {
                try {
                    Object sprites = handler.getClass().getMethod("getFluidSprites",
                            net.minecraft.world.level.BlockAndTintGetter.class, BlockPos.class,
                            net.minecraft.world.level.material.FluidState.class).invoke(handler, null, null, state);
                    var array = (net.minecraft.client.renderer.texture.TextureAtlasSprite[]) sprites;
                    still = array[0].contents().name().toString();
                    flow = array[1].contents().name().toString();
                } catch (Throwable t) {
                    still = null;
                }
            }
            if (still == null || flow == null || still.equals("null")) continue;
            var look = new com.google.gson.JsonObject();
            look.addProperty("still", still);
            look.addProperty("flow", flow);
            look.addProperty("tint", tint);
            look.addProperty("translucent", true);
            out.add(BuiltInRegistries.BLOCK.getKey(block.getBlock()).toString(), look);
        }
        return new Object[] {out.toString()};
    }

    private static Object field(Object on, String name) throws ReflectiveOperationException {
        for (Class<?> c = on.getClass(); c != null; c = c.getSuperclass()) {
            try {
                var f = c.getDeclaredField(name);
                f.setAccessible(true);
                return f.get(on);
            } catch (NoSuchFieldException e) {
                // Up the classes.
            }
        }
        throw new NoSuchFieldException(name);
    }

    private static String idField(Object on, String name) {
        try {
            Object v = field(on, name);
            return v instanceof ResourceLocation id ? id.toString() : null;
        } catch (Throwable t) {
            return null;
        }
    }

    private static FluidTicks120 fluidTicksOf(ServerLevel level) {
        return level.getFluidTicks() instanceof FluidTicks120 t ? t : null;
    }

    /**
     * Horizon changed blocks at or beside `positions` (x, y, z each), a mod's fluid among
     * them: each such fluid's tick is scheduled at its own delay, as `LiquidBlock.onPlace` and
     * `neighborChanged` do. {ticks pending}.
     */
    public static Object[] fluidChanged(String dimension, int[] positions, long gameTime) {
        begin();
        try {
            ServerLevel level = level(dimension);
            worldData.setGameTime(gameTime);
            FluidTicks120 ticks = fluidTicksOf(level);
            if (ticks == null) return new Object[] {0};
            for (int i = 0; i + 2 < positions.length; i += 3) {
                BlockPos pos = new BlockPos(positions[i], positions[i + 1], positions[i + 2]);
                var fluid = level.getFluidState(pos);
                if (fluid.isEmpty() || !ourFluid(fluid.getType())) continue;
                ticks.schedule(pos.asLong(), gameTime + fluid.getType().getTickDelay(level));
            }
            return new Object[] {ticks.count()};
        } finally {
            finish();
        }
    }

    /**
     * The mods' fluid ticks due in `dimension` by Horizon's `gameTime`: each runs
     * `FluidState.tick` (the mod's own spread, delay and drop-off), as ServerLevel.tickFluid
     * does, at most 4096 a call. What they set goes back as any call's writes; the ticks they
     * schedule are kept. {ticks still pending}.
     */
    public static Object[] fluidTicks(String dimension, long gameTime) {
        begin();
        try {
            ServerLevel level = level(dimension);
            worldData.setGameTime(gameTime);
            FluidTicks120 ticks = fluidTicksOf(level);
            if (ticks == null) return new Object[] {0};
            var due = new java.util.ArrayList<Map.Entry<Long, Long>>();
            for (var e : ticks.due.entrySet()) if (e.getValue() <= gameTime) due.add(e);
            due.sort(Map.Entry.comparingByValue());
            int ran = 0;
            for (var e : due) {
                if (ran++ >= 4096) break;
                long at = e.getKey();
                ticks.due.remove(at);
                BlockPos pos = BlockPos.of(at);
                var fluid = level.getFluidState(pos);
                if (fluid.isEmpty() || !ourFluid(fluid.getType())) continue;
                try {
                    fluid.tick(level, pos);
                } catch (Throwable t) {
                    log("fluid " + BuiltInRegistries.FLUID.getKey(fluid.getType()) + " at " + pos + " failed: " + t);
                }
            }
            return new Object[] {ticks.count()};
        } finally {
            finish();
        }
    }

    /** A message the mods sent a player (ServerPlayerMixin): to Horizon's, as JSON. */
    public static void message(ServerPlayer player, net.minecraft.network.chat.Component message, boolean overlay) {
        if (event == null) return;
        event.accept("message", new Object[] {player.getStringUUID(),
                net.minecraft.network.chat.Component.Serializer.toJson(message), overlay ? 1 : 0});
    }

    private static com.mojang.brigadier.CommandDispatcher<net.minecraft.commands.CommandSourceStack> dispatcher() {
        return data == null ? null : data.getCommands().getDispatcher();
    }

    /**
     * Every command the server's dispatcher has, vanilla's and the mods' (Fabric API's command
     * callback registers the mods' as the server's data loads): {names}. Horizon takes the
     * ones that aren't its own.
     */
    public static Object[] commandNames() {
        var dispatcher = dispatcher();
        var names = new java.util.TreeSet<String>();
        if (dispatcher != null) dispatcher.getRoot().getChildren().forEach(n -> names.add(n.getName()));
        return new Object[] {names.toArray(new String[0])};
    }

    /**
     * These commands as a root of the data generator's commands report, the form Horizon's
     * command tree is in: {@code {"type": "root", "children": {...}}}. An argument of a mod's
     * own type is sent as a greedy string: the player's game still parses the line, and the
     * command checks it when it runs. As the 26.3 bridge's HorizonServer.commandTree.
     */
    public static Object[] commandTree(String names) {
        var dispatcher = dispatcher();
        com.google.gson.JsonObject root = new com.google.gson.JsonObject();
        root.addProperty("type", "root");
        com.google.gson.JsonObject children = new com.google.gson.JsonObject();
        if (dispatcher != null) {
            for (String name : names.split(" ")) {
                var node = dispatcher.getRoot().getChild(name);
                if (node == null) continue;
                try {
                    com.google.gson.JsonObject json =
                            net.minecraft.commands.synchronization.ArgumentUtils.serializeNodeToJson(dispatcher, node);
                    vanillaParsers(json);
                    children.add(name, json);
                } catch (Throwable t) {
                    log("command /" + name + " can't be sent to the players: " + t);
                    children.add(name, anyArguments());
                }
            }
        }
        root.add("children", children);
        return new Object[] {root.toString()};
    }

    /** A literal that runs alone or with anything after it (a greedy string). */
    private static com.google.gson.JsonObject anyArguments() {
        var literal = new com.google.gson.JsonObject();
        literal.addProperty("type", "literal");
        literal.addProperty("executable", true);
        var argument = new com.google.gson.JsonObject();
        argument.addProperty("type", "argument");
        argument.addProperty("parser", "brigadier:string");
        var properties = new com.google.gson.JsonObject();
        properties.addProperty("type", "greedy");
        argument.add("properties", properties);
        argument.addProperty("executable", true);
        var children = new com.google.gson.JsonObject();
        children.add("arguments", argument);
        literal.add("children", children);
        return literal;
    }

    private static void vanillaParsers(com.google.gson.JsonObject node) {
        if (node.has("parser")) {
            String parser = node.get("parser").getAsString();
            if (!parser.startsWith("minecraft:") && !parser.startsWith("brigadier:")) {
                node.addProperty("parser", "brigadier:string");
                com.google.gson.JsonObject properties = new com.google.gson.JsonObject();
                properties.addProperty("type", "greedy");
                node.add("properties", properties);
            }
        }
        if (node.has("children")) {
            for (var entry : node.getAsJsonObject("children").entrySet()) {
                vanillaParsers(entry.getValue().getAsJsonObject());
            }
        }
    }

    /**
     * A command line (no slash) run as a player of Horizon's at their permission level, as
     * vanilla's server runs a player's command: their command source, the dispatcher's parse
     * and its errors. What it says goes to the player (ServerPlayerMixin), what it does to the
     * world to Horizon as any call's changes do. {1}.
     */
    public static Object[] command(String uuid, String name, String dimension, int permission, String line) {
        begin();
        try {
            ServerPlayer player = player(uuid, name, level(dimension));
            var source = new net.minecraft.commands.CommandSourceStack(player, player.position(),
                    player.getRotationVector(), (ServerLevel) player.level(), permission,
                    player.getName().getString(), player.getDisplayName(), server, player);
            try {
                data.getCommands().performPrefixedCommand(source, line);
            } catch (Throwable t) {
                message(player, net.minecraft.network.chat.Component.literal(
                        "An error occurred while running /" + line + ": " + t), false);
                log("/" + line + " failed: " + t);
            }
            return new Object[] {1};
        } finally {
            finish();
        }
    }

    /** HORIZON_120_TRACE: why each of the mods' spawns did or didn't happen. */
    private static final boolean SPAWN_TRACE = System.getenv("HORIZON_120_TRACE") != null;

    /** What the mods' block entities at `positions` look like (Render120.renderBlockEntities). */
    public static Object[] renderBlockEntities(String dimension, int[] positions) {
        begin();
        try {
            return new Object[] {Render120.renderBlockEntities(level(dimension), positions)};
        } finally {
            finish();
        }
    }

    /** Each level's players this tick (tickEntities), as ServerLevel.players gives them. */
    private static final Map<Level, List<ServerPlayer>> PRESENT = new HashMap<>();

    /** ServerLevel.players of a mirror level (ServerLevelMixin). */
    public static List<ServerPlayer> present(Level level) {
        synchronized (PRESENT) {
            return PRESENT.getOrDefault(level, List.of());
        }
    }

    /**
     * NaturalSpawner.spawnCategoryForPosition, from where Horizon's spawner has picked one of
     * the mods' types in a biome's list, a spot and a yaw: the type's own checks
     * (isValidSpawnPositionForType: its placement, SpawnPlacements' rules, room for its box),
     * then the mob's (isValidPositionForMob: too far to stay, its checkSpawnRules and
     * checkSpawnObstruction), finalizeSpawn as a natural spawn, and in. `distance` is the
     * squared distance to the nearest player. {width, height, max cluster size} of what
     * spawned, or {} for nothing.
     */
    public static Object[] spawnNatural(String dimension, String type, double x, double y, double z, double yaw, double distance) {
        begin();
        try {
            ServerLevel level = level(dimension);
            var entityType = net.minecraft.world.entity.EntityType.byString(type).orElse(null);
            if (entityType == null) return new Object[0];
            var category = entityType.getCategory();
            BlockPos pos = BlockPos.containing(x, y, z);
            int despawn = category.getDespawnDistance();
            var natural = net.minecraft.world.entity.MobSpawnType.NATURAL;
            if (category == net.minecraft.world.entity.MobCategory.MISC
                    || !entityType.canSpawnFarFromPlayer() && distance > (double) despawn * despawn
                    || !entityType.canSummon()
                    || !net.minecraft.world.level.NaturalSpawner.isSpawnPositionOk(
                            net.minecraft.world.entity.SpawnPlacements.getPlacementType(entityType), level, pos, entityType)
                    || !net.minecraft.world.entity.SpawnPlacements.checkSpawnRules(entityType, level, natural, pos, level.getRandom())
                    || !level.noCollision(entityType.getAABB(x, y, z))) {
                return new Object[0];
            }
            if (!(entityType.create(level) instanceof net.minecraft.world.entity.Mob mob)) return new Object[0];
            mob.moveTo(x, y, z, (float) yaw, 0.0F);
            if (distance > (double) despawn * despawn && mob.removeWhenFarAway(distance)
                    || !mob.checkSpawnRules(level, natural)
                    || !mob.checkSpawnObstruction(level)) {
                return new Object[0];
            }
            mob.finalizeSpawn(level, level.getCurrentDifficultyAt(mob.blockPosition()), natural, null, null);
            level.addFreshEntityWithPassengers(mob);
            return new Object[] {(double) mob.getBbWidth(), (double) mob.getBbHeight(), mob.getMaxSpawnClusterSize()};
        } catch (RuntimeException | LinkageError e) {
            log("a " + type + " didn't spawn: " + e);
            return new Object[0];
        } finally {
            finish();
        }
    }

    /**
     * A player of Horizon's attacks (`attack`) or uses one of the mods' entities, with `held`
     * (1.20.1 item NBT, "" for nothing) in that hand, standing and looking as {@link #stand}
     * says, `ticks` since their last blow (how charged it is).
     *
     * <ul>
     *   <li>An attack is ServerPlayer.attack, whose head is Fabric's AttackEntityCallback: the
     *       damage of the held item, its enchantments, knockback, a sweep over the mods' mobs
     *       around, the item's wear. A mob it kills drops its loot as any call's drops go.
     *   <li>A use is what ServerGamePacketListenerImpl.handleInteract does with an INTERACT_AT:
     *       Fabric's UseEntityCallback first, then the entity's interactAt at `(lx, ly, lz)`
     *       (relative to it), then Player.interactOn (the mob's own interact: milking, shearing,
     *       feeding, a spawn egg on its kind).
     * </ul>
     *
     * Gives as useItem: 0 passed, 1 taken, 2 taken with a swing; the held item after; a menu it
     * opened (kind, title).
     */
    public static Object[] entityAction(String uuid, String name, String dimension, int id, boolean attack, int hand,
            String held, boolean creative, boolean sneaking, int ticks, double lx, double ly, double lz,
            double px, double py, double pz, double yaw, double pitch) {
        begin();
        try {
            ServerLevel level = level(dimension);
            ServerPlayer player = player(uuid, name, level);
            stand(player, px, py, pz, yaw, pitch, px, py + 1.62, pz);
            mode(player, creative);
            player.setShiftKeyDown(sneaking);
            InteractionHand which = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
            player.setItemInHand(which, heldStack(held));
            net.minecraft.world.entity.Entity target = Entities120.of(level).get(id);
            if (target == null || target.isRemoved()) return new Object[] {0, held, "", "", new String[0]};
            var menuBefore = player.containerMenu;
            Screens120.forgetTitle(player);
            Screens120.clientPlayer(player);
            var inventory = player.getInventory();
            int heldSlot = which == InteractionHand.OFF_HAND ? 40 : inventory.selected;
            ItemStack[] before = new ItemStack[inventory.getContainerSize()];
            for (int slot = 0; slot < before.length; slot++) before[slot] = inventory.getItem(slot).copy();
            int code;
            if (attack) {
                ((dev.horizonlab.platform120.mixin.LivingEntityAccess) player).horizon$setAttackStrengthTicker(ticks);
                // The held item's damage and speed: LivingEntity.tick puts its modifiers on as
                // it notices the hand change, and this player never ticks.
                var modifiers = player.getMainHandItem().getAttributeModifiers(net.minecraft.world.entity.EquipmentSlot.MAINHAND);
                player.getAttributes().addTransientAttributeModifiers(modifiers);
                try {
                    player.attack(target);
                } finally {
                    player.getAttributes().removeAttributeModifiers(modifiers);
                }
                code = 1;
            } else {
                Vec3 at = new Vec3(lx, ly, lz);
                InteractionResult result = net.fabricmc.fabric.api.event.player.UseEntityCallback.EVENT.invoker()
                        .interact(player, level, which, target,
                                new net.minecraft.world.phys.EntityHitResult(target, at.add(target.position())));
                if (result == InteractionResult.PASS) {
                    result = target.interactAt(player, at, which);
                    if (!result.consumesAction()) result = player.interactOn(target, which);
                }
                code = !result.consumesAction() ? 0 : result.shouldSwing() ? 2 : 1;
            }
            ItemStack after = player.getItemInHand(which);
            String[] menu = opened(player, menuBefore, null);
            return new Object[] {code, after.isEmpty() ? "" : after.save(new net.minecraft.nbt.CompoundTag()).toString(),
                    menu == null ? "" : menu[0], menu == null ? "" : menu[1], given(inventory, before, heldSlot)};
        } catch (RuntimeException | LinkageError e) {
            log("a player's " + (attack ? "attack on" : "use of") + " entity " + id + " failed: " + e);
            for (StackTraceElement at : e.getStackTrace()) log("    at " + at);
            return new Object[] {0, held, "", "", new String[0]};
        } finally {
            player(uuid, name, level(dimension)).setShiftKeyDown(false);
            finish();
        }
    }

    /**
     * What a call put into the player's inventory beside the held stack, as 1.20.1 item NBT:
     * a mob's milk in creative (ItemUtils.createFilledResult keeps the empty bucket there and
     * adds the full one), a mod's gift. Horizon gives it to its player, whose inventory this
     * one isn't; the copy here goes back as it was.
     */
    private static String[] given(net.minecraft.world.entity.player.Inventory inventory, ItemStack[] before, int heldSlot) {
        List<String> out = new ArrayList<>();
        for (int slot = 0; slot < before.length; slot++) {
            if (slot == heldSlot) continue;
            ItemStack now = inventory.getItem(slot);
            ItemStack was = before[slot];
            int added = ItemStack.isSameItemSameTags(now, was) ? now.getCount() - was.getCount()
                    : now.isEmpty() ? 0 : now.getCount();
            if (added > 0) out.add(now.copyWithCount(added).save(new net.minecraft.nbt.CompoundTag()).toString());
            inventory.setItem(slot, was);
        }
        return out.toArray(new String[0]);
    }

    /** What the mods' entities in `dimension` look like this tick (Render120.render), as JSON. */
    public static Object[] renderEntities(String dimension) {
        begin();
        try {
            return new Object[] {Render120.render(level(dimension))};
        } finally {
            finish();
        }
    }

    /** The mods' placed features, which Horizon's generator asks this side to place. */
    public static Object[] modFeatures() {
        setUp();
        return new Object[] {Worldgen120.modFeatures()};
    }

    /**
     * A mod's placed feature in the chunks Horizon is decorating (Worldgen120.place). The
     * level's blocks, heights and biomes are the region's for this call.
     */
    public static Object[] placeFeature(String dimension, String feature, long worldSeed, int chunkX, int chunkZ,
            int index, int step, int x, int y, int z) {
        begin();
        try {
            return new Object[] {Worldgen120.place(level(dimension), feature, worldSeed, chunkX, chunkZ, index, step,
                    new BlockPos(x, y, z))};
        } finally {
            finish();
        }
    }

    /** The pools the mods add to loot tables in code (Loot120.added): {json}. */
    public static Object[] lootAdded() {
        setUp();
        return new Object[] {Loot120.added()};
    }

    /** The structure types of the mods' own structures (Worldgen120.modStructureTypes). */
    public static Object[] modStructureTypes() {
        setUp();
        return new Object[] {Worldgen120.modStructureTypes()};
    }

    /**
     * A mod's structure's start in a chunk (Worldgen120.structureStart): heights asked of Horizon
     * through the level for this call. {handle, position, piece boxes}.
     */
    public static Object[] structureStart(String dimension, String id, long seed, int chunkX, int chunkZ) {
        begin();
        try {
            return Worldgen120.structureStart(level(dimension), id, seed, chunkX, chunkZ);
        } finally {
            finish();
        }
    }

    /** A kept start placed in the chunk Horizon is decorating (Worldgen120.structurePlace). */
    public static Object[] structurePlace(String dimension, long handle, long seed, int chunkX, int chunkZ,
            int index, int step) {
        begin();
        try {
            return new Object[] {Worldgen120.structurePlace(level(dimension), handle, seed, chunkX, chunkZ, index, step)};
        } finally {
            finish();
        }
    }

    /** Horizon let go of a kept start. */
    public static Object[] structureForget(long handle) {
        Worldgen120.structureForget(handle);
        return new Object[0];
    }

    /** A heightmap's height at a column of a mirror level: Horizon's, the level's bottom if it can't say. */
    public static int height(Level level, net.minecraft.world.level.levelgen.Heightmap.Types type, int x, int z) {
        Object[] answer = call.apply("height", new Object[] {dimension(level), type.name(), x, z});
        return answer.length > 0 && answer[0] instanceof Integer h ? h : level.getMinBuildHeight();
    }

    private static final Map<Level, Biomes120> BIOME_MANAGERS = new HashMap<>();

    /** A mirror level's biome manager, which asks Horizon (Biomes120). */
    public static net.minecraft.world.level.biome.BiomeManager biomeManager(Level level) {
        return BIOME_MANAGERS.computeIfAbsent(level, Biomes120::new);
    }

    private static final Map<Long, net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome>> BIOMES = new HashMap<>();

    /** The biome at a position of a mirror level: Horizon's, plains where it can't say. */
    public static net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> biome(Level level, BlockPos pos) {
        // Biomes come in 4x4x4 cells: one ask a cell, kept for the call.
        long cell = BlockPos.asLong(pos.getX() >> 2, pos.getY() >> 2, pos.getZ() >> 2);
        var known = BIOMES.get(cell);
        if (known != null) return known;
        var biomes = registries().registryOrThrow(Registries.BIOME);
        Object[] answer = call.apply("biome", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ()});
        net.minecraft.core.Holder<net.minecraft.world.level.biome.Biome> holder = null;
        if (answer.length > 0 && answer[0] instanceof String name) {
            var location = ResourceLocation.tryParse(name);
            if (location != null) {
                holder = biomes.getHolder(ResourceKey.create(Registries.BIOME, location)).orElse(null);
            }
        }
        if (holder == null) holder = biomes.getHolderOrThrow(net.minecraft.world.level.biome.Biomes.PLAINS);
        BIOMES.put(cell, holder);
        return holder;
    }

    /** The mods' biome modifications as data (Worldgen120.biomeChanges). */
    public static Object[] biomeChanges() {
        return new Object[] {Worldgen120.biomeChanges()};
    }

    public static Object[] keyProbe() {
        if (!keyProbeRegistered) {
            keyProbeRegistered = true;
            // Mods register their keys while they load, before the options exist; Fabric refuses
            // one after. The probe comes later, so it registers with the options set aside.
            var client = net.minecraft.client.Minecraft.getInstance();
            var options = client.options;
            var access = (dev.horizonlab.platform120.mixin.MinecraftAccess) client;
            access.horizon$setOptions(null);
            net.minecraft.client.KeyMapping mapping;
            try {
                mapping = net.fabricmc.fabric.api.client.keybinding.v1.KeyBindingHelper.registerKeyBinding(
                        new net.minecraft.client.KeyMapping("key.horizon.probe", 75, "key.categories.misc"));
            } finally {
                access.horizon$setOptions(options);
            }
            var probe = mapping;
            net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents.END_CLIENT_TICK.register(ticking -> {
                while (probe.consumeClick()) keyProbeClicks++;
            });
        }
        return new Object[] {keyProbeClicks};
    }

    /** The keys the mods' key bindings are set to, as GLFW's codes (Keys120). */
    public static Object[] modKeys() {
        return new Object[] {Keys120.modKeys()};
    }

    /**
     * `uuid`'s player pressed (or let go of) a key the mods bind, while playing: it is set and
     * clicked on the mods' mappings, and their next client tick reads it (Keys120).
     */
    public static Object[] keyPress(String uuid, String name, int key, boolean down) {
        begin();
        try {
            ServerPlayer player = player(uuid, name, level("minecraft:overworld"));
            Keys120.press(player, key, down);
            return new Object[] {true};
        } finally {
            finish();
        }
    }

    /**
     * Horizon's player `uuid` left the game: their screen closes as a server's closes on
     * disconnect (what the cursor held goes back first), the mods hear Fabric's
     * ServerPlayConnectionEvents.DISCONNECT, and the mirror player goes from the server's
     * list; the next time they reach the mods' code they join again.
     */
    public static Object[] playerLeft(String uuid) {
        ServerPlayer player = PLAYERS.get(UUID.fromString(uuid));
        if (player == null) return new Object[] {false};
        begin();
        try {
            Screens120.close(player);
            Keys120.forget(player.getUUID());
            Events120.left(server, player);
            PLAYERS.remove(player.getUUID());
            playerList.getPlayers().remove(player);
            ((dev.horizonlab.platform120.mixin.PlayerListAccess) playerList).horizon$setPlayersByUUID(playersByUuid());
            return new Object[] {true};
        } finally {
            finish();
        }
    }

    /**
     * A player of Horizon's right-clicks with `held` (1.20.1 item NBT) in `hand` (0 main, 1
     * off), at nothing or at a block that didn't take the click: 1.20.1's own
     * ServerPlayerGameMode.useItem, so the item's own use and Fabric's UseItemCallback run --
     * a wand's spell, a backpack or a guide book opening. Gives {code, the held item after as
     * 1.20.1 NBT, what opened (as useBlock's)}. The code is useBlock's (0 passed, 1 taken, 2
     * taken with a swing), except that a use the item holds over many ticks (eating, drawing a bow, raising a
     * shield) is let go and passed: Horizon runs those itself, for the content items it knows.
     */
    public static Object[] useItem(String uuid, String name, String dimension, int hand, String held, boolean creative,
            double px, double py, double pz, double yaw, double pitch) {
        begin();
        try {
            ServerLevel level = level(dimension);
            ServerPlayer player = player(uuid, name, level);
            stand(player, px, py, pz, yaw, pitch, px, py + 1.62, pz);
            mode(player, creative);
            InteractionHand which = hand == 1 ? InteractionHand.OFF_HAND : InteractionHand.MAIN_HAND;
            player.setItemInHand(which, heldStack(held));
            var menuBefore = player.containerMenu;
            Screens120.forgetTitle(player);
            Screens120.clientPlayer(player);
            ItemStack stack = player.getItemInHand(which);
            InteractionResult result = player.gameMode.useItem(player, level, stack, which);
            // As useBlock's: 0 passed, 1 taken, 2 taken with a swing of the arm.
            int code = !result.consumesAction() ? 0 : result.shouldSwing() ? 2 : 1;
            if (player.isUsingItem()) {
                player.stopUsingItem();
                code = 0;
            }
            ItemStack after = player.getItemInHand(which);
            String[] menu = opened(player, menuBefore, null);
            return new Object[] {code, after.isEmpty() ? "" : after.save(new net.minecraft.nbt.CompoundTag()).toString(),
                    menu == null ? "" : menu[0], menu == null ? "" : menu[1]};
        } finally {
            finish();
        }
    }

    /**
     * The mirror player where Horizon's player is: feet at (px, py, pz), looking (yaw, pitch)
     * in degrees as Minecraft counts them. A block placed faces the way they look
     * (BlockPlaceContext.getHorizontalDirection), and a block is placed only where they don't
     * stand. NaN for a position Horizon didn't give: the eyes at (hx, hy, hz), the old answer.
     */
    private static void stand(ServerPlayer player, double px, double py, double pz, double yaw, double pitch,
            double hx, double hy, double hz) {
        if (Double.isNaN(px) || Double.isNaN(py) || Double.isNaN(pz)) {
            player.setPos(hx, hy - 1.62, hz);
        } else {
            player.setPos(px, py, pz);
        }
        if (!Double.isNaN(yaw) && !Double.isNaN(pitch)) {
            player.setYRot((float) yaw);
            player.setXRot((float) pitch);
            player.setYHeadRot((float) yaw);
            player.yRotO = (float) yaw;
            player.xRotO = (float) pitch;
        }
    }

    private static void mode(ServerPlayer player, boolean creative) {
        GameType mode = creative ? GameType.CREATIVE : GameType.SURVIVAL;
        if (player.gameMode.getGameModeForPlayer() != mode) {
            ((dev.horizonlab.platform120.mixin.GameModeAccess) player.gameMode).horizon$setGameMode(mode, player.gameMode.getGameModeForPlayer());
        }
    }

    /** {@link #breakBlock}: the mods' block, broken by 1.20.1's own code. */
    static final int BREAK_THEIRS = 0;
    /** {@link #breakBlock}: another block, about to be broken by Horizon. */
    static final int BREAK_BEFORE = 1;
    /** {@link #breakBlock}: another block, just broken by Horizon. */
    static final int BREAK_AFTER = 2;

    /**
     * A player of Horizon's breaks the block at (x, y, z), holding `held` (1.20.1 item NBT, ""
     * for nothing), standing and looking as {@link #stand} says. `how`:
     *
     * <ul>
     *   <li>{@link #BREAK_THEIRS}: a 1.20.1 mod's block, broken by vanilla's
     *       ServerPlayerGameMode.destroyBlock, so the mod's own code runs as in 1.20.1: Fabric's
     *       PlayerBlockBreakEvents (BEFORE, AFTER, CANCELED, fired by Fabric itself), the block's
     *       playerWillDestroy, its block entity's contents dropped as it goes (a cooking pot's
     *       ingredients), its own loot table with the real tool, the tool's wear. What it set
     *       and dropped goes back as every call's changes do.
     *   <li>{@link #BREAK_BEFORE}: any other block, which Horizon breaks: only Fabric's
     *       PlayerBlockBreakEvents.BEFORE, so a 1.20.1 mod's listener may stop it (and then
     *       hears CANCELED, as Fabric does).
     *   <li>{@link #BREAK_AFTER}: the same block once Horizon broke it (`state` is what it was):
     *       PlayerBlockBreakEvents.AFTER.
     * </ul>
     *
     * Gives 1 when the break goes ahead (or went), 0 when a mod stopped it; and the held item
     * after, as 1.20.1 item NBT ("" for nothing).
     */
    public static Object[] breakBlock(String uuid, String name, String dimension, int x, int y, int z, String state,
            String held, boolean creative, double px, double py, double pz, double yaw, double pitch, int how) {
        begin();
        try {
            ServerLevel level = level(dimension);
            ServerPlayer player = player(uuid, name, level);
            stand(player, px, py, pz, yaw, pitch, x + 0.5, y + 0.5, z + 0.5);
            mode(player, creative);
            player.setItemInHand(InteractionHand.MAIN_HAND, heldStack(held));
            BlockPos pos = new BlockPos(x, y, z);
            Events120.around(level, pos);
            boolean goes;
            switch (how) {
                case BREAK_THEIRS -> goes = player.gameMode.destroyBlock(pos);
                case BREAK_BEFORE -> {
                    BlockState was = level.getBlockState(pos);
                    var entity = level.getBlockEntity(pos);
                    goes = net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents.BEFORE.invoker()
                            .beforeBlockBreak(level, player, pos, was, entity);
                    if (!goes) {
                        net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents.CANCELED.invoker()
                                .onBlockBreakCanceled(level, player, pos, was, entity);
                    }
                }
                default -> {
                    net.fabricmc.fabric.api.event.player.PlayerBlockBreakEvents.AFTER.invoker()
                            .afterBlockBreak(level, player, pos, state(state), null);
                    goes = true;
                }
            }
            ItemStack after = player.getMainHandItem();
            return new Object[] {goes ? 1 : 0, after.isEmpty() ? "" : after.save(new net.minecraft.nbt.CompoundTag()).toString()};
        } finally {
            finish();
        }
    }

    /** A held item from 1.20.1 item NBT; empty for "" or one that doesn't read. */
    private static ItemStack heldStack(String snbt) {
        if (snbt == null || snbt.isEmpty()) return ItemStack.EMPTY;
        try {
            return ItemStack.of(net.minecraft.nbt.TagParser.parseTag(snbt));
        } catch (com.mojang.brigadier.exceptions.CommandSyntaxException | RuntimeException e) {
            log("a held item didn't read (" + e.getMessage() + "): " + snbt);
            return ItemStack.EMPTY;
        }
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
            Events120.around(level, pos);
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
        return shapes(dimension, positions, false);
    }

    /**
     * What the block at each position is to walk into: `getCollisionShape(level, pos)`, as
     * {@link #blockShapes} gives the outline. A thin Wired Redstone wire is one to step on, as in
     * 1.20.1, where Horizon's registry has every LibMultipart container shapeless.
     */
    public static Object[] collisionShapes(String dimension, int[] positions) {
        return shapes(dimension, positions, true);
    }

    /**
     * The redstone each block at `positions` gives toward each side, twelve numbers a position:
     * `getSignal(level, pos, side)` then `getDirectSignal`, for the sides in vanilla's order
     * (down, up, north, south, west, east; `side` as vanilla passes it, from the block reading
     * toward this one). A wire carrying power lights the lamp it runs into.
     */
    public static Object[] blockSignals(String dimension, int[] positions) {
        begin();
        try {
            ServerLevel level = level(dimension);
            int count = positions.length / 3;
            int[] out = new int[count * 12];
            for (int i = 0; i < count; i++) {
                BlockPos pos = new BlockPos(positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2]);
                try {
                    BlockState state = blockState(level, pos);
                    for (net.minecraft.core.Direction side : net.minecraft.core.Direction.values()) {
                        int d = side.get3DDataValue();
                        out[i * 12 + d] = state.getSignal(level, pos, side);
                        out[i * 12 + 6 + d] = state.getDirectSignal(level, pos, side);
                    }
                } catch (RuntimeException | LinkageError e) {
                    shapeFailed(pos, e);
                }
            }
            return new Object[] {out};
        } finally {
            finish();
        }
    }

    /** The mods' block states that are redstone sources (`isSignalSource`), as Horizon names states. */
    public static Object[] signalSources() {
        setUp();
        java.util.List<String> out = new java.util.ArrayList<>();
        for (var block : net.minecraft.core.registries.BuiltInRegistries.BLOCK) {
            var id = net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(block);
            if ("minecraft".equals(id.getNamespace())) continue;
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                try {
                    if (state.isSignalSource()) {
                        out.add(net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(state));
                    }
                } catch (RuntimeException | LinkageError e) {
                    // A block that can't say is not one.
                }
            }
        }
        return new Object[] {out.toArray(new String[0])};
    }

    private static Object[] shapes(String dimension, int[] positions, boolean collision) {
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
                var shape = collision ? state.getCollisionShape(level, pos) : state.getShape(level, pos);
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

    /**
     * A Horizon hopper (or dropper) pushes one item into the 1.20.1 mod's block at (x, y, z)
     * through `face` (Direction.get3DDataValue): `stacks` is its slots in order, as
     * {@code [[id, count, damage] | null, ...]}. Gives the index of the slot whose item went in,
     * -1 when none fitted, -2 when the block has no item storage on that face (Hoppers120).
     */
    public static Object[] hopperInsert(String dimension, int x, int y, int z, int face, String stacks) {
        begin();
        try {
            return new Object[] {Hoppers120.insert(level(dimension), new BlockPos(x, y, z), face, stacks(stacks))};
        } catch (RuntimeException | LinkageError e) {
            hopperFailed(x, y, z, e);
            return new Object[] {Hoppers120.NO_STORAGE};
        } finally {
            finish();
        }
    }

    /**
     * A Horizon hopper pulls one item out of the 1.20.1 mod's block at (x, y, z) through
     * `face`: the first it offers that the hopper has room for (`anyRoom`: an empty slot; else
     * one of `partial`, as in {@link #hopperInsert}). Gives a code (0 moved, -1 nothing, -2 no
     * storage) and the item as 1.20.1 NBT.
     */
    public static Object[] hopperExtract(String dimension, int x, int y, int z, int face, boolean anyRoom, String partial) {
        begin();
        try {
            ItemStack one = Hoppers120.extract(level(dimension), new BlockPos(x, y, z), face, anyRoom, stacks(partial));
            if (one == null) return new Object[] {Hoppers120.NO_STORAGE, ""};
            if (one.isEmpty()) return new Object[] {Hoppers120.NOTHING, ""};
            return new Object[] {0, one.save(new net.minecraft.nbt.CompoundTag()).toString()};
        } catch (RuntimeException | LinkageError e) {
            hopperFailed(x, y, z, e);
            return new Object[] {Hoppers120.NO_STORAGE, ""};
        } finally {
            finish();
        }
    }

    private static final java.util.Set<String> HOPPER_FAILED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /** A block whose storage threw: said once per position; to Horizon it holds no items. */
    private static void hopperFailed(int x, int y, int z, Throwable e) {
        if (HOPPER_FAILED.add(x + " " + y + " " + z)) {
            log("a hopper at the block at " + x + " " + y + " " + z + " could not move items: " + e);
        }
    }

    /** Stacks as {@code [[id, count, damage] | null, ...]}, an empty one for each null. */
    private static java.util.List<ItemStack> stacks(String json) {
        var out = new java.util.ArrayList<ItemStack>();
        var slots = com.google.gson.JsonParser.parseString(json == null || json.isEmpty() ? "[]" : json).getAsJsonArray();
        for (var slot : slots) {
            if (slot != null && slot.isJsonArray()) {
                var a = slot.getAsJsonArray();
                out.add(stack(a.get(0).getAsString(), a.get(1).getAsInt(), a.get(2).getAsInt()));
            } else {
                out.add(ItemStack.EMPTY);
            }
        }
        return out;
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
