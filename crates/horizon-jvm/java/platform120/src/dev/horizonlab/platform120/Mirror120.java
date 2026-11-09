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
            sl.horizon$setChunkSource(chunks);
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

    /** A new call from Horizon: what the last one wrote is Horizon's now. */
    static void begin() {
        WRITES.clear();
    }

    /** Level.getBlockState of a mirror level: this call's own write, else Horizon's block. */
    public static BlockState blockState(Level level, BlockPos pos) {
        Map<Long, BlockState> writes = WRITES.get(level);
        if (writes != null) {
            BlockState written = writes.get(pos.asLong());
            if (written != null) return written;
        }
        if (level.isOutsideBuildHeight(pos)) return Blocks.VOID_AIR.defaultBlockState();
        Object[] answer = call.apply("blockState", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ()});
        String name = answer.length > 0 && answer[0] instanceof String s ? s : null;
        // Where Horizon has no chunk loaded: air, as vanilla reads an unloaded position.
        if (name == null) return Blocks.AIR.defaultBlockState();
        return state(name);
    }

    /** Level.setBlock of a mirror level: kept for this call and sent to Horizon. */
    public static boolean setBlock(Level level, BlockPos pos, BlockState state, int flags) {
        if (level.isOutsideBuildHeight(pos)) return false;
        BlockState before = blockState(level, pos);
        if (before == state) return false;
        WRITES.computeIfAbsent(level, l -> new HashMap<>()).put(pos.asLong(), state);
        event.accept("setBlock", new Object[] {dimension(level), pos.getX(), pos.getY(), pos.getZ(), BlockStateParser.serialize(state), flags});
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
        InteractionResult result = player.gameMode.useItemOn(player, level, stack, h, hit);
        ItemStack after = player.getItemInHand(h);
        int code = !result.consumesAction() ? 0 : result.shouldSwing() ? 2 : 1;
        return new Object[] {code, BuiltInRegistries.ITEM.getKey(after.getItem()).toString(), after.getCount(), after.getDamageValue()};
    }

    private static ItemStack stack(String item, int count, int damage) {
        ResourceLocation id = ResourceLocation.tryParse(item);
        if (id == null || count <= 0) return ItemStack.EMPTY;
        Item it = BuiltInRegistries.ITEM.get(id);
        ItemStack stack = new ItemStack(it, count);
        if (damage > 0) stack.setDamageValue(damage);
        return stack;
    }
}
