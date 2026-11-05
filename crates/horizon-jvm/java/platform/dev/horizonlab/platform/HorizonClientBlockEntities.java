package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.ArrayList;
import java.util.Map;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.network.protocol.game.ClientboundBlockEntityDataPacket;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.ProblemReporter;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.EntityBlock;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityTicker;
import net.minecraft.world.level.storage.TagValueInput;

/**
 * The client's own block entities of mod blocks, as a vanilla client has them: made by the
 * block entity's type in the client level, loaded from what the server would send it (the
 * chunk's update tag, then each ClientboundBlockEntityDataPacket at sendBlockUpdated), ticked
 * by the block's client ticker. Mods keep client-only state on them: Create adds its client
 * behaviours (goggles' tooltips, value boxes, frogport sounds) when its block entity
 * initializes in a client level; with the server's own object they were never there.
 *
 * <p>A copy is made when client code first asks the client level for it, and kept while the
 * server's block entity is the same object and someone asked for it in the last minute.
 * Besides the server's updates (vanilla's), a copy is refreshed from the server's block
 * entity at most every 10 ticks after it changed (setChanged): Horizon's client sees each
 * mod block's state without the packets a mod may rely on a dedicated server to send.
 * While copies tick and their client code runs, the client's changes to the world are the
 * client's prediction, dropped (HorizonServer.clientHalf), as the client half of a use's are.
 */
public final class HorizonClientBlockEntities {
    private HorizonClientBlockEntities() {}

    private static final int REFRESH_TICKS = 10;
    /** HORIZON_JAVA_DEBUG=cbe: each copy made, and each level's copies ticked now and then. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("cbe");
    private static final int KEEP_TICKS = 1200;

    private static final class Copy {
        final BlockEntity server;
        final BlockEntity client;
        BlockEntityTicker<BlockEntity> ticker;
        boolean tickerMade;
        /** sendBlockUpdated: the server's update, loaded at the next use. */
        volatile boolean updated;
        /** setChanged: loaded once REFRESH_TICKS passed since the last load. */
        volatile boolean changed;
        long loadedAt;
        long usedAt;

        Copy(BlockEntity server, BlockEntity client) {
            this.server = server;
            this.client = client;
        }
    }

    private static final Map<ClientLevel, Map<Long, Copy>> COPIES = new java.util.concurrent.ConcurrentHashMap<>();
    private static volatile long now;

    /** Level.getBlockEntity on a client level: the client's copy of a mod's block entity. */
    public static BlockEntity get(ClientLevel level, BlockPos pos) {
        Level serverLevel = HorizonServer.serverSide(level);
        BlockEntity server = HorizonBlockEntities.get(serverLevel, pos);
        Map<Long, Copy> copies = COPIES.get(level);
        long key = pos.asLong();
        Copy copy = copies == null ? null : copies.get(key);
        if (server == null || !HorizonBlockEntities.modded(server.getBlockState())) {
            if (copy != null) drop(copies, key, copy);
            return server;
        }
        if (copy != null && copy.server != server) {
            drop(copies, key, copy);
            copy = null;
        }
        if (copy == null) {
            BlockEntity made = make(level, server);
            if (made == null) return server;
            copy = new Copy(server, made);
            if (DEBUG) Natives.log("cbe: client copy of " + made.getClass().getName() + " at " + pos.toShortString());
            copy.loadedAt = now;
            COPIES.computeIfAbsent(level, l -> new java.util.concurrent.ConcurrentHashMap<>()).put(key, copy);
        } else {
            refresh(level, copy);
        }
        copy.usedAt = now;
        if (copy.client.getBlockState() != server.getBlockState()) copy.client.setBlockState(server.getBlockState());
        return copy.client;
    }

    /** ServerLevel.sendBlockUpdated on a mirror: what the server would send the client. */
    public static void updated(ServerLevel level, BlockPos pos) {
        for (var e : COPIES.entrySet()) {
            if (HorizonServer.serverSide(e.getKey()) != level) continue;
            Copy copy = e.getValue().get(pos.asLong());
            if (copy != null) copy.updated = true;
        }
    }

    /** BlockEntity.setChanged on a mirror's block entity. */
    public static void changed(ServerLevel level, BlockPos pos) {
        if (COPIES.isEmpty()) return;
        for (var e : COPIES.entrySet()) {
            if (HorizonServer.serverSide(e.getKey()) != level) continue;
            Copy copy = e.getValue().get(pos.asLong());
            if (copy != null) copy.changed = true;
        }
    }

    /**
     * A client tick of a level's copies (ClientLevel.tickBlockEntities), as the player whose
     * client runs now: copies whose server block entity went are removed, the rest loaded if
     * the server updated them, then ticked by their block's client ticker.
     */
    public static void tick(ClientLevel level) {
        Map<Long, Copy> copies = COPIES.get(level);
        if (copies == null || copies.isEmpty()) return;
        Level serverLevel = HorizonServer.serverSide(level);
        java.util.List<Copy> list = new ArrayList<>();
        for (var e : new ArrayList<>(copies.entrySet())) {
            Copy copy = e.getValue();
            BlockEntity server = HorizonBlockEntities.peek(serverLevel, BlockPos.of(e.getKey()));
            if (server != copy.server || server.isRemoved() || now - copy.usedAt > KEEP_TICKS) {
                copies.remove(e.getKey(), copy);
                copy.client.setRemoved();
            } else {
                list.add(copy);
            }
        }
        if (DEBUG && now % 100 == 0) Natives.log("cbe: " + list.size() + " client block entities tick in " + level.dimension().identifier());
        Thread before = HorizonServer.clientHalf;
        HorizonServer.clientHalf = Thread.currentThread();
        try {
            for (Copy copy : list) {
                refresh(level, copy);
                if (!copy.tickerMade) {
                    copy.tickerMade = true;
                    copy.ticker = ticker(level, copy.client);
                }
                if (copy.ticker == null) continue;
                try {
                    copy.ticker.tick(level, copy.client.getBlockPos(), copy.client.getBlockState(), copy.client);
                } catch (Throwable t) {
                    // One mod's client block entity failing doesn't stop the others; it ticks no more.
                    copy.ticker = null;
                    Throwable root = t;
                    while (root.getCause() != null && root.getCause() != root) root = root.getCause();
                    Natives.log("Horizon's client: a mod's block entity at " + copy.client.getBlockPos() + " failed in its client tick: " + root);
                    HorizonFailures.report("a block entity's client tick", root);
                }
            }
        } finally {
            HorizonServer.clientHalf = before;
        }
    }

    /** How far from a player the client keeps copies of every mod block entity (blocks). */
    private static final int NEAR = Integer.getInteger("horizon.clientBlockEntities", 32);

    /**
     * The copies of the mod block entities near a player, as a vanilla client holds every one
     * in its loaded chunks: made before the player looks at or clicks one, so they have ticked
     * (Create adds its client behaviours at its first tick) when client code asks.
     */
    public static void near(ClientLevel level, net.minecraft.world.entity.player.Player player) {
        if (NEAR <= 0) return;
        Level serverLevel = HorizonServer.serverSide(level);
        if (!(serverLevel instanceof ServerLevel server)) return;
        double far = (double) NEAR * NEAR;
        for (BlockEntity be : HorizonBlockEntities.live(server)) {
            if (be.getBlockPos().distToCenterSqr(player.position()) > far) continue;
            get(level, be.getBlockPos());
        }
    }

    /** The end of a client tick (all levels ticked). */
    public static void endTick() {
        now++;
    }

    /** The world closed: the copies belong to its levels. */
    public static void clear() {
        COPIES.values().forEach(m -> m.values().forEach(c -> c.client.setRemoved()));
        COPIES.clear();
    }

    @SuppressWarnings("unchecked")
    private static BlockEntityTicker<BlockEntity> ticker(ClientLevel level, BlockEntity be) {
        try {
            if (!(be.getBlockState().getBlock() instanceof EntityBlock block)) return null;
            return (BlockEntityTicker<BlockEntity>) block.getTicker(level, be.getBlockState(), (net.minecraft.world.level.block.entity.BlockEntityType<BlockEntity>) be.getType());
        } catch (Throwable t) {
            Natives.log("Horizon's client: no client ticker for the block entity at " + be.getBlockPos() + ": " + t);
            return null;
        }
    }

    private static void drop(Map<Long, Copy> copies, long key, Copy copy) {
        copies.remove(key);
        copy.client.setRemoved();
    }

    /** A copy loaded again from the server's block entity, when it updated or changed. */
    private static void refresh(ClientLevel level, Copy copy) {
        if (!copy.updated && !(copy.changed && now - copy.loadedAt >= REFRESH_TICKS)) return;
        CompoundTag tag = updateTag(copy.server, level, copy.updated);
        copy.updated = false;
        copy.changed = false;
        copy.loadedAt = now;
        if (tag != null) load(level, copy.client, tag);
    }

    /** The client's block entity as a chunk brings it: the type's, loaded from the update tag. */
    private static BlockEntity make(ClientLevel level, BlockEntity server) {
        try {
            BlockEntity made = server.getType().create(server.getBlockPos(), server.getBlockState());
            if (made == null) return null;
            made.setLevel(level);
            CompoundTag tag = updateTag(server, level, false);
            if (tag != null) load(level, made, tag);
            return made;
        } catch (Throwable t) {
            Natives.log("Horizon's client: no client copy of the block entity at " + server.getBlockPos() + ": " + t);
            return null;
        }
    }

    /** What the server sends: the update packet's tag (an update), else the chunk's update tag. */
    private static CompoundTag updateTag(BlockEntity server, ClientLevel level, boolean packet) {
        try {
            if (packet && server.getUpdatePacket() instanceof ClientboundBlockEntityDataPacket data) return data.getTag();
            return server.getUpdateTag(level.registryAccess());
        } catch (Throwable t) {
            Natives.log("Horizon's client: the block entity at " + server.getBlockPos() + " gave no update: " + t);
            return null;
        }
    }

    private static void load(ClientLevel level, BlockEntity be, CompoundTag tag) {
        try {
            be.loadWithComponents(TagValueInput.create(ProblemReporter.DISCARDING, level.registryAccess(), tag));
        } catch (Throwable t) {
            Natives.log("Horizon's client: the client's block entity at " + be.getBlockPos() + " couldn't load its update: " + t);
        }
    }
}
