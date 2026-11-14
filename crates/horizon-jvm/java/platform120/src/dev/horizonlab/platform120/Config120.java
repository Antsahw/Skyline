package dev.horizonlab.platform120;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * The 1.20.1 mods' settings screens (phase 5h), as Mod Menu finds them: each mod's `modmenu`
 * entrypoint (`ModMenuApi`) gives a factory for its screen (`getModConfigScreenFactory`), and some
 * give factories for other mods' (`getProvidedConfigScreenFactories`). Called by reflection, so a
 * mod's screen is found whether or not Mod Menu itself is among the mods (its API classes then
 * come with the mod, or the entrypoint can't load and the mod has none here).
 */
final class Config120 {
    private Config120() {}

    /** Mod id to factory (`ConfigScreenFactory`), found once. */
    private static Map<String, Object> factories;

    private static synchronized Map<String, Object> factories() {
        if (factories != null) return factories;
        factories = new TreeMap<>();
        List<?> containers;
        try {
            Object loader = loader();
            containers = (List<?>) loader.getClass().getMethod("getEntrypointContainers", String.class, Class.class)
                    .invoke(loader, "modmenu", Object.class);
        } catch (ReflectiveOperationException e) {
            Mirror120.log("the mods' settings screens can't be listed: " + e);
            return factories;
        }
        for (Object container : containers) {
            String id = "?";
            try {
                id = (String) call(call(call(container, "getProvider"), "getMetadata"), "getId");
                Object api = call(container, "getEntrypoint");
                Object own = call(api, "getModConfigScreenFactory");
                if (own != null) factories.putIfAbsent(id, own);
                Object provided = call(api, "getProvidedConfigScreenFactories");
                if (provided instanceof Map<?, ?> map) {
                    for (var e : map.entrySet()) {
                        if (e.getKey() instanceof String other && e.getValue() != null) factories.putIfAbsent(other, e.getValue());
                    }
                }
            } catch (Throwable e) {
                // Reflection wraps the mod's own failure: say that one.
                Throwable cause = e;
                while (cause.getCause() != null && cause.getCause() != cause) cause = cause.getCause();
                Mirror120.log("the settings screen of " + id + " can't be found: " + cause);
            }
        }
        return factories;
    }

    /** Fabric Loader's API, by reflection: the platform jar is built without it. */
    private static Object loader() throws ReflectiveOperationException {
        return Class.forName("net.fabricmc.loader.api.FabricLoader").getMethod("getInstance").invoke(null);
    }

    private static Object call(Object target, String name) throws ReflectiveOperationException {
        for (Method m : target.getClass().getMethods()) {
            if (m.getName().equals(name) && m.getParameterCount() == 0) {
                m.setAccessible(true);
                return m.invoke(target);
            }
        }
        return null;
    }

    /** {ids, names} of the mods with a settings screen. */
    static Object[] mods() {
        List<String> ids = new ArrayList<>();
        List<String> names = new ArrayList<>();
        for (String id : factories().keySet()) {
            // Mod Menu's own entry for vanilla's options: Horizon has its own.
            if ("minecraft".equals(id)) continue;
            ids.add(id);
            String name = id;
            try {
                Object loader = loader();
                var found = (java.util.Optional<?>) loader.getClass().getMethod("getModContainer", String.class).invoke(loader, id);
                if (found.isPresent()) name = (String) call(call(found.get(), "getMetadata"), "getName");
            } catch (ReflectiveOperationException e) {
                // Its id, then.
            }
            names.add(name);
        }
        return new Object[] {ids.toArray(new String[0]), names.toArray(new String[0])};
    }

    /** The mod's settings screen, made with no parent (closing it closes it): opened on the client half. */
    static boolean open(String mod) {
        Object factory = factories().get(mod);
        if (factory == null) return false;
        // Settings screens measure their labels as they're built (Cloth Config's entries).
        Screens120.font();
        try {
            Method create = null;
            for (Method m : factory.getClass().getMethods()) {
                if (m.getName().equals("create") && m.getParameterCount() == 1) create = m;
            }
            if (create == null) return false;
            create.setAccessible(true);
            Object screen = create.invoke(factory, new Object[] {null});
            if (!(screen instanceof net.minecraft.client.gui.screens.Screen s)) return false;
            net.minecraft.client.Minecraft.getInstance().setScreen(s);
            return true;
        } catch (Throwable e) {
            Throwable cause = e instanceof java.lang.reflect.InvocationTargetException t && t.getCause() != null ? t.getCause() : e;
            Mirror120.log("the settings screen of " + mod + " didn't open: " + cause);
            for (StackTraceElement at : cause.getStackTrace()) Mirror120.log("    at " + at);
            return false;
        }
    }
}
