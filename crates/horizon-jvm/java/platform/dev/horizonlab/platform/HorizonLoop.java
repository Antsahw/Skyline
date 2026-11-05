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
            if (HorizonDeviceMods.RUN_THEIR_OWN.contains(player.getUUID())) {
                byte[] body = write(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY,
                        HorizonServer.server().registryAccess());
                if (body != null) HorizonPackets.customPayload(player, payload.type().id().toString(), body);
            }
            return true;
        }
        var handler = (ClientPlayNetworking.PlayPayloadHandler) ClientNetworkingImpl.PLAY.getHandler(payload.type().id());
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
        HorizonDeviceMods.RUN_THEIR_OWN.add(java.util.UUID.fromString(uuid));
    }

    /** The player whose game this process is (null: none, a dedicated server). */
    public static void ownPlayer(String uuid) {
        HorizonDeviceMods.own = uuid == null || uuid.isEmpty() ? null : java.util.UUID.fromString(uuid);
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
        if (HorizonDeviceClient.on()) {
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

    // ---- Mods' own game packets ------------------------------------------------------------

    /**
     * Whether a packet is one of a mod's own packet types in the game protocol: not a custom
     * payload, but a packet class with its own handle(listener), registered into the play
     * protocol by the mod (a mixin on GameProtocols). The bridge has no connection to carry
     * them, so they go between the two halves here, as payloads do.
     */
    public static boolean modPacket(Packet<?> packet) {
        String name = packet.getClass().getName();
        return !name.startsWith("net.minecraft.") && !name.startsWith("net.fabricmc.")
                && !name.startsWith("dev.horizonlab.") && !name.startsWith("com.mojang.");
    }

    /**
     * Client to server: a mod's own serverbound packet sent by the client running now
     * (LocalPlayer.connection.send: a mod's client code after a click, as a selection's end),
     * handled by its server listener, the player's mirrored connection, as the server reads it.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void packetToServer(Packet<?> packet) {
        if (packet.type().flow() != net.minecraft.network.protocol.PacketFlow.SERVERBOUND) return;
        ServerPlayer player = HorizonClient.serverPlayer(Minecraft.getInstance().player);
        boolean elsewhere = loopThread != null && Thread.currentThread() != loopThread;
        if (player == null && elsewhere) player = HorizonClient.lastPlayer();
        if (DEBUG) Natives.log("loop: client to server packet " + packet.type().id() + " from "
                + (player == null ? null : player.getName().getString()));
        if (player == null || player.connection == null) {
            Natives.log("Java mods: " + packet.type().id() + " was sent with no player's client running; dropped");
            return;
        }
        ServerPlayer from = player;
        Packet carried = carriedPacket(packet);
        Runnable read = () -> {
            if (HorizonServer.player(from.getUUID()) == null) return;
            ensureProcessors();
            try {
                carried.handle(from.connection);
                HorizonServer.afterLoop();
            } catch (net.minecraft.server.RunningOnDifferentThreadException ignored) {
                // Queued on the server (the mirror's thread check): it runs with its tasks.
            } catch (Throwable t) {
                log("the server side of " + packet.type().id() + " failed", t);
            }
        };
        if (elsewhere) {
            INBOX.add(read);
            return;
        }
        reading++;
        try {
            read.run();
        } finally {
            reading--;
        }
    }

    /**
     * Server to client: a mod's own clientbound packet a mirrored player's connection was sent
     * (the server asking the client for what it selected), handled by that player's headless
     * client connection at flush, as a payload is. Whether it was taken.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static boolean packetToClient(ServerPlayer player, Packet<?> packet) {
        if (!on() || packet.type().flow() != net.minecraft.network.protocol.PacketFlow.CLIENTBOUND) return false;
        if (DEBUG) Natives.log("loop: server to client packet " + packet.type().id() + " for " + player.getName().getString());
        Packet carried = carriedPacket(packet);
        OUTBOX.add(() -> {
            try {
                HorizonClient.as(player, () -> {
                    HorizonClient.joinBeforeMessages(player);
                    var connection = Minecraft.getInstance().getConnection();
                    if (connection == null) return;
                    ensureProcessors();
                    try {
                        carried.handle(connection);
                    } catch (net.minecraft.server.RunningOnDifferentThreadException ignored) {
                        // Scheduled on the client: run below, still as this player.
                    }
                    HorizonClient.tasks();
                });
            } catch (Throwable t) {
                log("the client of " + packet.type().id() + " failed", t);
            }
        });
        return true;
    }

    /**
     * The client's and the server's packet processors (Minecraft's and MinecraftServer's
     * packetProcessor, made in their constructors, which the mirror and the headless client
     * skip): a mod's packet handler checks its thread with them
     * (PacketUtils.ensureRunningOnSameThread). Both run on the loop's thread, Horizon's server
     * thread, as single player's halves each run on their own.
     */
    private static void ensureProcessors() {
        try {
            var minecraft = Minecraft.getInstance();
            if (minecraft != null && minecraft.packetProcessor() == null) {
                HorizonClient.set(Minecraft.class, minecraft, "packetProcessor",
                        new net.minecraft.network.PacketProcessor(Thread.currentThread()));
            }
            var server = HorizonServer.server();
            if (server != null && server.packetProcessor() == null) {
                HorizonClient.set(MinecraftServer.class, server, "packetProcessor",
                        new net.minecraft.network.PacketProcessor(Thread.currentThread()));
            }
        } catch (ReflectiveOperationException e) {
            Natives.log("Java mods: no packet processors: " + e);
        }
    }

    /** Packet classes whose codec couldn't carry them: they go over as the object. */
    private static final Set<Class<?>> UNCARRIED_PACKETS = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /**
     * A mod's packet written and read back with its codec (a static StreamCodec field of its
     * class, as packet types declare theirs), so the two halves don't share its objects, as
     * over a connection; the packet itself when there is none or it fails.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static Packet<?> carriedPacket(Packet<?> packet) {
        Class<?> type = packet.getClass();
        var server = HorizonServer.server();
        if (server == null || UNCARRIED_PACKETS.contains(type)) return packet;
        net.minecraft.network.codec.StreamCodec codec = null;
        for (var field : type.getDeclaredFields()) {
            if (!java.lang.reflect.Modifier.isStatic(field.getModifiers())
                    || !net.minecraft.network.codec.StreamCodec.class.isAssignableFrom(field.getType())) continue;
            try {
                field.setAccessible(true);
                codec = (net.minecraft.network.codec.StreamCodec) field.get(null);
                break;
            } catch (Throwable ignored) {
            }
        }
        if (codec == null) {
            UNCARRIED_PACKETS.add(type);
            return packet;
        }
        var buf = new net.minecraft.network.RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(), server.registryAccess());
        try {
            codec.encode(buf, packet);
            if (codec.decode(buf) instanceof Packet<?> back && back.getClass() == type) return back;
        } catch (Throwable t) {
            if (DEBUG) Natives.log("loop: " + type.getName() + " goes over as it is (its codec: " + t + ")");
        } finally {
            buf.release();
        }
        UNCARRIED_PACKETS.add(type);
        return packet;
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
