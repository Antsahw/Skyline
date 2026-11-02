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

    /** Client to server, for the player whose client runs now (Minecraft.player). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void toServer(CustomPacketPayload payload) {
        ServerPlayer player = HorizonClient.serverPlayer(Minecraft.getInstance().player);
        if (DEBUG) Natives.log("loop: client to server " + payload.type().id() + " from " + (player == null ? null : player.getName().getString()));
        if (player == null) {
            Natives.log("Java mods: " + payload.type().id() + " was sent with no player's client running; dropped");
            return;
        }
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
