package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.util.Map;
import java.util.Set;
import net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking;
import net.fabricmc.fabric.api.networking.v1.PacketSender;
import net.fabricmc.fabric.api.networking.v1.ServerPlayNetworking;
import net.fabricmc.fabric.impl.networking.client.ClientNetworkingImpl;
import net.fabricmc.fabric.impl.networking.server.ServerNetworkingImpl;
import net.minecraft.client.Minecraft;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ClientboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.ServerboundCustomPayloadPacket;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.resources.Identifier;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerPlayer;

/**
 * The mods' own messages between their two halves, both in this JVM: what a mod's server code
 * sends a player (ServerPlayNetworking.send) reaches its client code's receiver for that
 * player's headless client, and what its client code sends (ClientPlayNetworking.send) reaches
 * its server receiver as that player's. Payloads are written and read back with their codecs
 * (carried), as over a connection; one whose codec fails here goes over as the object.
 *
 * <p>Fabric runs play receivers on the receiving side's main thread; both sides' are Horizon's
 * server thread here. Server messages wait for the end of what sent them (flush), as packets
 * do; client messages run at once, as the server reads them next.
 */
public final class HorizonLoop {
    private HorizonLoop() {}

    /** Whether the loop is up: the headless client runs (the game's own JVM, not a server's). */
    public static boolean on() {
        return HorizonClient.headless();
    }

    /** The channels the mods' client code receives on: what a server may send a player. */
    public static Set<Identifier> clientChannels() {
        return ClientNetworkingImpl.PLAY.getChannels();
    }

    /**
     * Receivers a mod registered for one player (ServerPlayNetworking.registerReceiver with a
     * listener, often in ServerPlayConnectionEvents.INIT). Fabric keeps them in the listener's
     * network addon, made by its constructor: the mirror's listener has none, so they live here.
     */
    private static final Map<java.util.UUID, Map<Identifier, ServerPlayNetworking.PlayPayloadHandler<?>>> PLAYER_RECEIVERS =
            new java.util.concurrent.ConcurrentHashMap<>();

    /** ServerPlayNetworking.registerReceiver(listener, ...) for a mirrored player. */
    public static boolean registerReceiver(ServerPlayer player, Identifier id, ServerPlayNetworking.PlayPayloadHandler<?> handler) {
        return PLAYER_RECEIVERS.computeIfAbsent(player.getUUID(), u -> new java.util.concurrent.ConcurrentHashMap<>())
                .putIfAbsent(id, handler) == null;
    }

    /** ServerPlayNetworking.unregisterReceiver(listener, id) for a mirrored player. */
    public static ServerPlayNetworking.PlayPayloadHandler<?> unregisterReceiver(ServerPlayer player, Identifier id) {
        var own = PLAYER_RECEIVERS.get(player.getUUID());
        return own == null ? null : own.remove(id);
    }

    /** The channels the mods' server code receives on: what a client may send. */
    public static Set<Identifier> serverChannels() {
        return ServerNetworkingImpl.PLAY.getChannels();
    }

    /**
     * What the server sent the clients, in order, for after the server's changes show: as a
     * vanilla client reads its packets once they arrive, after the ones before (a menu's
     * opening, then its data: the screen is up when the data comes).
     */
    private static final java.util.Queue<Runnable> OUTBOX = new java.util.concurrent.ConcurrentLinkedQueue<>();

    /**
     * Delivers what the server sent (HorizonServer.syncInventories' end): what was queued when
     * it starts. What a delivery makes the server send waits for the next tick, as over a
     * network: the headless client's level reads the server's block entities, so a client
     * handler's change can send the same message again (Large Fluid Tank's content message set
     * the server's tank, which sent its contents again: the server thread never left this).
     */
    public static void flush() {
        // A client's message being read now (toServer runs the server's receiver at once): its
        // answer reaches the client after the client's code that sent it, as over a network
        // (Simple Voice Chat makes its client right after asking for its secret: the secret,
        // delivered inside the ask, found none).
        if (reading > 0) return;
        loopThread = Thread.currentThread();
        // Clients' messages sent off this thread (REI's reload threads: Advanced Loot Info asks
        // for its loot data there) reach the server here, as a real server reads its network.
        for (int left = INBOX.size(); left > 0; left--) {
            Runnable request = INBOX.poll();
            if (request == null) break;
            request.run();
        }
        for (int left = OUTBOX.size(); left > 0; left--) {
            Runnable delivery = OUTBOX.poll();
            if (delivery == null) break;
            delivery.run();
        }
    }

    /** Server to client: whether a mod's client code receives it (it is then delivered at flush). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    /** HORIZON_JAVA_DEBUG=loop: each message between the two halves is logged. */
    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("loop");

    public static boolean toClient(ServerPlayer player, CustomPacketPayload payload) {
        if (!on()) return false;
        if (DEBUG) Natives.log("loop: server to client " + payload.type().id() + " for " + player.getName().getString());
        // A device mod's message for a player whose client half isn't here: to their own game,
        // which runs it (HorizonDeviceMods); none for a game that doesn't.
        if (HorizonDeviceMods.deviceChannel(payload.type().id()) && !player.getUUID().equals(HorizonDeviceMods.own)) {
            byte[] body = null;
            if (HorizonDeviceMods.RUN_THEIR_OWN.contains(player.getUUID()) || HorizonDeviceMods.OWN_CLIENT) {
                body = write(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY,
                        HorizonServer.server().registryAccess());
            }
            if (body == null) return true;
            if (HorizonDeviceMods.RUN_THEIR_OWN.contains(player.getUUID())) {
                HorizonPackets.customPayload(player, payload.type().id().toString(), body);
            } else {
                // Every mod's client half in each game (OWN_CLIENT): the player's game starts its
                // own client a little after it joins; what the server sends before waits for it,
                // as a connection would carry it (the mods' join data: REI's display sync).
                var waiting = WAITING.computeIfAbsent(player.getUUID(), u -> new java.util.ArrayList<>());
                synchronized (waiting) {
                    if (waiting.size() < WAITING_MOST) waiting.add(new Object[] {payload.type().id().toString(), body});
                }
            }
            return true;
        }
        // The player's connection's own receivers first (ClientPlayNetworking.registerReceiver
        // while in game: ShulkerBoxTooltip's, at the connection's INIT), then the global ones.
        var own = (ClientPlayNetworking.PlayPayloadHandler) HorizonClient.connectionReceiver(player, payload.type().id());
        var handler = own != null ? own : (ClientPlayNetworking.PlayPayloadHandler) ClientNetworkingImpl.PLAY.getHandler(payload.type().id());
        if (handler == null) return false;
        CustomPacketPayload received = carried(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY);
        Runnable delivery = () -> {
            try {
                // What the handler schedules on the client's thread (Minecraft.schedule: corelib's
                // queued packets, Chunk Loaders' loaders) runs now, still as this player: in
                // their client level, where the packet is for.
                HorizonClient.as(player, () -> {
                    HorizonClient.joinBeforeMessages(player);
                    handler.receive(received, clientContext(player));
                    HorizonClient.tasks();
                });
            } catch (Throwable t) {
                log("the client of " + payload.type().id() + " failed", t);
            }
        };
        OUTBOX.add(delivery);
        return true;
    }

    /**
     * Server to client, a mod's own packet type (a Packet of its own class in the play
     * protocol: Create's OpenScreenPacket for its toolbox and other menus): handled by the
     * player's client connection, delivered at flush as payloads are. False with no client here.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static boolean toClientPacket(ServerPlayer player, net.minecraft.network.protocol.Packet<?> packet) {
        if (!on()) return false;
        if (DEBUG) Natives.log("loop: server to client packet " + packet.getClass().getName() + " for " + player.getName().getString());
        OUTBOX.add(() -> {
            try {
                HorizonClient.as(player, () -> {
                    HorizonClient.joinBeforeMessages(player);
                    var connection = net.minecraft.client.Minecraft.getInstance().getConnection();
                    if (connection != null) ((net.minecraft.network.protocol.Packet) packet).handle(connection);
                    HorizonClient.tasks();
                });
            } catch (Throwable t) {
                log("the client of " + packet.getClass().getName() + " failed", t);
            }
        });
        return true;
    }

    /** A payload as its codec writes it (a custom payload's body), or null when it can't be. */
    @SuppressWarnings({"unchecked", "rawtypes"})
    static byte[] write(CustomPacketPayload payload,
                        net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl<net.minecraft.network.RegistryFriendlyByteBuf> registry,
                        net.minecraft.core.RegistryAccess access) {
        var type = registry.get(payload.type().id());
        if (type == null) return null;
        var buf = new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(), access);
        try {
            ((net.minecraft.network.codec.StreamCodec) type.codec()).encode(buf, payload);
            byte[] out = new byte[buf.readableBytes()];
            buf.readBytes(out);
            return out;
        } catch (Throwable t) {
            Natives.log("Java mods: " + payload.type().id() + " couldn't be written: " + t);
            return null;
        } finally {
            buf.release();
        }
    }

    /** A payload read from its body by its registered codec, or null. */
    static CustomPacketPayload read(String channel, byte[] body,
                                    net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl<net.minecraft.network.RegistryFriendlyByteBuf> registry,
                                    net.minecraft.core.RegistryAccess access) {
        var type = registry.get(Identifier.parse(channel));
        if (type == null) return null;
        var buf = new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.wrappedBuffer(body), access);
        try {
            Object read = type.codec().decode(buf);
            return read instanceof CustomPacketPayload p ? p : null;
        } catch (Throwable t) {
            Natives.log("Java mods: " + channel + " couldn't be read: " + t);
            return null;
        } finally {
            buf.release();
        }
    }

    /**
     * A message a player's own game sent on a device mod's channel (its client half runs
     * there: HorizonDeviceMods), to the mod's server code as that player's. On the server thread.
     */
    public static void fromGame(String uuid, String channel, byte[] body) {
        ServerPlayer player = HorizonServer.player(java.util.UUID.fromString(uuid));
        var server = HorizonServer.server();
        if (player == null || server == null || !HorizonDeviceMods.deviceChannel(Identifier.parse(channel))) return;
        CustomPacketPayload payload = read(channel, body, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.SERVERBOUND_PLAY,
                server.registryAccess());
        if (DEBUG) Natives.log("loop: " + player.getName().getString() + "'s game to server " + channel);
        if (payload != null) receive(player, payload);
    }

    /** A player's own game runs the device mods' client half (it said so as it joined). */
    public static void runsTheirOwn(String uuid) {
        var id = java.util.UUID.fromString(uuid);
        HorizonDeviceMods.RUN_THEIR_OWN.add(id);
        // What waited for its client, in order.
        var waiting = WAITING.remove(id);
        ServerPlayer player = HorizonServer.player(id);
        if (waiting == null || player == null) return;
        synchronized (waiting) {
            for (Object[] message : waiting) HorizonPackets.customPayload(player, (String) message[0], (byte[]) message[1]);
        }
        if (DEBUG) Natives.log("loop: " + waiting.size() + " message(s) waited for " + player.getName().getString() + "'s own client");
    }

    /** Mods' messages for a player whose game hasn't started its own client yet (OWN_CLIENT). */
    private static final Map<java.util.UUID, java.util.List<Object[]>> WAITING = new java.util.concurrent.ConcurrentHashMap<>();
    private static final int WAITING_MOST = 512;

    /** A player left: what waited for their client goes. */
    static void forget(java.util.UUID id) {
        WAITING.remove(id);
    }

    /** The player whose game this process is (null: none, a dedicated server). */
    public static void ownPlayer(String uuid) {
        HorizonDeviceMods.own = uuid == null || uuid.isEmpty() ? null : java.util.UUID.fromString(uuid);
        // What waited for a client of its own before the server knew it is this game's player:
        // this process's client is theirs.
        var waiting = HorizonDeviceMods.own == null ? null : WAITING.remove(HorizonDeviceMods.own);
        ServerPlayer player = waiting == null ? null : HorizonServer.player(HorizonDeviceMods.own);
        if (player != null) {
            synchronized (waiting) {
                for (Object[] message : waiting) {
                    var payload = read((String) message[0], (byte[]) message[1],
                            net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY, HorizonServer.server().registryAccess());
                    if (payload != null) toClient(player, payload);
                }
            }
        }
        if (!HorizonDeviceMods.mods().isEmpty()) Natives.log("Java mods: this game's own player: " + HorizonDeviceMods.own);
    }

    /** Payload types whose codec failed here: theirs go over as objects. */
    private static final Set<Identifier> UNCARRIED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /**
     * The payload as the other side reads it: written and read back with its registered codec,
     * as over a connection. Most payloads read back as the same object, but some mods' are one
     * class for both ways with fields only one side fills (Pufferfish's Skills: what the server
     * writes, its client reads into another field, null when the object went over as it was).
     * A codec that fails here (none registered, a registry it can't reach) leaves the object.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static CustomPacketPayload carried(CustomPacketPayload payload,
                                               net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl<net.minecraft.network.RegistryFriendlyByteBuf> registry) {
        Identifier id = payload.type().id();
        if (UNCARRIED.contains(id)) return payload;
        var server = HorizonServer.server();
        var type = registry.get(id);
        if (type == null || server == null) return payload;
        var buf = new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(), server.registryAccess());
        try {
            ((net.minecraft.network.codec.StreamCodec) type.codec()).encode(buf, payload);
            Object read = type.codec().decode(buf);
            if (read instanceof CustomPacketPayload back) return back;
        } catch (Throwable t) {
            if (UNCARRIED.add(id) && DEBUG) Natives.log("loop: " + id + " goes over as it is (its codec: " + t + ")");
        } finally {
            buf.release();
        }
        UNCARRIED.add(id);
        return payload;
    }

    /** Client messages sent off the loop's thread, run at the next flush (on the loop's thread). */
    private static final java.util.Queue<Runnable> INBOX = new java.util.concurrent.ConcurrentLinkedQueue<>();
    /** The thread that runs the mods' server and clients (the one flush runs on). */
    private static volatile Thread loopThread;

    /**
     * What waits on the loop's thread for a mod's own thread (Horizon's REI reload) runs the
     * messages between the halves meanwhile, so a client thread waiting for its server's
     * answer (Advanced Loot Info's plugin) gets it instead of timing out.
     */
    public static void pump() {
        if (Thread.currentThread() == loopThread && (!INBOX.isEmpty() || !OUTBOX.isEmpty())) {
            flush();
        }
    }

    /** Client to server, for the player whose client runs now (Minecraft.player). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void toServer(CustomPacketPayload payload) {
        // A game connected to another's server: the device mods' messages go to it.
        if (HorizonDeviceClient.on() || HorizonDeviceClient.own()) {
            HorizonDeviceClient.send(payload);
            return;
        }
        ServerPlayer player = HorizonClient.serverPlayer(Minecraft.getInstance().player);
        boolean elsewhere = loopThread != null && Thread.currentThread() != loopThread;
        // A client thread of a mod's own (REI's reload) sends with no player's client running:
        // its client is the one whose client ran last (the only one in single player).
        if (player == null && elsewhere) player = HorizonClient.lastPlayer();
        if (DEBUG) Natives.log("loop: client to server " + payload.type().id() + " from " + (player == null ? null : player.getName().getString())
                + (elsewhere ? " (off the loop's thread: queued)" : ""));
        if (player == null) {
            Natives.log("Java mods: " + payload.type().id() + " was sent with no player's client running; dropped");
            return;
        }
        if (elsewhere) {
            ServerPlayer from = player;
            INBOX.add(() -> receive(from, payload));
            return;
        }
        reading++;
        try {
            receive(player, payload);
        } finally {
            reading--;
        }
    }

    /**
     * Client to server, a mod's own packet type (Create's ValueSettingsPacket: a value box's
     * new setting): handled by the sending player's server connection, at once on the loop's
     * thread (queued from another), as payloads are.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void toServerPacket(net.minecraft.network.protocol.Packet<?> packet) {
        ServerPlayer player = HorizonClient.serverPlayer(Minecraft.getInstance().player);
        boolean elsewhere = loopThread != null && Thread.currentThread() != loopThread;
        if (player == null && elsewhere) player = HorizonClient.lastPlayer();
        if (DEBUG) Natives.log("loop: client to server packet " + packet.getClass().getName() + " from "
                + (player == null ? null : player.getName().getString()));
        if (player == null || player.connection == null) return;
        ServerPlayer from = player;
        Runnable run = () -> {
            try {
                // The mirror's server is made without its constructor: its packet processor
                // (handlers check they run on its thread) is this thread's.
                var server = from.level().getServer();
                if (server != null && server.packetProcessor() == null) {
                    HorizonClient.set(net.minecraft.server.MinecraftServer.class, server, "packetProcessor",
                            new net.minecraft.network.PacketProcessor(Thread.currentThread()));
                }
                ((net.minecraft.network.protocol.Packet) packet).handle(from.connection);
            } catch (net.minecraft.server.RunningOnDifferentThreadException e) {
                // Queued on the server's processor: it runs at the server's tick.
            } catch (Throwable t) {
                log("the server of " + packet.getClass().getName() + " failed", t);
            }
        };
        if (elsewhere) INBOX.add(run);
        else run.run();
    }

    /** Client messages read now, inside the client code that sent them (on the loop's thread). */
    private static int reading;

    @SuppressWarnings({"unchecked", "rawtypes"})
    private static void receive(ServerPlayer player, CustomPacketPayload payload) {
        // Queued by a player who has left since: no one to answer.
        if (HorizonServer.player(player.getUUID()) == null) return;
        var own = PLAYER_RECEIVERS.getOrDefault(player.getUUID(), Map.of()).get(payload.type().id());
        ServerPlayNetworking.PlayPayloadHandler handler = own != null ? own
                : (ServerPlayNetworking.PlayPayloadHandler) ServerNetworkingImpl.PLAY.getHandler(payload.type().id());
        if (handler == null) return;
        try {
            handler.receive(carried(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.SERVERBOUND_PLAY), serverContext(player));
            // What the mod's server code changed shows as the server's other changes do.
            HorizonServer.afterLoop();
        } catch (Throwable t) {
            log("the server side of " + payload.type().id() + " failed", t);
        }
    }

    private static ClientPlayNetworking.Context clientContext(ServerPlayer player) {
        return new ClientPlayNetworking.Context() {
            @Override
            public Minecraft client() {
                return Minecraft.getInstance();
            }

            @Override
            public LocalPlayer player() {
                return Minecraft.getInstance().player;
            }

            @Override
            public PacketSender responseSender() {
                return clientSender();
            }
        };
    }

    private static ServerPlayNetworking.Context serverContext(ServerPlayer player) {
        return new ServerPlayNetworking.Context() {
            @Override
            public MinecraftServer server() {
                return player.level().getServer();
            }

            @Override
            public ServerPlayer player() {
                return player;
            }

            @Override
            public PacketSender responseSender() {
                return serverSender(player);
            }
        };
    }

    /** The client's sender (ClientPlayNetworking.getSender, a context's response sender). */
    public static PacketSender clientSender() {
        return new Sender() {
            @Override
            public Packet<?> createPacket(CustomPacketPayload payload) {
                return new ServerboundCustomPayloadPacket(payload);
            }

            @Override
            void send(CustomPacketPayload payload) {
                toServer(payload);
            }
        };
    }

    /** A player's server sender (ServerPlayNetworking.getSender, a context's response sender). */
    public static PacketSender serverSender(ServerPlayer player) {
        return new Sender() {
            @Override
            public Packet<?> createPacket(CustomPacketPayload payload) {
                return new ClientboundCustomPayloadPacket(payload);
            }

            @Override
            void send(CustomPacketPayload payload) {
                HorizonServer.payload(player, payload);
            }
        };
    }

    /** A sender whose packets are custom payloads, delivered in this JVM. */
    private abstract static class Sender implements PacketSender {
        abstract void send(CustomPacketPayload payload);

        @Override
        public void sendPacket(Packet<?> packet, io.netty.channel.ChannelFutureListener callback) {
            if (packet instanceof ServerboundCustomPayloadPacket(CustomPacketPayload payload)) send(payload);
            else if (packet instanceof ClientboundCustomPayloadPacket(CustomPacketPayload payload)) send(payload);
            else HorizonServer.dropped(packet);
        }

        @Override
        public void sendPacket(CustomPacketPayload payload) {
            send(payload);
        }

        @Override
        public void disconnect(Component reason) {
            Natives.log("Java mods: a mod asked to disconnect a player: " + reason.getString());
        }
    }

    private static void log(String what, Throwable t) {
        Throwable root = t;
        while (root.getCause() != null && root.getCause() != root) root = root.getCause();
        Natives.log("Java mods: " + what + ": " + root);
        int n = 0;
        for (StackTraceElement frame : root.getStackTrace()) {
            if (n++ == 12) break;
            Natives.log("    at " + frame);
        }
    }
}
