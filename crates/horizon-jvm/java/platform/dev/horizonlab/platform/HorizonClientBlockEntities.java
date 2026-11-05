package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.protocol.game.ClientboundBlockEntityDataPacket;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.entity.BlockEntityTicker;
import net.minecraft.world.level.block.state.BlockState;

/**
 * The client's copies of the mods' block entities, as a vanilla client keeps its own: made from
 * the server's update packet (getUpdatePacket's tag, loaded through
 * ClientPacketListener.handleBlockEntityData's own code, so the mods' wraps of it run: Create's
 * onDataPacket), loaded again when the server sends its block update (sendBlockUpdated), and
 * ticked with their client tickers. Their renderers draw these: what a mod keeps only on the
 * client copy (Create's client behaviours: the mixer's and press's head animations, built in
 * SmartBlockEntity.initialize when its level is a client's) is there. A copy that can't be made
 * leaves the server's to its renderer, as before.
 */
public final class HorizonClientBlockEntities {
    private HorizonClientBlockEntities() {}

    private static final class Copy {
        final BlockEntity be;
        final BlockEntity server;
        /** The server sent a block update since it was loaded. */
        boolean stale;
        /** The client tick it was last drawn at (copies no one draws go). */
        long used;
        /** The client tick it last ticked at (a level's players share its copies). */
        long lastTicked = -1;

        Copy(BlockEntity be, BlockEntity server) {
            this.be = be;
            this.server = server;
        }
    }

    private static final Map<Level, Map<Long, Copy>> COPIES = new IdentityHashMap<>();
    /** Types whose copies failed: drawn from the server's (logged once). */
    private static final Set<Object> NO_COPY = ConcurrentHashMap.newKeySet();
    /** Types whose client ticker failed too often: not ticked again. */
    private static final Map<Object, Integer> TICK_FAILURES = new HashMap<>();
    private static final boolean OFF = "0".equals(System.getenv("HORIZON_JAVA_CLIENT_BE"));
    private static Method load;
    private static long ticks;

    /** The client's copy of a mod's block entity for its renderer: the server's when there is none. */
    static synchronized BlockEntity of(BlockEntity server) {
        if (OFF || server.getLevel() == null || NO_COPY.contains(server.getType())) return server;
        var copies = COPIES.computeIfAbsent(server.getLevel(), l -> new HashMap<>());
        long key = server.getBlockPos().asLong();
        Copy copy = copies.get(key);
        if (copy != null && (copy.server != server || copy.be.getType() != server.getType())) {
            copy.be.setRemoved();
            copies.remove(key);
            copy = null;
        }
        if (copy == null) {
            try {
                BlockEntity be = server.getType().create(server.getBlockPos(), server.getBlockState());
                if (be == null) throw new IllegalStateException("its type made none");
                be.setLevel(HorizonClient.clientLevel(server.getLevel()));
                load(be, server);
                copy = new Copy(be, server);
                copies.put(key, copy);
                // Its first client tick before its first frame (mods set their client state up
                // there: Create's SmartBlockEntity.initialize adds the mixer's animation).
                tickOne(HorizonClient.clientLevel(server.getLevel()), copy);
            } catch (Throwable t) {
                NO_COPY.add(server.getType());
                Natives.log("Java mods: no client copy of " + BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(server.getType())
                        + " (its renderer draws the server's): " + t);
                return server;
            }
        } else {
            if (copy.be.getBlockState() != server.getBlockState()) {
                copy.be.setBlockState(server.getBlockState());
                copy.stale = true;
            }
            if (copy.stale) {
                copy.stale = false;
                try {
                    load(copy.be, server);
                } catch (Throwable t) {
                    if (TICK_FAILURES.merge("load " + server.getType(), 1, Integer::sum) <= 3) {
                        Natives.log("Java mods: a client copy of " + BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(server.getType())
                                + " couldn't be updated: " + t);
                    }
                }
            }
        }
        copy.used = ticks;
        return copy.be;
    }

    /**
     * The server's block update reached the clients (ServerLevel.sendBlockUpdated): the copy
     * there loads the server's update packet again before it is next drawn.
     */
    public static synchronized void updated(ServerLevel level, BlockPos pos) {
        var copies = COPIES.get(level);
        Copy copy = copies == null ? null : copies.get(pos.asLong());
        if (copy != null) copy.stale = true;
    }

    /** What the client does with a block entity's update packet (handleBlockEntityData's load). */
    private static void load(BlockEntity be, BlockEntity server) throws ReflectiveOperationException {
        var packet = server.getUpdatePacket() instanceof ClientboundBlockEntityDataPacket p ? p
                : ClientboundBlockEntityDataPacket.create(server);
        var listener = HorizonClient.minecraft().getConnection();
        if (listener != null) {
            if (load == null) {
                load = net.minecraft.client.multiplayer.ClientPacketListener.class.getDeclaredMethod(
                        "lambda$handleBlockEntityData$0", ClientboundBlockEntityDataPacket.class, BlockEntity.class);
                load.setAccessible(true);
            }
            try {
                load.invoke(listener, packet, be);
                return;
            } catch (java.lang.reflect.InvocationTargetException e) {
                if (e.getCause() instanceof RuntimeException r) throw r;
                throw e;
            }
        }
        be.loadWithComponents(net.minecraft.world.level.storage.TagValueInput.create(
                net.minecraft.util.ProblemReporter.DISCARDING, server.getLevel().registryAccess(), packet.getTag()));
    }

    /** A new client tick (HorizonClient.clientTick). */
    static synchronized void newTick() {
        ticks++;
    }

    /**
     * One client tick of a level's copies (each level once a tick, as its first player's client
     * stands there): their client tickers run; copies not drawn for 5 s, or whose block is
     * gone, go.
     */
    static synchronized void tick(Level server) {
        var copies = COPIES.get(server);
        if (copies == null) return;
        ClientLevel client = HorizonClient.clientLevel(server);
        for (var entry : new ArrayList<>(copies.entrySet())) {
            Copy copy = entry.getValue();
            if (copy.server.isRemoved() || ticks - copy.used > 100) {
                copy.be.setRemoved();
                copies.remove(entry.getKey());
                continue;
            }
            tickOne(client, copy);
        }
    }

    /** One client tick of a copy (once a client tick, its client ticker's). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static void tickOne(ClientLevel client, Copy copy) {
        if (copy.lastTicked == ticks) return;
        copy.lastTicked = ticks;
        Object type = copy.be.getType();
        Integer failed = TICK_FAILURES.get(type);
        if (failed != null && failed > 20) return;
        try {
            BlockState state = copy.be.getBlockState();
            BlockEntityTicker ticker = state.getTicker(client, copy.be.getType());
            if (ticker != null) ticker.tick(client, copy.be.getBlockPos(), state, copy.be);
        } catch (Throwable t) {
            int n = TICK_FAILURES.merge(type, 1, Integer::sum);
            if (n <= 2) {
                Throwable root = t;
                while (root.getCause() != null && root.getCause() != root) root = root.getCause();
                Natives.log("Java mods: the client tick of " + BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(copy.be.getType())
                        + " failed: " + root);
                StackTraceElement[] frames = root.getStackTrace();
                for (int f = 0; f < Math.min(frames.length, 8); f++) Natives.log("    at " + frames[f]);
            }
        }
    }
}
