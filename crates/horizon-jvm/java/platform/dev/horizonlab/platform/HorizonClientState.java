package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Mods' client state kept per player. One headless client runs every player's client in turn
 * (HorizonClient.as), so what a mod keeps in a static field or a singleton is shared by them
 * all: the host typed "iron" in a mod's search box and the guest's showed it. The fields listed
 * here are swapped as the client changes player: each player's values are put away when another
 * player's client runs, and come back with theirs. A player new to the client starts from the
 * field's empty value (null, false, 0), which these mods fill in lazily .
 */
final class HorizonClientState {
    private HorizonClientState() {}

    /**
     * A field of client state: static, or of the instance {@code owner} returns (a method) or
     * holds (a static field); {@code fresh} makes a new player's value (null: the field's
     * empty value).
     */
    private record Slot(Field field, java.lang.reflect.Member owner, java.util.concurrent.Callable<Object> fresh) {
        Object target() throws ReflectiveOperationException {
            if (owner instanceof Method m) return m.invoke(null);
            if (owner instanceof Field f) return f.get(null);
            return null;
        }

        Object first() throws Exception {
            return fresh != null ? fresh.call() : empty(field.getType());
        }
    }

    /**
     * {@code Class(arg)}: a new instance by its constructor, with no argument or one enum
     * constant ({@code pkg.Enum.CONSTANT}).
     */
    private static java.util.concurrent.Callable<Object> factory(String spec, ClassLoader loader) throws ReflectiveOperationException {
        int open = spec.indexOf('(');
        Class<?> type = Class.forName(spec.substring(0, open), false, loader);
        String arg = spec.substring(open + 1, spec.length() - 1);
        if (arg.isEmpty()) {
            var make = type.getDeclaredConstructor();
            make.setAccessible(true);
            return make::newInstance;
        }
        int dot = arg.lastIndexOf('.');
        @SuppressWarnings({"unchecked", "rawtypes"})
        Object constant = Enum.valueOf((Class) Class.forName(arg.substring(0, dot), true, loader), arg.substring(dot + 1));
        for (var make : type.getDeclaredConstructors()) {
            if (make.getParameterCount() == 1 && make.getParameterTypes()[0].isInstance(constant)) {
                make.setAccessible(true);
                return () -> make.newInstance(constant);
            }
        }
        throw new NoSuchMethodException(spec);
    }

    /**
     * The fields, as {@code class#field} (static) or {@code class#getInstance()#field} (of the
     * instance a static method returns). A class that isn't there (its mod isn't) is skipped.
     */
    private static final String[] FIELDS = {
        // JourneyMap: its minimap's and full-screen map's state and renderer (made non-final:
        // mixin.mods.ModsPlugin), each player's own, centred on them. (Its MiniMap object is
        // shared: a new one asks UIManager for the minimap while it is made, and recurses.)
        "journeymap.client.ui.minimap.MiniMap#state=journeymap.client.model.map.MapState()",
        "journeymap.client.ui.minimap.MiniMap#mapRenderer=journeymap.client.render.map.MapRenderer(journeymap.client.ui.UI.Minimap)",
        "journeymap.client.ui.fullscreen.Fullscreen#state=journeymap.client.model.map.MapState()",
        "journeymap.client.ui.fullscreen.Fullscreen#mapRenderer=journeymap.client.render.map.MapRenderer(journeymap.client.ui.UI.Fullscreen)",
        // JourneyMap: its waypoints (handler: DAO and backend; the stores of waypoints and
        // groups). A new player's are made when asked for, from their own waypoint folder
        // (mixin.mods.JourneyMapFilesMixin), and loaded (loadedNow).
        "journeymap.client.waypoint.WaypointHandler#instance",
        "journeymap.common.waypoint.WaypointStore#INSTANCE",
        "journeymap.common.waypoint.WaypointGroupStore#INSTANCE",
        // VoxelMap: its minimap (the map image it shifts as its player moves, its zoom).
        "com.mamiyaotaru.voxelmap.VoxelConstants#getVoxelMapInstance()#map=com.mamiyaotaru.voxelmap.Map()",
    };

    private static List<Slot> slots;
    /** Each player's put-away values, by slot. */
    private static final Map<UUID, Object[]> SAVED = new ConcurrentHashMap<>();
    /** The player whose values are in the fields now (null: nobody's yet). */
    private static UUID loaded;

    /** The player whose values are in the fields now (null: nobody's yet). */
    static UUID loaded() {
        return loaded;
    }

    private static synchronized List<Slot> slots() {
        if (slots != null) return slots;
        List<Slot> made = new ArrayList<>();
        ClassLoader loader = HorizonClientState.class.getClassLoader();
        for (String full : FIELDS) {
            String[] made2 = full.split("=", 2);
            String spec = made2[0];
            String[] parts = spec.split("#");
            try {
                Class<?> owner = Class.forName(parts[0], false, loader);
                java.lang.reflect.Member instance = null;
                Class<?> holder = owner;
                if (parts.length == 3) {
                    if (parts[1].endsWith("()")) {
                        Method m = owner.getMethod(parts[1].replace("()", ""));
                        instance = m;
                        holder = m.getReturnType();
                    } else {
                        Field f = owner.getDeclaredField(parts[1]);
                        f.setAccessible(true);
                        instance = f;
                        holder = f.getType();
                    }
                }
                Field field = holder.getDeclaredField(parts[parts.length - 1]);
                if (Modifier.isFinal(field.getModifiers())) {
                    Natives.log("Horizon's client: per-player state " + spec + " is final; kept for all players");
                    continue;
                }
                field.setAccessible(true);
                made.add(new Slot(field, instance, made2.length == 2 ? factory(made2[1], loader) : null));
            } catch (ClassNotFoundException e) {
                // The mod isn't here.
            } catch (ReflectiveOperationException | RuntimeException e) {
                Natives.log("Horizon's client: per-player state " + spec + " can't be kept: " + e);
            }
        }
        slots = made;
        return made;
    }

    /** The fields hold {@code player}'s values (put away whose were there). */
    static synchronized void load(UUID player) {
        if (player == null || player.equals(loaded)) return;
        // Their mods' settings too (HorizonClientConfig).
        HorizonClientConfig.swap(loaded, player);
        List<Slot> all = slots();
        if (all.isEmpty()) {
            loaded = player;
            return;
        }
        try {
            if (loaded != null) {
                Object[] values = new Object[all.size()];
                for (int i = 0; i < all.size(); i++) {
                    Slot slot = all.get(i);
                    values[i] = slot.field().get(slot.target());
                }
                SAVED.put(loaded, values);
            }
            Object[] values = SAVED.get(player);
            boolean first = values == null;
            for (int i = 0; i < all.size(); i++) {
                Slot slot = all.get(i);
                Object value = values != null ? values[i] : null;
                if (values == null) {
                    // The first player keeps what the mod made (its own objects, started).
                    Object current = loaded == null ? slot.field().get(slot.target()) : null;
                    value = current != null ? current : slot.first();
                }
                slot.field().set(slot.target(), value);
            }
            boolean newGuest = first && loaded != null;
            loaded = player;
            loadedNow(first, newGuest);
        } catch (Exception e) {
            Throwable root = e;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's client: per-player state couldn't be swapped: " + root);
        }
    }

    /** A player left: their values go (and the fields', when theirs are in them). */
    static synchronized void forget(UUID player) {
        Object[] gone = SAVED.remove(player);
        HorizonClientConfig.forget(player);
        List<Slot> all = slots();
        if (player.equals(loaded)) {
            gone = new Object[all.size()];
            for (int i = 0; i < all.size(); i++) {
                Slot slot = all.get(i);
                try {
                    gone[i] = slot.field().get(slot.target());
                    slot.field().set(slot.target(), slot.first());
                } catch (Exception e) {
                    // Left as it is.
                }
            }
            loaded = null;
        }
        if (gone != null) stopped(gone, all);
    }

    /**
     * What a player who left had that runs on its own: VoxelMap's map's thread (each Map
     * starts one as it's made) is stopped, unless another player's values hold that map.
     */
    private static void stopped(Object[] gone, List<Slot> all) {
        for (int i = 0; i < gone.length; i++) {
            Object value = gone[i];
            if (value == null || !value.getClass().getName().equals("com.mamiyaotaru.voxelmap.Map")) continue;
            boolean kept = false;
            for (Object[] other : SAVED.values()) kept |= other.length > i && other[i] == value;
            try {
                Slot slot = all.get(i);
                kept |= slot.field().get(slot.target()) == value;
            } catch (ReflectiveOperationException | RuntimeException e) {
                kept = true;
            }
            if (kept) continue;
            try {
                value.getClass().getMethod("shutdown").invoke(value);
                Natives.log("Horizon's client: VoxelMap's map of a player who left stopped its thread");
            } catch (ReflectiveOperationException | RuntimeException e) {
                Natives.log("Horizon's client: VoxelMap's map of a player who left couldn't stop: " + e);
            }
        }
    }

    /**
     * What a mod needs after its state changed player: JourneyMap's cached player and labels
     * (its coordinates) are read again; VoxelMap's map images are the texture manager's again
     * (it draws them by name), and a new map knows the world and computes on the client's
     * thread (its own thread reads Minecraft.player, which is the first player between ticks).
     */
    private static void loadedNow(boolean first, boolean newGuest) {
        ClassLoader loader = HorizonClientState.class.getClassLoader();
        if (newGuest) {
            // A new player's JourneyMap waypoints: their stores made and filled from their DAO.
            try {
                Object groups = Class.forName("journeymap.common.waypoint.WaypointGroupStore", false, loader).getMethod("getInstance").invoke(null);
                groups.getClass().getMethod("reset").invoke(groups);
                Object store = Class.forName("journeymap.common.waypoint.WaypointStore", false, loader).getMethod("getInstance").invoke(null);
                store.getClass().getMethod("reset", boolean.class).invoke(store, false);
                Object all = store.getClass().getMethod("getAll").invoke(store);
                Natives.log("Horizon's client: a new player's JourneyMap waypoints, from their own folder: "
                        + ((java.util.Collection<?>) all).size());
            } catch (ClassNotFoundException e) {
                // No JourneyMap.
            } catch (ReflectiveOperationException | RuntimeException e) {
                Throwable root = e;
                while (root.getCause() != null && root.getCause() != root) root = root.getCause();
                Natives.log("Horizon's client: a new player's JourneyMap waypoints couldn't load: " + root);
            }
        }
        try {
            Class<?> cache = Class.forName("journeymap.client.data.DataCache", false, loader);
            Object instance = cache.getField("INSTANCE").get(null);
            Field player = cache.getDeclaredField("player");
            player.setAccessible(true);
            if (player.get(instance) instanceof com.google.common.cache.Cache<?, ?> loading) loading.invalidateAll();
            // Its labels (coordinates...) are cached for a while: made again for this player.
            Class.forName("journeymap.client.ui.theme.ThemeLabelSource", false, loader).getMethod("resetCaches").invoke(null);
        } catch (ClassNotFoundException e) {
            // No JourneyMap.
        } catch (ReflectiveOperationException | RuntimeException e) {
            Natives.log("Horizon's client: JourneyMap's player couldn't be read again: " + e);
        }
        try {
            Class<?> constants = Class.forName("com.mamiyaotaru.voxelmap.VoxelConstants", false, loader);
            Object voxel = constants.getMethod("getVoxelMapInstance").invoke(null);
            Object map = voxel.getClass().getMethod("getMap").invoke(voxel);
            if (map == null) return;
            Class<?> mapClass = map.getClass();
            var textures = net.minecraft.client.Minecraft.getInstance().getTextureManager();
            for (String[] pair : new String[][] {{"resourceMapImageFiltered", "mapImagesFiltered"}, {"resourceMapImageUnfiltered", "mapImagesUnfiltered"}}) {
                Field ids = mapClass.getDeclaredField(pair[0]);
                Field images = mapClass.getDeclaredField(pair[1]);
                ids.setAccessible(true);
                images.setAccessible(true);
                Object[] idArray = (Object[]) ids.get(map);
                Object[] imageArray = (Object[]) images.get(map);
                for (int i = 0; i < idArray.length; i++) {
                    if (idArray[i] != null && imageArray[i] != null) {
                        textures.register((net.minecraft.resources.Identifier) idArray[i],
                                (net.minecraft.client.renderer.texture.AbstractTexture) imageArray[i]);
                    }
                }
            }
            if (!SAVED.isEmpty()) {
                // More than one player: no map computes on its own thread.
                Field threading = mapClass.getDeclaredField("threading");
                threading.setAccessible(true);
                if (threading.getBoolean(map)) threading.setBoolean(map, false);
            }
            Field world = mapClass.getDeclaredField("world");
            world.setAccessible(true);
            Field voxelWorld = voxel.getClass().getDeclaredField("world");
            voxelWorld.setAccessible(true);
            Object level = voxelWorld.get(voxel);
            if (world.get(map) == null && level != null) {
                mapClass.getMethod("newWorld", net.minecraft.client.multiplayer.ClientLevel.class).invoke(map, level);
            }
        } catch (ClassNotFoundException e) {
            // No VoxelMap.
        } catch (ReflectiveOperationException | RuntimeException e) {
            Natives.log("Horizon's client: VoxelMap's map couldn't change player: " + e);
        }
    }

    private static Object empty(Class<?> type) {
        if (!type.isPrimitive()) {
            // A collection a mod makes once and never again (REI's lastDisplayScreen, cleared at
            // each reload): a new empty one of its class.
            if ((java.util.Collection.class.isAssignableFrom(type) || java.util.Map.class.isAssignableFrom(type))
                    && !type.isInterface() && !Modifier.isAbstract(type.getModifiers())) {
                try {
                    return type.getDeclaredConstructor().newInstance();
                } catch (ReflectiveOperationException e) {
                    return null;
                }
            }
            return null;
        }
        if (type == boolean.class) return false;
        if (type == char.class) return (char) 0;
        if (type == long.class) return 0L;
        if (type == float.class) return 0F;
        if (type == double.class) return 0D;
        if (type == byte.class) return (byte) 0;
        if (type == short.class) return (short) 0;
        return 0;
    }
}
