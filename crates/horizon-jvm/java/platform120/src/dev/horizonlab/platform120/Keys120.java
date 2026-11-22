package dev.horizonlab.platform120;

import com.mojang.blaze3d.platform.InputConstants;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
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

    /** The mods' key bindings (the loader's registry of them), each as the GLFW key it is set to. */
    static int[] modKeys() {
        List<KeyMapping> mods = Loader120.get().modKeyMappings();
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

    /**
     * The mods' mappings as the player's keys leave them now (held ones down), for what reads
     * them outside a client tick of keys (Hud120's frame: a zoom mod's key while held).
     */
    static void holdFor(UUID player) {
        KeyMapping.releaseAll();
        var held = DOWN.getOrDefault(player, Set.of());
        if (held.isEmpty()) return;
        var options = Minecraft.getInstance().options;
        if (options == null) return;
        // Every mapping bound to a held key, not only the one 1.20.1's key map keeps per key
        // (a mod's zoom on C shares it with vanilla's toolbar key, which would win).
        for (var mapping : options.keyMappings) {
            var key = ((dev.horizonlab.platform120.mixin.KeyMappingAccess) mapping).horizon$key();
            if (key.getType() == InputConstants.Type.KEYSYM && held.contains(key.getValue())) mapping.setDown(true);
        }
    }

    /** The player's keys held now. */
    static Set<Integer> held(UUID player) {
        return DOWN.getOrDefault(player, Set.of());
    }

    static void forget(UUID player) {
        DOWN.remove(player);
        CLICKS.remove(player);
    }

    /**
     * One client tick for each player with a key down or a click waiting: their keys set on
     * the mods' mappings (as the keyboard handler leaves them), then the loader's client tick
     * start and end with the headless client standing in for them.
     */
    static void tick(Map<UUID, ServerPlayer> players) {
        boolean listening = Loader120.get().clientTickListeners();
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
            Loader120.get().clientTick(minecraft);
            KeyMapping.releaseAll();
        }
    }
}
