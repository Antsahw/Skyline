package dev.horizonlab.platform120;

import java.util.List;
import java.util.Map;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.model.BakedModel;
import net.minecraft.core.BlockPos;
import net.minecraft.network.protocol.Packet;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.EntityHitResult;

/**
 * The mod loader under the 1.20.1 platform (docs/java-bridge-120-forge.md): what the mirror asks
 * of the loader's own API, so the same sources build the platform for the Fabric JVM and for the
 * Forge JVM. Each JVM runs one loader; {@link #get} is that one. Everything not here is
 * Minecraft 1.20.1's and the same under both.
 */
public interface Loader120 {
    // Server events: the server starting, levels loading, players coming and going, ticks.

    /** The server starting and started, and the levels made so far loading: once. */
    void serverStart(MinecraftServer server, Iterable<ServerLevel> levels);

    /** Whether {@link #serverStart} ran. */
    boolean serverStarted();

    /** A level made after the start loading. */
    void levelLoaded(MinecraftServer server, ServerLevel level);

    void playerJoined(MinecraftServer server, ServerPlayer player);

    void playerLeft(MinecraftServer server, ServerPlayer player);

    /** The chunks around `pos` loading for the listeners that hear of chunks, once each. */
    void around(ServerLevel level, BlockPos pos);

    /** Whether any mod listens to server or level ticks. */
    boolean anyTickListeners();

    /** The start and end of a server tick and of `level`'s tick. */
    void serverTick(MinecraftServer server, ServerLevel level, long tick);

    // Client ticks: the headless client standing in for a player.

    /** Whether any mod listens to client ticks. */
    boolean clientTickListeners();

    /** A client tick's start and end listeners. */
    void clientTick(Minecraft minecraft);

    /** The mods' key mappings. */
    List<KeyMapping> modKeyMappings();

    // Players acting.

    /** A player uses an entity: the loader's listeners first; PASS lets vanilla's code run. */
    InteractionResult useEntity(ServerPlayer player, ServerLevel level, InteractionHand hand, Entity target, EntityHitResult hit);

    /** A player is about to break a block Horizon breaks: false when a listener stops it. */
    boolean beforeBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    /** A break {@link #beforeBreak} stopped. */
    void canceledBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    /** A block Horizon broke (`state` is what it was). */
    void afterBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity);

    // Items moving: Horizon's hoppers on the mods' storages.

    /** Hoppers120.insert: the index of the stack one item of went in, or NOTHING / NO_STORAGE. */
    int hopperInsert(ServerLevel level, BlockPos pos, int face, List<ItemStack> stacks);

    /** Hoppers120.extract: one item taken, empty for none, null without storage. */
    ItemStack hopperExtract(ServerLevel level, BlockPos pos, int face, boolean anyRoom, List<ItemStack> partial);

    // Packets between the headless client and the server half.

    /** A packet the server sends `player`. */
    void toClient(ServerPlayer player, Packet<?> packet);

    /** A packet the client half sends the server. */
    void fromClient(Packet<?> packet);

    // Client resources and models: the loader's own model loading and rendering.

    /** Vanilla's and the mods' client resource packs. */
    MultiPackResourceManager clientResources();

    /** Every model baked, by location. */
    Map<ResourceLocation, BakedModel> topLevel();

    /** Every state's baked model. */
    Map<BlockState, BakedModel> baked();

    /** The blocks whose models are drawn by code. */
    String[] drawnByCode();

    /** The quads of the block at `pos`, as its model draws it there. */
    Object[] render(Level level, BlockPos pos);

    // Data: the mods' own packs and their changes to the world's registries.

    /** The source of the mods' own resource or data packs; null when it can't be had. */
    net.minecraft.server.packs.repository.RepositorySource modPacks(net.minecraft.server.packs.PackType type);

    /** The mods' biome modifications applied to `registries` (features, spawns, effects). */
    void finalizeBiomes(net.minecraft.core.RegistryAccess registries);

    /** Where this runtime's mods' entities' ids start (Entities120's FIRST_ID, FORGE_FIRST_ID). */
    int firstEntityId();

    /** The data packs' registries a world loads: vanilla's and those the loader's mods add. */
    java.util.List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>> worldgenRegistries();

    // The bridge's own checks of the loader.

    /** A payload down to the client half and one back up: what each side got, and canSend. */
    Object[] packetProbe(ServerPlayer player);

    /** A key mapping of the bridge's own, and how often a client tick saw it clicked. */
    Object[] keyProbe();

    /** The loader this JVM runs, found once. */
    static Loader120 get() {
        return Holder.LOADER;
    }

    /**
     * Whether the client is headless now: no GL, no window, no ticks of its own; the platform's
     * mixins for that (GL calls stopped, the window stood in for, keys from Horizon) are on.
     * Always under Fabric, whose runtime starts a client with none. Forge's client is a real
     * one until every mod has loaded; then {@link #goHeadless} parks it.
     */
    static boolean headless() {
        return Holder.headless;
    }

    /** From now on the client is headless ({@link #headless}): its render thread parks. */
    static void goHeadless() {
        Holder.headless = true;
    }

    /**
     * `then` on the render thread once the client has finished loading (its first resource
     * reload done: the models baked, the loading overlay gone), before it goes headless: Forge's
     * mods have loaded well before that (FMLLoadCompleteEvent), its models not.
     */
    static void whenLoaded(Runnable then) {
        Holder.whenLoaded = then;
    }

    /** MinecraftParkMixin, each frame until it has run: the {@link #whenLoaded} waiting, if due. */
    static void runWhenLoaded(boolean loaded) {
        Runnable then = Holder.whenLoaded;
        if (then != null && loaded) {
            Holder.whenLoaded = null;
            then.run();
        }
    }

    final class Holder {
        private Holder() {}

        /** Read by the GL mixins from the first GL call: not through LOADER, which is made later. */
        static volatile boolean headless = exists("net.fabricmc.loader.api.FabricLoader");

        /** What waits for the client to finish loading ({@link #whenLoaded}). */
        static volatile Runnable whenLoaded;

        static final Loader120 LOADER = find();

        private static Loader120 find() {
            String name = exists("net.fabricmc.loader.api.FabricLoader")
                    ? "dev.horizonlab.platform120.FabricLoader120"
                    : "dev.horizonlab.platform120.ForgeLoader120";
            try {
                return (Loader120) Class.forName(name).getDeclaredConstructor().newInstance();
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException("no 1.20.1 loader side: " + name, e);
            }
        }

        private static boolean exists(String name) {
            try {
                Class.forName(name, false, Loader120.class.getClassLoader());
                return true;
            } catch (ClassNotFoundException e) {
                return false;
            }
        }
    }
}
