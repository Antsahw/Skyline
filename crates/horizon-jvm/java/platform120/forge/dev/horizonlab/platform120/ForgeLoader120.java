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

    /** Vanilla's key categories: a mapping in another is a mod's. */
    private static final Set<String> VANILLA_CATEGORIES = Set.of("key.categories.movement", "key.categories.misc",
            "key.categories.multiplayer", "key.categories.gameplay", "key.categories.ui", "key.categories.inventory",
            "key.categories.creative");

    @Override
    public List<KeyMapping> modKeyMappings() {
        List<KeyMapping> mods = new ArrayList<>();
        var options = Minecraft.getInstance().options;
        if (options == null) return mods;
        for (KeyMapping mapping : options.keyMappings) {
            if (!VANILLA_CATEGORIES.contains(mapping.getCategory())) mods.add(mapping);
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

    /** Forge's networking rides vanilla's custom payloads; the headless client's half comes later. */
    @Override public void toClient(ServerPlayer player, Packet<?> packet) {}

    @Override public void fromClient(Packet<?> packet) {}

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

    /** Models drawn by code: none told apart yet under Forge (phase 3). */
    @Override public String[] drawnByCode() { return new String[0]; }

    @Override public Object[] render(Level level, BlockPos pos) { return new Object[0]; }

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

    /** Forge applies biome modifiers as its server starts; the mirror's levels are told later (phase 3). */
    @Override public void finalizeBiomes(net.minecraft.core.RegistryAccess registries) {}

    @Override public boolean clientRunsItsLoop() { return true; }

    @Override public int firstEntityId() { return Entities120.FORGE_FIRST_ID; }

    @Override public Object[] packetProbe(ServerPlayer player) { return new Object[] {-1, -1, false, false}; }

    @Override public Object[] keyProbe() { return new Object[] {0}; }
}
