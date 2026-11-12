package dev.horizonlab.platform120;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.server.level.ServerPlayer;

/**
 * The 1.20.1 mods' key bindings, and the client tick that reads them (phase 5). A mod's key
 * works like this in 1.20.1: it registers a KeyMapping (Fabric's KeyBindingHelper); the
 * keyboard handler clicks it when the key goes down; each client tick (ClientTickEvents) the
 * mod asks it consumeClick() and, usually, sends its server half a packet.
 *
 * <p>Here Horizon's player's game sends the keys the mods bind ({@link #modKeys}); each press
 * sets and clicks the mod's mapping as the keyboard handler would ({@link #press}); and once
 * per server tick, for each player with a key down or a click waiting, the headless client
 * stands in for them and runs a client tick ({@link #tick}), where the mods read their keys
 * and send their packets (Packets120 hands those to the server half).
 */
final class Keys120 {
    private Keys120() {}

    private static final Map<UUID, Set<Integer>> DOWN = new ConcurrentHashMap<>();
    private static final Map<UUID, List<Integer>> CLICKS = new ConcurrentHashMap<>();

    /** The mods' key bindings (Fabric's registry of them), each as the GLFW key it is set to. */
    @SuppressWarnings("unchecked")
    static int[] modKeys() {
        List<KeyMapping> mods;
        try {
            var field = net.fabricmc.fabric.impl.client.keybinding.KeyBindingRegistryImpl.class
                    .getDeclaredField("MODDED_KEY_BINDINGS");
            field.setAccessible(true);
            mods = (List<KeyMapping>) field.get(null);
        } catch (ReflectiveOperationException | RuntimeException e) {
            Mirror120.log("the mods' key bindings could not be read: " + e);
            return new int[0];
        }
        List<Integer> keys = new ArrayList<>();
        for (KeyMapping mapping : mods) {
            InputConstants.Key key = ((dev.horizonlab.platform120.mixin.KeyMappingAccess) mapping).horizon$key();
            if (key != null && key.getType() == InputConstants.Type.KEYSYM && key.getValue() >= 0
                    && !keys.contains(key.getValue())) {
                keys.add(key.getValue());
            }
        }
        return keys.stream().mapToInt(Integer::intValue).toArray();
    }

    /** `player`'s game says a key (GLFW's code) went down or up while playing. */
    static void press(ServerPlayer player, int key, boolean down) {
        if (down) {
            DOWN.computeIfAbsent(player.getUUID(), u -> ConcurrentHashMap.newKeySet()).add(key);
            CLICKS.computeIfAbsent(player.getUUID(), u -> new java.util.concurrent.CopyOnWriteArrayList<>()).add(key);
        } else {
            var held = DOWN.get(player.getUUID());
            if (held != null) held.remove(key);
        }
    }

    static void forget(UUID player) {
        DOWN.remove(player);
        CLICKS.remove(player);
    }

    /**
     * One client tick for each player with a key down or a click waiting: their keys set on
     * the mods' mappings (as the keyboard handler leaves them), then Fabric's ClientTickEvents
     * START and END with the headless client standing in for them.
     */
    static void tick(Map<UUID, ServerPlayer> players) {
        boolean listening = Events120.listeners(ClientTickEvents.START_CLIENT_TICK) != 0
                || Events120.listeners(ClientTickEvents.END_CLIENT_TICK) != 0;
        for (UUID id : new ArrayList<>(players.keySet())) {
            var held = DOWN.getOrDefault(id, Set.of());
            var clicks = CLICKS.remove(id);
            if (held.isEmpty() && (clicks == null || clicks.isEmpty())) continue;
            ServerPlayer player = players.get(id);
            if (player == null || !listening) continue;
            Screens120.clientPlayer(player);
            KeyMapping.releaseAll();
            for (int key : held) KeyMapping.set(InputConstants.Type.KEYSYM.getOrCreate(key), true);
            if (clicks != null) {
                for (int key : clicks) KeyMapping.click(InputConstants.Type.KEYSYM.getOrCreate(key));
            }
            Minecraft minecraft = Minecraft.getInstance();
            Events120.fire(ClientTickEvents.START_CLIENT_TICK, "client tick start", l -> l.onStartTick(minecraft));
            Events120.fire(ClientTickEvents.END_CLIENT_TICK, "client tick end", l -> l.onEndTick(minecraft));
            KeyMapping.releaseAll();
        }
    }
}
