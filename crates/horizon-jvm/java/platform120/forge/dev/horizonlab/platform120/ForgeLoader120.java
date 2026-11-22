package dev.horizonlab.platform120;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.resources.model.BakedModel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.network.protocol.Packet;
import net.minecraft.resources.ResourceLocation;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.RepositorySource;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.InteractionResult;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.EntityHitResult;
import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.common.capabilities.ForgeCapabilities;
import net.minecraftforge.event.TickEvent;
import net.minecraftforge.event.entity.player.PlayerEvent;
import net.minecraftforge.event.entity.player.PlayerInteractEvent;
import net.minecraftforge.event.level.BlockEvent;
import net.minecraftforge.event.level.ChunkEvent;
import net.minecraftforge.event.level.LevelEvent;
import net.minecraftforge.event.server.ServerAboutToStartEvent;
import net.minecraftforge.event.server.ServerStartedEvent;
import net.minecraftforge.event.server.ServerStartingEvent;
import net.minecraftforge.eventbus.api.Event;
import net.minecraftforge.fml.LogicalSide;
import net.minecraftforge.items.IItemHandler;

/**
 * {@link Loader120} under Forge 1.20.1 (docs/java-bridge-120-forge.md, phase 3): Forge's events
 * posted on its bus as Forge's documentation describes them
 * (https://docs.minecraftforge.net/en/1.20.x/), its item handler capability for hoppers. Built
 * only into the Forge platform jar. Written from Forge's public API (its documentation and the
 * signatures of its public classes), never from its sources, which are LGPL.
 */
final class ForgeLoader120 implements Loader120 {
    private static final Set<String> FAILED = new HashSet<>();
    private boolean started;
    private final Set<Long> chunksLoaded = new HashSet<>();

    /** Posts `event` on Forge's bus; a listener that throws is said once and the call goes on. */
    private static <T extends Event> T post(T event, String what) {
        try {
            MinecraftForge.EVENT_BUS.post(event);
        } catch (RuntimeException | LinkageError e) {
            if (FAILED.add(what)) Mirror120.log(what + ": a mod's listener failed: " + e);
        }
        return event;
    }

    @Override
    public void serverStart(MinecraftServer server, Iterable<ServerLevel> levels) {
        if (started) return;
        started = true;
        post(new ServerAboutToStartEvent(server), "server about to start");
        post(new ServerStartingEvent(server), "server starting");
        for (ServerLevel level : levels) post(new LevelEvent.Load(level), "level load");
        post(new ServerStartedEvent(server), "server started");
    }

    @Override public boolean serverStarted() { return started; }

    @Override
    public void levelLoaded(MinecraftServer server, ServerLevel level) {
        post(new LevelEvent.Load(level), "level load");
    }

    @Override
    public void playerJoined(MinecraftServer server, ServerPlayer player) {
        post(new PlayerEvent.PlayerLoggedInEvent(player), "player join");
    }

    @Override
    public void playerLeft(MinecraftServer server, ServerPlayer player) {
        post(new PlayerEvent.PlayerLoggedOutEvent(player), "player leave");
    }

    @Override
    public void around(ServerLevel level, BlockPos pos) {
        // The chunk at `pos` loading, once: Forge's ChunkEvent.Load, as a level's chunk source
        // fires it when a chunk is loaded.
        long key = (long) (pos.getX() >> 4) << 32 ^ (pos.getZ() >> 4) & 0xffffffffL ^ (long) level.dimension().location().hashCode() << 48;
        if (!chunksLoaded.add(key)) return;
        try {
            post(new ChunkEvent.Load(level.getChunk(pos), false), "a chunk loading");
        } catch (RuntimeException e) {
            if (FAILED.add("chunk")) Mirror120.log("a chunk's load event could not be made: " + e);
        }
    }

    /** Forge's bus can't say who listens without posting: ticks are always posted. */
    @Override public boolean anyTickListeners() { return true; }

    @Override
    public void serverTick(MinecraftServer server, ServerLevel level, long tick) {
        post(new TickEvent.ServerTickEvent(TickEvent.Phase.START, () -> true, server), "server tick start");
        post(new TickEvent.LevelTickEvent(LogicalSide.SERVER, TickEvent.Phase.START, level, () -> true), "level tick start");
        post(new TickEvent.LevelTickEvent(LogicalSide.SERVER, TickEvent.Phase.END, level, () -> true), "level tick end");
        post(new TickEvent.ServerTickEvent(TickEvent.Phase.END, () -> true, server), "server tick end");
    }

    @Override public boolean clientTickListeners() { return true; }

    @Override
    public void clientTick(Minecraft minecraft) {
        post(new TickEvent.ClientTickEvent(TickEvent.Phase.START), "client tick start");
        post(new TickEvent.ClientTickEvent(TickEvent.Phase.END), "client tick end");
    }

    /**
     * The mods' key mappings: the game's (Forge adds those registered with RegisterKeyMappingsEvent
     * to them) but vanilla's own, which are the options' fields (keyUp..., the hotbar's slots).
     */
    @Override
    public List<KeyMapping> modKeyMappings() {
        List<KeyMapping> mods = new ArrayList<>();
        var options = Minecraft.getInstance().options;
        if (options == null) return mods;
        Set<KeyMapping> vanilla = java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
        for (var field : net.minecraft.client.Options.class.getFields()) {
            if (java.lang.reflect.Modifier.isStatic(field.getModifiers())) continue;
            try {
                if (field.getType() == KeyMapping.class) {
                    vanilla.add((KeyMapping) field.get(options));
                } else if (field.getType() == KeyMapping[].class && field.get(options) != options.keyMappings) {
                    vanilla.addAll(List.of((KeyMapping[]) field.get(options)));
                }
            } catch (IllegalAccessException e) {
                Mirror120.log("vanilla's key mappings could not be read: " + e);
            }
        }
        for (KeyMapping mapping : options.keyMappings) {
            if (!vanilla.contains(mapping)) mods.add(mapping);
        }
        return mods;
    }

    @Override
    public InteractionResult useEntity(ServerPlayer player, ServerLevel level, InteractionHand hand, Entity target, EntityHitResult hit) {
        var specific = post(new PlayerInteractEvent.EntityInteractSpecific(player, hand, target,
                hit.getLocation().subtract(target.position())), "entity use (at)");
        if (specific.isCanceled()) return specific.getCancellationResult();
        var plain = post(new PlayerInteractEvent.EntityInteract(player, hand, target), "entity use");
        if (plain.isCanceled()) return plain.getCancellationResult();
        return InteractionResult.PASS;
    }

    @Override
    public boolean beforeBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {
        return !post(new BlockEvent.BreakEvent(level, pos, state, player), "block break").isCanceled();
    }

    /** Forge has no event for a stopped break: its BreakEvent, cancelled, was it. */
    @Override public void canceledBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {}

    /** Forge has no event after a break: BreakEvent before it is the one mods hear. */
    @Override public void afterBreak(ServerLevel level, ServerPlayer player, BlockPos pos, BlockState state, BlockEntity entity) {}

    private static IItemHandler handler(ServerLevel level, BlockPos pos, int face) {
        BlockEntity entity = level.getBlockEntity(pos);
        if (entity == null) return null;
        return entity.getCapability(ForgeCapabilities.ITEM_HANDLER, Direction.from3DDataValue(face)).orElse(null);
    }

    @Override
    public int hopperInsert(ServerLevel level, BlockPos pos, int face, List<ItemStack> stacks) {
        IItemHandler target = handler(level, pos, face);
        if (target == null) return Hoppers120.NO_STORAGE;
        for (int i = 0; i < stacks.size(); i++) {
            ItemStack stack = stacks.get(i);
            if (stack.isEmpty()) continue;
            ItemStack one = stack.copyWithCount(1);
            for (int slot = 0; slot < target.getSlots(); slot++) {
                if (target.insertItem(slot, one, true).isEmpty()) {
                    target.insertItem(slot, one, false);
                    return i;
                }
            }
        }
        return Hoppers120.NOTHING;
    }

    @Override
    public ItemStack hopperExtract(ServerLevel level, BlockPos pos, int face, boolean anyRoom, List<ItemStack> partial) {
        IItemHandler source = handler(level, pos, face);
        if (source == null) return null;
        for (int slot = 0; slot < source.getSlots(); slot++) {
            ItemStack offered = source.extractItem(slot, 1, true);
            if (offered.isEmpty()) continue;
            if (!anyRoom && partial.stream().noneMatch(r -> ItemStack.isSameItemSameTags(r, offered))) continue;
            ItemStack taken = source.extractItem(slot, 1, false);
            if (!taken.isEmpty()) return taken;
        }
        return ItemStack.EMPTY;
    }

    /**
     * Forge's networking rides vanilla's custom payloads: a payload the server sends a mirror
     * player goes straight to Forge's client half (NetworkHooks.onCustomPayload, as the client's
     * packet listener hands it on), and one the client sends to the server half.
     */
    @Override
    public void toClient(ServerPlayer player, Packet<?> packet) {
        if (!(packet instanceof net.minecraft.network.protocol.game.ClientboundCustomPayloadPacket payload)
                || player == null) return;
        Screens120.clientPlayer(player);
        var listener = Minecraft.getInstance().getConnection();
        deliver("to the client on " + payload.getIdentifier(), payload, clientConnection(listener));
    }

    @Override
    public void fromClient(Packet<?> packet) {
        if (!(packet instanceof net.minecraft.network.protocol.game.ServerboundCustomPayloadPacket payload)) return;
        ServerPlayer player = Screens120.standingIn();
        if (player == null) return;
        deliver("to the server on " + payload.getIdentifier(), payload, player.connection.connection);
    }

    private static net.minecraft.network.Connection clientConnection;

    /** The client listener's connection, for Forge's NetworkEvent.Context: no channel behind it. */
    private static net.minecraft.network.Connection clientConnection(net.minecraft.network.PacketListener listener) {
        if (clientConnection == null) {
            var connection = Allocate.make(net.minecraft.network.Connection.class);
            ((dev.horizonlab.platform120.mixin.ConnectionAccess) connection)
                    .horizon$setChannel(new io.netty.channel.embedded.EmbeddedChannel());
            clientConnection = connection;
        }
        ((dev.horizonlab.platform120.mixin.ConnectionAccess) clientConnection).horizon$setListener(listener);
        return clientConnection;
    }

    private static final java.util.Set<String> PACKETS_FAILED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    private static void deliver(String what, net.minecraftforge.network.ICustomPacket<?> payload,
            net.minecraft.network.Connection connection) {
        if (System.getenv("HORIZON_120_PACKETS") != null) System.out.println("ForgeLoader120: " + what);
        try {
            net.minecraftforge.network.NetworkHooks.onCustomPayload(payload, connection);
        } catch (RuntimeException | LinkageError e) {
            if (PACKETS_FAILED.add(what + " " + e.getClass().getName())) {
                Mirror120.log("a mod's packet " + what + " failed: " + e);
            }
        }
    }

    /** The client's own packs, as Forge's client loaded them: vanilla's and every mod's. */
    @Override
    public MultiPackResourceManager clientResources() {
        var repository = Minecraft.getInstance().getResourcePackRepository();
        return new MultiPackResourceManager(PackType.CLIENT_RESOURCES, repository.openAllSelected());
    }

    @Override
    public Map<ResourceLocation, BakedModel> topLevel() {
        return Minecraft.getInstance().getModelManager().getModelBakery().getBakedTopLevelModels();
    }

    @Override
    public Map<BlockState, BakedModel> baked() {
        Map<BlockState, BakedModel> out = new java.util.HashMap<>();
        var shaper = Minecraft.getInstance().getModelManager().getBlockModelShaper();
        for (var block : net.minecraft.core.registries.BuiltInRegistries.BLOCK) {
            for (BlockState state : block.getStateDefinition().getPossibleStates()) out.put(state, shaper.getBlockModel(state));
        }
        return out;
    }

    /**
     * The mods' states whose model is a mod's own code (a baked model class of its own, as
     * Forge's geometry loaders make them: connected textures, models of a block entity's data):
     * what they draw is asked for where they stand. Vanilla's and Forge's own model classes
     * (simple, multipart, weighted, composite, OBJ) draw the same everywhere, from their files.
     */
    @Override
    public String[] drawnByCode() {
        List<String> out = new ArrayList<>();
        for (var entry : baked().entrySet()) {
            BlockState state = entry.getKey();
            if ("minecraft".equals(net.minecraft.core.registries.BuiltInRegistries.BLOCK.getKey(state.getBlock()).getNamespace())) continue;
            String model = entry.getValue().getClass().getName();
            if (model.startsWith("net.minecraft.") || model.startsWith("net.minecraftforge.")) continue;
            out.add(net.minecraft.commands.arguments.blocks.BlockStateParser.serialize(state));
        }
        return out.toArray(new String[0]);
    }

    /**
     * The quads of the block at `pos`, as its model gives them there (Forge's model data, every
     * render type, every side): {sprites, corners (x y z u v, u v in the sprite), colours}, as
     * Models120.render answers.
     */
    @Override
    public Object[] render(Level level, BlockPos pos) {
        BlockState state = level.getBlockState(pos);
        var model = Minecraft.getInstance().getModelManager().getBlockModelShaper().getBlockModel(state);
        var view = new Models120.View(level);
        List<String> sprites = new ArrayList<>();
        List<Double> corners = new ArrayList<>();
        List<Integer> colors = new ArrayList<>();
        try {
            var entity = level.getBlockEntity(pos);
            var data = model.getModelData(view, pos, state,
                    entity == null ? net.minecraftforge.client.model.data.ModelData.EMPTY : entity.getModelData());
            var random = net.minecraft.util.RandomSource.create();
            long seed = state.getSeed(pos);
            for (var type : model.getRenderTypes(state, net.minecraft.util.RandomSource.create(seed), data)) {
                for (int side = -1; side < 6; side++) {
                    random.setSeed(seed);
                    var direction = side < 0 ? null : net.minecraft.core.Direction.from3DDataValue(side);
                    for (var quad : model.getQuads(state, direction, random, data, type)) {
                        quad(quad, state, view, pos, sprites, corners, colors);
                    }
                }
            }
        } catch (RuntimeException | LinkageError e) {
            Throwable cause = e;
            while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
            var at = cause.getStackTrace().length > 0 ? " at " + cause.getStackTrace()[0] : "";
            Mirror120.log("the block at " + pos.toShortString() + " (" + state + ") didn't draw: " + cause + at);
        }
        double[] flat = new double[corners.size()];
        for (int i = 0; i < flat.length; i++) flat[i] = corners.get(i);
        int[] argb = new int[colors.size()];
        for (int i = 0; i < argb.length; i++) argb[i] = colors.get(i);
        return new Object[] {sprites.toArray(new String[0]), flat, argb};
    }

    /** One baked quad: vanilla's block vertex format, eight ints a corner. */
    private static void quad(net.minecraft.client.renderer.block.model.BakedQuad quad, BlockState state,
            Models120.View view, BlockPos pos, List<String> sprites, List<Double> corners, List<Integer> colors) {
        int[] vertices = quad.getVertices();
        if (vertices.length < 32) return;
        var sprite = quad.getSprite();
        float u0 = sprite.getU0(), du = sprite.getU1() - u0, v0 = sprite.getV0(), dv = sprite.getV1() - v0;
        int tint = -1;
        if (quad.isTinted()) {
            tint = 0xFF000000 | Minecraft.getInstance().getBlockColors().getColor(state, view, pos, quad.getTintIndex());
        }
        sprites.add(sprite.contents().name().toString());
        int stride = vertices.length / 4;
        for (int c = 0; c < 4; c++) {
            int at = c * stride;
            corners.add((double) Float.intBitsToFloat(vertices[at]));
            corners.add((double) Float.intBitsToFloat(vertices[at + 1]));
            corners.add((double) Float.intBitsToFloat(vertices[at + 2]));
            corners.add(du == 0 ? 0 : (double) ((Float.intBitsToFloat(vertices[at + 4]) - u0) / du));
            corners.add(dv == 0 ? 0 : (double) ((Float.intBitsToFloat(vertices[at + 5]) - v0) / dv));
            int abgr = vertices[at + 3];
            int argb = abgr & 0xFF00FF00 | (abgr & 0xFF) << 16 | abgr >> 16 & 0xFF;
            colors.add(tint == -1 ? argb : multiply(argb, tint));
        }
    }

    private static int multiply(int a, int b) {
        int out = 0;
        for (int shift = 0; shift < 32; shift += 8) {
            out |= ((a >>> shift & 0xFF) * (b >>> shift & 0xFF) / 255) << shift;
        }
        return out;
    }

    /** Each mod's own pack, as Forge makes one per mod file. */
    @Override
    public RepositorySource modPacks(PackType type) {
        return add -> {
            for (String name : net.minecraftforge.resource.ResourcePackLoader.getPackNames()) {
                if (!name.startsWith("mod:")) continue;
                String id = name.substring("mod:".length());
                net.minecraftforge.resource.ResourcePackLoader.getPackFor(id).ifPresent(resources -> {
                    var pack = net.minecraft.server.packs.repository.Pack.readMetaAndCreate(name,
                            net.minecraft.network.chat.Component.literal(id), true, n -> resources, type,
                            net.minecraft.server.packs.repository.Pack.Position.BOTTOM,
                            net.minecraft.server.packs.repository.PackSource.DEFAULT);
                    if (pack != null) add.accept(pack);
                });
            }
        };
    }

    /**
     * The mods' biome modifiers (data, `forge:biome_modifier`), applied to the mirror's biomes as
     * Forge applies them to a server's as it starts: each biome's modifiable info, with every
     * modifier the registries hold. A biome's features and spawns read the modified info after.
     */
    @Override
    public void finalizeBiomes(net.minecraft.core.RegistryAccess registries) {
        var found = registries.registry(net.minecraftforge.registries.ForgeRegistries.Keys.BIOME_MODIFIERS);
        if (found.isEmpty()) {
            Mirror120.log("the mirror's registries have no biome modifiers");
            return;
        }
        List<net.minecraftforge.common.world.BiomeModifier> modifiers = found.get().stream().toList();
        var biomes = registries.registryOrThrow(net.minecraft.core.registries.Registries.BIOME);
        int done = 0;
        for (var holder : biomes.holders().toList()) {
            var info = holder.value().modifiableBiomeInfo();
            if (info.getModifiedBiomeInfo() != null) continue;
            try {
                info.applyBiomeModifiers(holder, modifiers);
                done++;
            } catch (RuntimeException | LinkageError e) {
                Mirror120.log("biome " + holder.key().location() + " can't be modified: " + e);
            }
        }
        Mirror120.log(modifiers.size() + " biome modifier(s) applied to " + done + " biome(s)");
    }

    @Override public int firstEntityId() { return Entities120.FORGE_FIRST_ID; }

    /** Vanilla's and Forge's data pack registries (biome modifiers, the mods' own). */
    @Override
    public List<net.minecraft.resources.RegistryDataLoader.RegistryData<?>> worldgenRegistries() {
        return net.minecraftforge.registries.DataPackRegistriesHooks.getDataPackRegistries();
    }

    @Override public Object[] packetProbe(ServerPlayer player) { return new Object[] {-1, -1, false, false}; }

    @Override public Object[] keyProbe() { return new Object[] {0}; }
}
