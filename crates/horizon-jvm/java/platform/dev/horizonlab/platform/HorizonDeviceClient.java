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

    // ---- Every mod's client half here (HorizonDeviceMods.OWN_CLIENT) -------------------------

    /** The player whose own client this process runs, for every mod (null: none). */
    private static volatile UUID own;
    private static String ownName;
    private static String ownServer, ownHost;
    private static int ownPort;

    /** Whether this process runs every mod's client half for its player, connected to another's server. */
    public static boolean own() {
        return own != null;
    }

    /**
     * Every mod's client half for this game's player: the host's own client code, run on a
     * mirror of what this game sees (HorizonServer.ownClientTick), with the mods' messages over
     * this game's connection. Returns "*" and the mods' ids.
     */
    private static String ownJoin(String host, int port, String uuid, String name) {
        Thread.currentThread().setContextClassLoader(HorizonDeviceClient.class.getClassLoader());
        own = UUID.fromString(uuid);
        ownName = name;
        ownServer = host + ":" + port;
        ownHost = host;
        ownPort = port;
        HorizonLoop.ownPlayer(uuid);
        Natives.log("Java mods: every mod's client half runs here for this player, on " + ownServer);
        java.util.List<String> ids = new java.util.ArrayList<>();
        for (var mod : net.fabricmc.loader.api.FabricLoader.getInstance().getAllMods()) ids.add(mod.getMetadata().getId());
        return "*," + String.join(",", ids);
    }

    /** The server this game plays on, as its own client names it (host:port), or null. */
    static String ownServer() {
        return own != null ? ownServer : null;
    }

    /**
     * The own client's connection is to the server this game plays on, as a vanilla client's
     * is: its address (mods name their per-server maps and waypoints by it) and its server
     * entry (Minecraft.getCurrentServer).
     */
    static void ownConnection(ClientPacketListener listener) throws ReflectiveOperationException {
        if (own == null) return;
        Class<?> common = net.minecraft.client.multiplayer.ClientCommonPacketListenerImpl.class;
        HorizonClient.set(common, listener, "connection", serverConnection(ownHost, ownPort));
        HorizonClient.set(common, listener, "serverData", new net.minecraft.client.multiplayer.ServerData(
                ownHost, ownServer, net.minecraft.client.multiplayer.ServerData.Type.OTHER));
    }

    private static void ownTick(JsonObject state) {
        if (state.has("w") && state.has("h")) {
            int w = state.get("w").getAsInt(), h = state.get("h").getAsInt();
            if (w > 0 && h > 0 && (w != HorizonClient.width || h != HorizonClient.height)) {
                HorizonClient.width = w;
                HorizonClient.height = h;
                HorizonClient.windowSize();
            }
        }
        double[] motion = {state.get("x").getAsDouble(), state.get("y").getAsDouble(), state.get("z").getAsDouble(),
                state.get("yaw").getAsDouble(), state.get("pitch").getAsDouble()};
        String[] inventory = null;
        if (state.has("inv")) {
            var slots = state.getAsJsonArray("inv");
            inventory = new String[slots.size()];
            for (int i = 0; i < inventory.length; i++) inventory[i] = slots.get(i).getAsString();
        }
        HorizonServer.ownClientTick(state.get("tick").getAsLong(), own.toString(), ownName, state.get("dim").getAsString(),
                motion, state.get("mode").getAsInt(), state.get("health").getAsFloat(),
                new float[] {state.get("food").getAsFloat(), state.get("sat").getAsFloat()}, state.get("perm").getAsInt(), inventory);
    }

    /**
     * The game joined another's server at {@code host}:{@code port} as {@code uuid}: the device
     * mods' client joins. Returns the device mods' ids (comma-separated) for the game to tell the
     * server, or "" when there are none (nothing is started).
     */
    public static String join(String host, int port, String uuid, String name) {
        if (!HorizonClient.headless()) return "";
        if (on || own != null) leave();
        if (HorizonDeviceMods.OWN_CLIENT) return ownJoin(host, port, uuid, name);
        var mods = HorizonDeviceMods.mods();
        if (mods.isEmpty()) return "";
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
     * players [{u n x y z yaw}] in the player's dimension, the GUI's size w h), then the
     * client's tick and frame for the device mods, and their tasks. Returns what the game
     * shows ({"hud": ops, "screen": ops, "close": true}, each only when it changed), or "{}".
     */
    public static String tick(String json) {
        if (own != null) {
            try {
                ownTick(JsonParser.parseString(json).getAsJsonObject());
            } catch (Throwable t) {
                Natives.log("Java mods: this game's own client's tick failed: " + t);
                HorizonFailures.report("the client's tick", t);
            }
            return "{}";
        }
        if (!on) return "{}";
        try {
            JsonObject state = JsonParser.parseString(json).getAsJsonObject();
            if (state.has("w") && state.has("h")) {
                int w = state.get("w").getAsInt(), h = state.get("h").getAsInt();
                if (w > 0 && h > 0 && (w != HorizonClient.width || h != HorizonClient.height)) {
                    HorizonClient.width = w;
                    HorizonClient.height = h;
                    HorizonClient.windowSize();
                    if (screen != null) screen.resize(w, h);
                }
            }
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
        try {
            // The player's keys on the mods' key bindings (isDown, clicks) for the tick.
            HorizonClient.withKeys(local.getUUID(), () -> {
                if (screen != null) HorizonScreenEvents.tick(screen);
                HorizonClient.tickOnce();
            });
        } catch (Throwable t) {
            Natives.log("Java mods: the device client's tick failed: " + t);
        }
        if (mc.gameRenderer != null) {
            HorizonHooks.fireFrame(mc.gameRenderer, true);
            HorizonHooks.fireRender(mc.gameRenderer);
            HorizonHooks.fireFrame(mc.gameRenderer, false);
        }
        HorizonClient.tasks();
        return shown();
    }

    // ---- What the player's game shows: the mods' HUD and screens ----------------------------

    private static net.minecraft.client.gui.screens.Screen screen;
    private static boolean closed;
    private static String hudSent, screenSent;
    private static double mouseX, mouseY;

    /** Minecraft.setScreen in the device client: the player's game shows it, or leaves it. */
    static void setScreen(net.minecraft.client.gui.screens.Screen next) {
        if (next instanceof HorizonOverlayScreen || next instanceof HorizonModsPage) next = null;
        if (next == screen) return;
        Minecraft mc = Minecraft.getInstance();
        try {
            if (screen != null) HorizonScreenEvents.removed(screen);
            screen = next;
            screenSent = null;
            HorizonClient.set(net.minecraft.client.gui.Gui.class, mc.gui, "screen", next);
            if (next == null) {
                closed = true;
                return;
            }
            next.added();
            next.init(HorizonClient.width, HorizonClient.height);
        } catch (Throwable t) {
            Natives.log("Java mods: the device client's screen " + (next == null ? "couldn't close" : next.getClass().getName()
                    + " couldn't open") + ": " + t);
            screen = null;
            closed = true;
        }
    }

    private static String shown() {
        JsonObject out = new JsonObject();
        try {
            var hud = HorizonClient.hud(local.getUUID(), false).toString();
            if (!hud.equals(hudSent)) {
                hudSent = hud;
                out.add("hud", JsonParser.parseString(hud));
            }
        } catch (Throwable t) {
            Natives.log("Java mods: the device client's HUD failed: " + t);
        }
        if (closed) {
            closed = false;
            out.addProperty("close", true);
        }
        if (screen != null) {
            try {
                HorizonGraphics graphics = new HorizonGraphics(HorizonClient.width, HorizonClient.height);
                graphics.imagesSent = HorizonClient.imagesSent(local.getUUID());
                HorizonClient.mouseAt(mouseX, mouseY);
                HorizonScreenEvents.extract(screen, graphics, (int) mouseX, (int) mouseY, 0);
                String ops = graphics.ops().toString();
                if (!ops.equals(screenSent)) {
                    screenSent = ops;
                    out.add("screen", graphics.ops());
                }
            } catch (Throwable t) {
                Natives.log("Java mods: the device client's screen failed to draw: " + t);
                HorizonFailures.report("drawing a screen", t);
            }
        }
        return out.toString();
    }

    /** A key of the player's game while playing (a device mod's key binding, its key handlers). */
    public static void key(int key, int keycode, int mods, boolean down) {
        if (!on) return;
        HorizonClient.bind(local.getUUID(), key, down);
        try {
            HorizonClient.firePress(key, keycode, mods, down);
        } catch (Throwable t) {
            Natives.log("Java mods: a key press in the device client failed: " + t);
        }
    }

    /** Input to the device client's screen ({"kind": move press release drag scroll key char close, ...}). */
    public static String input(String json) {
        if (!on) return "{}";
        var in = JsonParser.parseString(json).getAsJsonObject();
        String kind = in.get("kind").getAsString();
        var shown = screen;
        if (shown == null) return "{}";
        Minecraft mc = Minecraft.getInstance();
        double x = in.has("x") ? in.get("x").getAsDouble() : mouseX;
        double y = in.has("y") ? in.get("y").getAsDouble() : mouseY;
        int button = in.has("button") ? in.get("button").getAsInt() : 0;
        int mods = in.has("mods") ? in.get("mods").getAsInt() : 0;
        mouseX = x;
        mouseY = y;
        HorizonClient.mouseAt(x, y);
        var event = new net.minecraft.client.input.MouseButtonEvent(x, y, new net.minecraft.client.input.MouseButtonInfo(button, mods));
        try {
            switch (kind) {
                case "move" -> shown.mouseMoved(x, y);
                case "press" -> HorizonScreenEvents.click(mc, shown, event, in.has("double") && in.get("double").getAsBoolean());
                case "release" -> HorizonScreenEvents.release(mc, shown, event);
                case "drag" -> HorizonScreenEvents.drag(mc, shown, event,
                        in.has("dx") ? in.get("dx").getAsDouble() : 0, in.has("dy") ? in.get("dy").getAsDouble() : 0);
                case "scroll" -> HorizonScreenEvents.scroll(mc, shown, x, y, in.get("h").getAsDouble(), in.get("v").getAsDouble());
                case "key" -> {
                    int key = in.get("key").getAsInt();
                    HorizonScreenEvents.key(mc, shown, new net.minecraft.client.input.KeyEvent(
                            key, in.has("keycode") ? in.get("keycode").getAsInt() : key, mods));
                    // Escape on a screen that closes on it but stayed: closed, so the game never
                    // keeps a screen it can't leave.
                    if (key == 41 && screen == shown && shown.shouldCloseOnEsc()) setScreen(null);
                }
                case "char" -> HorizonScreenEvents.character(mc, shown, new net.minecraft.client.input.CharacterEvent(in.get("char").getAsInt()));
                case "close" -> {
                    // The game left the screen itself: it is gone here too (no close back).
                    setScreen(null);
                    closed = false;
                }
                default -> {}
            }
        } catch (Throwable t) {
            Natives.log("Java mods: the device client's screen " + kind + " failed: " + t);
            HorizonFailures.report("a screen", t);
        }
        HorizonClient.tasks();
        return shown();
    }

    /** A device mod's message from the server (its channel, its body). */
    @SuppressWarnings({"unchecked", "rawtypes"})
    public static void payload(String channel, byte[] body) {
        if (own != null) {
            var player = HorizonServer.player(own);
            var server = HorizonServer.server();
            if (player == null || server == null) return;
            CustomPacketPayload received = HorizonLoop.read(channel, body,
                    net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.CLIENTBOUND_PLAY, server.registryAccess());
            // To the mods' client receivers as their player's, delivered with the client's next tick.
            if (received != null) HorizonLoop.toClient(player, received);
            return;
        }
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
        var access = own != null && HorizonServer.server() != null ? HorizonServer.server().registryAccess() : registries;
        byte[] body = HorizonLoop.write(payload, net.fabricmc.fabric.impl.networking.PayloadTypeRegistryImpl.SERVERBOUND_PLAY, access);
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
        if (own != null) {
            UUID id = own;
            var player = HorizonServer.player(id);
            Minecraft mc = Minecraft.getInstance();
            try {
                if (player != null) {
                    // Minecraft.disconnect and Fabric's DISCONNECT, as this player's client leaves.
                    HorizonClient.as(player, () -> {
                        HorizonHooks.fire(mc, Minecraft.class, "disconnect", "(Lnet/minecraft/client/gui/screens/Screen;ZZ)V", "HEAD",
                                new Object[] {null, false, false}, null, true);
                        var listener = HorizonClient.connection(player);
                        HorizonServer.fireEach("ClientPlayConnectionEvents.DISCONNECT",
                                net.fabricmc.fabric.api.client.networking.v1.ClientPlayConnectionEvents.DISCONNECT,
                                l -> l.onPlayDisconnect(listener, mc));
                        HorizonClient.tasks();
                    });
                }
            } catch (Throwable t) {
                Natives.log("Java mods: this game's own client couldn't leave cleanly: " + t);
            }
            HorizonServer.ownClientLeft(id);
            own = null;
            Natives.log("Java mods: this game's own client left " + ownServer);
            return;
        }
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
        screen = null;
        closed = false;
        hudSent = screenSent = null;
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
