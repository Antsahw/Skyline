package dev.horizonlab.platform;

import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import dev.horizonlab.bridge.Natives;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.player.AbstractClientPlayer;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.player.RemotePlayer;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.common.custom.CustomPacketPayload;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.phys.Vec3;

/**
 * The device mods' client half in a game connected to another's server (a LAN guest, a
 * dedicated server's player): HorizonDeviceMods. That game's own JVM is otherwise idle (its
 * mirror runs only for a world of its own); here it is the player's client, for those mods only,
 * as a real client is for its mods: a client player and level (the player and the others Horizon's
 * game knows of, where they are), a connection whose address is the server's, the mods' messages
 * over Horizon's connection, the client's joins, ticks and frames. Their sound devices are the
 * player's own. Everything runs on the game's thread (Rust calls join, tick, payload, leave).
 */
public final class HorizonDeviceClient {
    private HorizonDeviceClient() {}

    private static volatile boolean on;
    private static LocalPlayer local;
    private static ClientLevel level;
    private static ClientPacketListener connection;
    private static final Map<UUID, RemotePlayer> OTHERS = new ConcurrentHashMap<>();
    private static net.minecraft.core.RegistryAccess registries;

    /** Whether this process is a device client now (its game is in another's world). */
    public static boolean on() {
        return on;
    }

    /**
     * The game joined another's server at {@code host}:{@code port} as {@code uuid}: the device
     * mods' client joins. Returns the device mods' ids (comma-separated) for the game to tell the
     * server, or "" when there are none (nothing is started).
     */
    public static String join(String host, int port, String uuid, String name) {
        var mods = HorizonDeviceMods.mods();
        if (mods.isEmpty() || !HorizonClient.headless()) return "";
        if (on) leave();
        Thread.currentThread().setContextClassLoader(HorizonDeviceClient.class.getClassLoader());
        Minecraft mc = Minecraft.getInstance();
        try {
            registries = net.minecraft.core.RegistryAccess.fromRegistryOfRegistries(net.minecraft.core.registries.BuiltInRegistries.REGISTRY);
            UUID id = UUID.fromString(uuid);
            connection = (ClientPacketListener) Natives.allocate(ClientPacketListener.class);
            HorizonClient.set(ClientPacketListener.class, connection, "registryAccess", registries);
            HorizonClient.set(ClientPacketListener.class, connection, "enabledFeatures", net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS);
            Class<?> common = net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl.class;
            HorizonClient.set(common, connection, "minecraft", mc);
            HorizonClient.set(common, connection, "connection", serverConnection(host, port));
            HorizonClient.set(common, connection, "serverData", new net.minecraft.client.multiplayer.ServerData(
                    host, host + ":" + port, net.minecraft.client.multiplayer.ServerData.Type.OTHER));
            HorizonClient.players(connection);
            level = (ClientLevel) Natives.allocate(ClientLevel.class);
            HorizonClient.set(ClientLevel.class, level, "players", new java.util.concurrent.CopyOnWriteArrayList<AbstractClientPlayer>());
            HorizonClient.set(ClientLevel.class, level, "connection", connection);
            HorizonClient.set(net.minecraft.world.level.Level.class, level, "isClientSide", true);
            HorizonClient.set(net.minecraft.world.level.Level.class, level, "dimension", net.minecraft.world.level.Level.OVERWORLD);
            HorizonClient.set(ClientPacketListener.class, connection, "level", level);
            local = (LocalPlayer) Natives.allocate(LocalPlayer.class);
            person(local, id, name);
            HorizonClient.set(LocalPlayer.class, local, "minecraft", mc);
            HorizonClient.set(LocalPlayer.class, local, "connection", connection);
            HorizonClient.set(LocalPlayer.class, local, "input", new net.minecraft.client.player.ClientInput());
            level.players().add(local);
            HorizonClient.set(Minecraft.class, mc, "player", local);
            mc.level = level;
            on = true;
            Natives.log("Java mods: " + String.join(", ", mods) + " run here for this player, on " + host + ":" + port);
            var sender = HorizonLoop.clientSender();
            HorizonServer.fireEach("ClientPlayConnectionEvents.INIT",
                    net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.INIT, l -> l.onPlayInit(connection, mc));
            HorizonServer.fireEach("ClientPlayConnectionEvents.JOIN",
                    net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.JOIN, l -> l.onPlayReady(connection, sender, mc));
            HorizonClient.tasks();
            return String.join(",", mods);
        } catch (Throwable t) {
            Natives.log("Java mods: the device mods' client couldn't join: " + t);
            for (StackTraceElement frame : t.getStackTrace()) Natives.log("    at " + frame);
            on = false;
            return "";
        }
    }

    /**
     * The game's connection: closed (nothing is written on it: the mods' messages go through
     * send), answering the server's address (Simple Voice Chat connects its voice there).
     */
    private static net.minecraft.network.Connection serverConnection(String host, int port) throws ReflectiveOperationException {
        var address = new java.net.InetSocketAddress(host, port);
        var connection = (net.minecraft.network.Connection) Natives.allocate(net.minecraft.network.Connection.class);
        var channel = new io.netty.channel.embedded.EmbeddedChannel() {
            @Override
            protected java.net.SocketAddress remoteAddress0() {
                return address;
            }
        };
        channel.remoteAddress();
        channel.close();
        HorizonClient.set(net.minecraft.network.Connection.class, connection, "channel", channel);
        return connection;
    }

    /** A player made without its constructor: who they are, where they stand. */
    private static void person(net.minecraft.world.entity.player.Player player, UUID id, String name) throws ReflectiveOperationException {
        HorizonClient.set(Entity.class, player, "uuid", id);
        HorizonClient.set(Entity.class, player, "stringUUID", id.toString());
        HorizonClient.set(net.minecraft.world.entity.player.Player.class, player, "gameProfile", new com.mojang.authlib.GameProfile(id, name));
        HorizonClient.set(Entity.class, player, "level", level);
        HorizonClient.set(Entity.class, player, "eyeHeight", 1.62f);
        HorizonClient.set(Entity.class, player, "dimensions", net.minecraft.world.entity.EntityDimensions.scalable(0.6f, 1.8f).withEyeHeight(1.62f));
        place(player, 0, 0, 0, 0, 0);
    }

    private static void place(Entity entity, double x, double y, double z, float yaw, float pitch) throws ReflectiveOperationException {
        HorizonClient.set(Entity.class, entity, "position", new Vec3(x, y, z));
        HorizonClient.set(Entity.class, entity, "blockPosition", net.minecraft.core.BlockPos.containing(x, y, z));
        HorizonClient.set(Entity.class, entity, "chunkPosition", new net.minecraft.world.level.ChunkPos(
                net.minecraft.core.SectionPos.blockToSectionCoord(x), net.minecraft.core.SectionPos.blockToSectionCoord(z)));
        HorizonClient.set(Entity.class, entity, "yRot", yaw);
        HorizonClient.set(Entity.class, entity, "xRot", pitch);
    }

    /**
     * One tick of the game: where the player and the others are ({@code json}: x y z yaw pitch,
     * players [{u n x y z yaw}] in the player's dimension), then the client's tick and frame for
     * the device mods, and their tasks.
     */
    public static void tick(String json) {
        if (!on) return;
        try {
            JsonObject state = JsonParser.parseString(json).getAsJsonObject();
            place(local, state.get("x").getAsDouble(), state.get("y").getAsDouble(), state.get("z").getAsDouble(),
                    state.get("yaw").getAsFloat(), state.get("pitch").getAsFloat());
            java.util.Set<UUID> seen = new java.util.HashSet<>();
            for (var element : state.getAsJsonArray("players")) {
                JsonObject p = element.getAsJsonObject();
                UUID id = UUID.fromString(p.get("u").getAsString());
                if (id.equals(local.getUUID())) continue;
                seen.add(id);
                RemotePlayer other = OTHERS.get(id);
                if (other == null) {
                    other = (RemotePlayer) Natives.allocate(RemotePlayer.class);
                    person(other, id, p.get("n").getAsString());
                    OTHERS.put(id, other);
                    level.players().add(other);
                }
                place(other, p.get("x").getAsDouble(), p.get("y").getAsDouble(), p.get("z").getAsDouble(),
                        p.get("yaw").getAsFloat(), 0f);
            }
            for (var it = OTHERS.entrySet().iterator(); it.hasNext(); ) {
                var entry = it.next();
                if (seen.contains(entry.getKey())) continue;
                level.players().remove(entry.getValue());
                it.remove();
            }
        } catch (Throwable t) {
            Natives.log("Java mods: the device client's tick couldn't read where players are: " + t);
        }
        Minecraft mc = Minecraft.getInstance();
        HorizonClient.tasks();
        HorizonClient.tickOnce();
        if (mc.gameRenderer != null) {
            HorizonHooks.fireFrame(mc.gameRenderer, true);
            HorizonHooks.fireRender(mc.gameRenderer);
            HorizonHooks.fireFrame(mc.gameRenderer, false);
        }
        HorizonClient.tasks();
    }

    /** A device mod's message from the server (its channel, its body). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void payload(String channel, byte[] body) {
        if (!on) return;
        CustomPacketPayload payload = HorizonLoop.read(channel, body,
                net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY, registries);
        if (payload == null) return;
        var handler = (net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.PlayPayloadHandler)
                net.fabricmc.fabric.impl.networking.client.ClientNetworkingImpl.PLAY.getHandler(payload.type().id());
        if (handler == null) return;
        Minecraft mc = Minecraft.getInstance();
        HorizonServer.fire("the client of " + channel, () -> handler.receive(payload, new net.fabricmc.fabric.api.client.networking.v1.ClientPlayNetworking.Context() {
            @Override
            public Minecraft client() {
                return mc;
            }

            @Override
            public LocalPlayer player() {
                return local;
            }

            @Override
            public net.fabricmc.fabric.api.networking.v1.PacketSender responseSender() {
                return HorizonLoop.clientSender();
            }
        }));
        HorizonClient.tasks();
    }

    /** A device mod's message to the server: over Horizon's connection. */
    static void send(CustomPacketPayload payload) {
        if (!HorizonDeviceMods.deviceChannel(payload.type().id())) return;
        byte[] body = HorizonLoop.write(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.SERVERBOUND_PLAY, registries);
        if (body != null) Natives.devicePayload(payload.type().id().toString(), body);
    }

    /** A line the mods' client code tells its player: to the game's chat. */
    static boolean message(LocalPlayer player, Component message, boolean overlay) {
        if (!on || player != local) return false;
        String json = HorizonServer.chatJson(message);
        if (overlay) Natives.overlay(local.getStringUUID(), json);
        else Natives.tell(local.getStringUUID(), json);
        return true;
    }

    /** The game left the server: the device mods' client disconnects (Minecraft.disconnect). */
    public static void leave() {
        if (!on) return;
        Minecraft mc = Minecraft.getInstance();
        try {
            HorizonHooks.fire(mc, Minecraft.class, "disconnect", "(Lnet/minecraft/client/gui/screens/Screen;ZZ)V", "HEAD",
                    new Object[] {null, false, false}, null, true);
            HorizonServer.fireEach("ClientPlayConnectionEvents.DISCONNECT",
                    net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.DISCONNECT, l -> l.onPlayDisconnect(connection, mc));
            HorizonClient.tasks();
        } catch (Throwable t) {
            Natives.log("Java mods: the device mods' client couldn't leave cleanly: " + t);
        }
        on = false;
        try {
            HorizonClient.set(Minecraft.class, mc, "player", null);
        } catch (ReflectiveOperationException e) {
            // Gone anyway.
        }
        mc.level = null;
        OTHERS.clear();
        local = null;
        level = null;
        connection = null;
    }
}
