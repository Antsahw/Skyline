package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import net.minecraft.commands.CommandSourceStack;
import net.minecraft.network.chat.Component;
import net.minecraft.world.item.ItemStack;

/**
 * A recipe viewer mod's screens (REI's), for {@code /recipes} and {@code /uses}: what its R and U
 * keys open over an inventory, which Horizon draws its own way. Reached by reflection: the
 * viewer is an optional mod.
 *
 * <p>REI's client fills its display registry when its plugins reload, which a real client does
 * as it joins (the recipe packets); the headless client never joins, so the first command
 * reloads them (on the calling thread: REI's own reload thread is turned off for it) once the
 * server's displays are there.
 */
public final class HorizonRecipeViewer {
    private HorizonRecipeViewer() {}

    private static boolean reloaded;

    public static int open(CommandSourceStack source, ItemStack stack, boolean uses) {
        var player = source.getPlayer();
        if (player == null) {
            source.sendFailure(Component.literal("Only a player can open the recipe viewer"));
            return 0;
        }
        ItemStack item = stack != null ? stack : player.getMainHandItem().copyWithCount(1);
        if (item.isEmpty()) {
            source.sendFailure(Component.literal("Hold an item or name one: /" + (uses ? "uses" : "recipes") + " <item>"));
            return 0;
        }
        ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
        try {
            Class.forName("me.shedaniel.rei.api.client.view.ViewSearchBuilder", false, loader);
        } catch (ClassNotFoundException e) {
            if (HorizonJei.present()) return openJei(source, player, item, uses);
            source.sendFailure(Component.literal("No recipe viewer: add Roughly Enough Items or JEI to the Java mods"));
            return 0;
        }
        boolean[] opened = new boolean[1];
        int[] displays = new int[1];
        try {
            HorizonClient.as(player, () -> {
                if (!reloaded) {
                    reload(loader);
                    refreshSearch();
                }
                Class<?> registry = Class.forName("me.shedaniel.rei.api.client.registry.display.DisplayRegistry", true, loader);
                Object instance = registry.getMethod("getInstance").invoke(null);
                displays[0] = (int) registry.getMethod("size").invoke(instance);
                Class<?> entryStacks = Class.forName("me.shedaniel.rei.api.common.util.EntryStacks", true, loader);
                Object entry = entryStacks.getMethod("of", ItemStack.class).invoke(null, item);
                Class<?> builders = Class.forName("me.shedaniel.rei.api.client.view.ViewSearchBuilder", true, loader);
                Object builder = builders.getMethod("builder").invoke(null);
                Class<?> entryStack = Class.forName("me.shedaniel.rei.api.common.entry.EntryStack", true, loader);
                Method add = builders.getMethod(uses ? "addUsagesFor" : "addRecipesFor", entryStack);
                builder = add.invoke(builder, entry);
                opened[0] = (boolean) builders.getMethod("open").invoke(builder);
            });
        } catch (Throwable t) {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's recipe viewer failed: " + root);
            HorizonFailures.report("the recipe viewer", root);
            StackTraceElement[] frames = root.getStackTrace();
            for (int f = 0; f < Math.min(frames.length, 20); f++) Natives.log("    at " + frames[f]);
            source.sendFailure(Component.literal("The recipe viewer failed: " + root));
            return 0;
        }
        Natives.log("Horizon's recipe viewer: " + (uses ? "uses" : "recipes") + " of " + item + ": "
                + (opened[0] ? "opened" : "nothing to show") + " (" + displays[0] + " displays)");
        if (!opened[0]) {
            source.sendFailure(Component.literal("No " + (uses ? "uses" : "recipes") + " for " + item.getHoverName().getString()
                    + " (" + displays[0] + " recipes known)"));
            return 0;
        }
        return 1;
    }

    /** JEI's recipes screen for {@code item} (no REI among the mods). */
    private static int openJei(CommandSourceStack source, net.minecraft.server.level.ServerPlayer player, ItemStack item, boolean uses) {
        boolean[] opened = new boolean[1];
        try {
            HorizonClient.as(player, () -> opened[0] = HorizonJei.open(item, uses));
        } catch (Throwable t) {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's recipe viewer (JEI) failed: " + root);
            HorizonFailures.report("the recipe viewer", root);
            StackTraceElement[] frames = root.getStackTrace();
            for (int f = 0; f < Math.min(frames.length, 20); f++) Natives.log("    at " + frames[f]);
            source.sendFailure(Component.literal("The recipe viewer failed: " + root));
            return 0;
        }
        Natives.log("Horizon's recipe viewer (JEI): " + (uses ? "uses" : "recipes") + " of " + item + ": "
                + (opened[0] ? "opened" : "nothing to show"));
        if (!opened[0]) {
            source.sendFailure(Component.literal("No " + (uses ? "uses" : "recipes") + " for " + item.getHoverName().getString()));
            return 0;
        }
        return 1;
    }

    /**
     * The recipe viewers ready for the client, in the player's client (as()), before an overlay
     * that shows their item list: JEI started (HorizonJei), REI's displays in (its first reload:
     * ~3 s).
     */
    public static void prepare() {
        HorizonJei.prepare();
        if (reloaded) return;
        ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
        try {
            Class.forName("me.shedaniel.rei.api.client.view.ViewSearchBuilder", false, loader);
        } catch (ClassNotFoundException e) {
            return;
        }
        try {
            reload(loader);
        } catch (Throwable t) {
            Natives.log("Horizon's recipe viewer: REI could not reload: " + t);
        }
    }

    /**
     * REI's item list searched again (REIRuntime's overlay: queueReloadSearch). Its list searches
     * once as its overlay is made, which can be before the displays and entries are in: the list
     * then stays empty (0/1) until the search text changes.
     */
    public static void refreshSearch() {
        try {
            ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
            Class<?> overlay = Class.forName("me.shedaniel.rei.impl.client.gui.ScreenOverlayImpl", true, loader);
            // No overlay yet (a recipe view opened before any container screen): REI searches
            // as it makes one.
            Class<?> runtime = Class.forName("me.shedaniel.rei.api.client.REIRuntime", true, loader);
            Object made = runtime.getMethod("getOverlay").invoke(runtime.getMethod("getInstance").invoke(null));
            if (made instanceof java.util.Optional<?> optional && optional.isEmpty()) return;
            Object list = overlay.getMethod("getEntryListWidget").invoke(null);
            if (list != null) list.getClass().getMethod("updateSearch", String.class, boolean.class).invoke(list, searchText(loader), true);
        } catch (ClassNotFoundException e) {
            // No REI.
        } catch (Throwable t) {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's recipe viewer: REI's item list could not search: " + root);
            for (StackTraceElement frame : root.getStackTrace()) Natives.log("    at " + frame);
        }
    }

    private static java.lang.reflect.Method searchField, focused;
    private static boolean noRei;

    /** REI's search box has the keyboard (REIRuntimeImpl's search field, focused). */
    public static boolean searchFocused() {
        if (noRei) return false;
        try {
            if (searchField == null) {
                ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
                searchField = Class.forName("me.shedaniel.rei.impl.client.REIRuntimeImpl", true, loader).getMethod("getSearchField");
                focused = Class.forName("me.shedaniel.rei.impl.client.gui.widget.basewidgets.TextFieldWidget", true, loader).getMethod("isFocused");
            }
            Object field = searchField.invoke(null);
            return field != null && (boolean) focused.invoke(field);
        } catch (ClassNotFoundException | NoSuchMethodException e) {
            noRei = true;
            return false;
        } catch (Throwable t) {
            return false;
        }
    }

    private static String searchText(ClassLoader loader) {
        try {
            Object field = Class.forName("me.shedaniel.rei.impl.client.REIRuntimeImpl", true, loader).getMethod("getSearchField").invoke(null);
            if (field != null) return (String) field.getClass().getMethod("getText").invoke(field);
        } catch (Throwable t) {
            // The default: no search text.
        }
        return "";
    }

    /** REI's plugins, client and common, reloaded at once on this thread. */
    private static void reload(ClassLoader loader) throws ReflectiveOperationException {
        long started = System.nanoTime();
        Class<?> manager = Class.forName("me.shedaniel.rei.impl.common.plugins.ReloadManagerImpl", true, loader);
        Class<?> interruption = Class.forName("me.shedaniel.rei.impl.common.plugins.ReloadInterruptionContext", true, loader);
        Class<?> stage = Class.forName("me.shedaniel.rei.api.common.registry.ReloadStage", true, loader);
        Object never = interruption.getMethod("ofNever").invoke(null);
        // REI reloads its client on a thread of its own when its config says so (the default);
        // the command waits for it instead.
        Method reload = manager.getMethod("reloadPlugins", stage, interruption);
        // The server REI fills its displays from (GameInstance.getServer: on a client, the
        // client's single-player server): the mirror, while the reload runs.
        var mc = net.minecraft.client.Minecraft.getInstance();
        java.lang.reflect.Field single = net.minecraft.client.Minecraft.class.getDeclaredField("singleplayerServer");
        single.setAccessible(true);
        Object singleBefore = single.get(mc);
        if (HorizonServer.server() instanceof net.minecraft.client.server.IntegratedServer integrated) single.set(mc, integrated);
        try {
            reload.invoke(null, null, never);
            Method running = manager.getMethod("countRunningReloadTasks");
            long deadline = System.currentTimeMillis() + 60_000;
            while ((int) running.invoke(null) > 0 && System.currentTimeMillis() < deadline) {
                // Plugins that ask the server from REI's threads (Advanced Loot Info) get
                // their answer while this thread waits.
                HorizonLoop.pump();
                try {
                    Thread.sleep(20);
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                    break;
                }
            }
        } finally {
            single.set(mc, singleBefore);
        }
        int count = syncDisplays(loader);
        Natives.log("Horizon's recipe viewer: " + count + " of the server's displays synced to REI's client");
        reloaded = true;
        Natives.log("Horizon's recipe viewer: REI's plugins reloaded in " + (System.nanoTime() - started) / 1_000_000 + " ms");
    }

    /** The client registry's size after the last sync: fewer means REI reloaded since. */
    private static int syncedSize = -1;

    /**
     * What the server's display sync packet (DisplaySyncPacket, SET) does on a client that
     * joins: the server's displays (crafting, smelting, the mods' machines) into the client's
     * registry, as synced ones. The count added.
     */
    private static int syncDisplays(ClassLoader loader) throws ReflectiveOperationException {
        Class<?> serverRegistry = Class.forName("me.shedaniel.rei.api.common.registry.display.ServerDisplayRegistry", true, loader);
        Object server = serverRegistry.getMethod("getInstance").invoke(null);
        @SuppressWarnings("unchecked")
        var all = (java.util.Map<Object, java.util.List<Object>>) serverRegistry.getMethod("getAll").invoke(server);
        Class<?> clientImpl = Class.forName("me.shedaniel.rei.impl.client.registry.display.DisplayRegistryImpl", true, loader);
        Object client = Class.forName("me.shedaniel.rei.api.client.registry.display.DisplayRegistry", true, loader)
                .getMethod("getInstance").invoke(null);
        Object synced = clientImpl.getField("SYNCED").get(null);
        clientImpl.getMethod("removeSyncedRecipes").invoke(client);
        Class<?> display = Class.forName("me.shedaniel.rei.api.common.display.Display", true, loader);
        Method add = clientImpl.getMethod("add", display, Object.class);
        int count = 0;
        for (var list : all.values()) {
            for (Object d : list) {
                if ((boolean) add.invoke(client, d, synced)) count++;
            }
        }
        syncedSize = (int) Class.forName("me.shedaniel.rei.api.common.registry.display.DisplayRegistryCommon", true, loader)
                .getMethod("size").invoke(client);
        reindex(clientImpl.getMethod("cache").invoke(client));
        return count;
    }

    /**
     * REI's display lookup index made again over every display. Displays added after its reload
     * ended (the synced ones, here) go to its "not cached" set, which every lookup by input or
     * output scans whole: the craftable filter then took over 30 s on 2 cores (5,391 displays).
     * Its endReload indexes the not-cached set (and drops what was indexed), so the indexed ones
     * join them first. (Not cached is also where every display is while the option is off.)
     */
    @SuppressWarnings("unchecked")
    private static void reindex(Object cache) {
        try {
            Class<?> type = cache.getClass();
            java.lang.reflect.Field enabled = type.getDeclaredField("cache");
            java.lang.reflect.Field cached = type.getDeclaredField("displaysCached");
            java.lang.reflect.Field notCached = type.getDeclaredField("displaysNotCached");
            java.lang.reflect.Field preprocessed = type.getDeclaredField("preprocessed");
            for (var f : new java.lang.reflect.Field[] {enabled, cached, notCached, preprocessed}) f.setAccessible(true);
            // REI's "cache display lookup" option (off by default: then every lookup scans every
            // display); turned on for the headless client, which serves every player's lookups.
            enabled.setBoolean(cache, true);
            var waiting = (java.util.Set<Object>) notCached.get(cache);
            if (waiting.isEmpty()) return;
            java.util.Set<Object> all = java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
            all.addAll((java.util.Set<Object>) cached.get(cache));
            all.addAll(waiting);
            long started = System.nanoTime();
            notCached.set(cache, all);
            preprocessed.setBoolean(cache, false);
            type.getMethod("endReload").invoke(cache);
            Natives.log("Horizon's recipe viewer: REI's display index made again (" + all.size() + " displays, "
                    + (System.nanoTime() - started) / 1_000_000 + " ms)");
        } catch (ReflectiveOperationException | RuntimeException e) {
            if (HorizonServer.failedOnce("recipe viewer reindex")) Natives.log("Horizon's recipe viewer: REI's display index can't be made again: " + e);
        }
    }

    /**
     * REI reloads its plugins on its own too (on its thread, as a player's client joins:
     * Architectury's join event), which empties the client's displays of the server's: after
     * such a reload, they are synced again. True when they were (the item lists then search
     * again). Each tick; cheap when nothing changed.
     */
    public static boolean keepSynced() {
        if (!reloaded) return false;
        try {
            ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
            if (running == null) {
                running = Class.forName("me.shedaniel.rei.impl.common.plugins.ReloadManagerImpl", true, loader).getMethod("countRunningReloadTasks");
                size = Class.forName("me.shedaniel.rei.api.common.registry.display.DisplayRegistryCommon", true, loader).getMethod("size");
                registry = Class.forName("me.shedaniel.rei.api.client.registry.display.DisplayRegistry", true, loader).getMethod("getInstance");
            }
            if ((int) running.invoke(null) > 0) return false;
            if ((int) size.invoke(registry.invoke(null)) >= syncedSize) return false;
            int count = syncDisplays(loader);
            Natives.log("Horizon's recipe viewer: REI reloaded on its own; " + count + " of the server's displays synced again");
            return true;
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("recipe viewer keepSynced")) Natives.log("Horizon's recipe viewer: REI's displays can't be kept: " + t);
            return false;
        }
    }

    private static Method running, size, registry;

    private static volatile boolean perPlayer;
    /** The searches REI started inside the current as() on this thread, run at its end. */
    private static final ThreadLocal<java.util.ArrayDeque<Runnable>> QUEUED = new ThreadLocal<>();

    /**
     * REI's searches run inside their player's {@code as()} once the client serves a second
     * player. Its one search manager runs on threads of its own and reads the craftable-only
     * switch and the config when it runs, which with several players was often another
     * player's. A search started inside {@code as()} is queued and run at that call's end
     * ({@link #runQueued}), its player's settings still in (run where it was asked, inside the
     * overlay's layout, it stopped the overlay drawing); one started elsewhere goes to REI's
     * threads. This needs the display index ({@link #reindex}): without it the craftable search
     * took over 30 s and held the server thread. One player: REI's threads, as before.
     */
    static void searchPerPlayer() {
        if (perPlayer) return;
        perPlayer = true;
        try {
            Class<?> manager = Class.forName("me.shedaniel.rei.impl.client.search.AsyncSearchManager", true,
                    HorizonRecipeViewer.class.getClassLoader());
            java.lang.reflect.Field executor = manager.getDeclaredField("EXECUTOR_SERVICE");
            if (java.lang.reflect.Modifier.isFinal(executor.getModifiers())) {
                Natives.log("Horizon's recipe viewer: REI's search executor is final; searches stay on its threads");
                return;
            }
            executor.setAccessible(true);
            var own = (java.util.concurrent.ExecutorService) executor.get(null);
            executor.set(null, new java.util.concurrent.AbstractExecutorService() {
                @Override
                public void execute(Runnable command) {
                    var queue = QUEUED.get();
                    if (queue != null) queue.add(command);
                    else own.execute(command);
                }

                @Override
                public void shutdown() {
                    own.shutdown();
                }

                @Override
                public java.util.List<Runnable> shutdownNow() {
                    return own.shutdownNow();
                }

                @Override
                public boolean isShutdown() {
                    return own.isShutdown();
                }

                @Override
                public boolean isTerminated() {
                    return own.isTerminated();
                }

                @Override
                public boolean awaitTermination(long timeout, java.util.concurrent.TimeUnit unit) throws InterruptedException {
                    return own.awaitTermination(timeout, unit);
                }
            });
            Natives.log("Horizon's recipe viewer: REI searches inside each player's turn (several players)");
        } catch (ClassNotFoundException e) {
            // No REI.
        } catch (ReflectiveOperationException | RuntimeException e) {
            Natives.log("Horizon's recipe viewer: REI's search executor can't be replaced: " + e);
        }
    }

    /** An as() starts: REI's searches started now wait for its end (once searches are per player). */
    static java.util.ArrayDeque<Runnable> queueSearches() {
        if (!perPlayer) return null;
        var outer = QUEUED.get();
        QUEUED.set(new java.util.ArrayDeque<>());
        return outer;
    }

    /** The as() ends, its player's state still in: the searches it queued run (and the steps they queue). */
    static void runQueued(java.util.ArrayDeque<Runnable> outer) {
        var queue = QUEUED.get();
        if (queue == null) return;
        long started = System.nanoTime();
        int ran = 0;
        try {
            for (Runnable next; (next = queue.poll()) != null; ran++) {
                try {
                    next.run();
                } catch (Throwable t) {
                    if (HorizonServer.failedOnce("recipe viewer search")) Natives.log("Horizon's recipe viewer: REI's search failed: " + t);
                }
            }
        } finally {
            if (outer != null) QUEUED.set(outer);
            else QUEUED.remove();
        }
        long ms = (System.nanoTime() - started) / 1_000_000;
        if (ran > 0 && ms > 200 && SLOW_SEARCHES.getAndIncrement() < 5) {
            Natives.log("Horizon's recipe viewer: a search took " + ms + " ms (" + ran + " steps)");
        }
        if (ran > 0 && DEBUG) logListSize();
    }

    private static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("rei");

    /** HORIZON_JAVA_DEBUG=rei: how many entries the current player's item list holds after a search. */
    private static void logListSize() {
        try {
            ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
            Object list = Class.forName("me.shedaniel.rei.impl.client.gui.ScreenOverlayImpl", true, loader)
                    .getMethod("getEntryListWidget").invoke(null);
            for (Class<?> c = list.getClass(); c != null; c = c.getSuperclass()) {
                try {
                    java.lang.reflect.Field stacks = c.getDeclaredField("stacks");
                    stacks.setAccessible(true);
                    var player = net.minecraft.client.Minecraft.getInstance().player;
                    Natives.log("Horizon's recipe viewer: REI's list of " + (player == null ? "?" : player.getName().getString())
                            + ": " + ((java.util.List<?>) stacks.get(list)).size() + " entries");
                    return;
                } catch (NoSuchFieldException e) {
                    // Up the hierarchy.
                }
            }
        } catch (Throwable t) {
            // Debug only.
        }
    }

    private static final java.util.concurrent.atomic.AtomicInteger SLOW_SEARCHES = new java.util.concurrent.atomic.AtomicInteger();

    /**
     * After REI's own reload, on the player's screen (in as()): its reload drops the overlay
     * renderer it applied to the screen (ScreenRegistryImpl's last renderer provider, applied as
     * a screen is laid out: INIT_POST), so nothing of REI draws until the next screen; applied
     * again here, the overlay laid out again, and the list searched.
     */
    public static void afterOwnReload() {
        try {
            ClassLoader loader = HorizonRecipeViewer.class.getClassLoader();
            var screen = net.minecraft.client.Minecraft.getInstance().gui.screen();
            if (screen != null) {
                Class<?> impl = Class.forName("me.shedaniel.rei.impl.client.registry.screen.ScreenRegistryImpl", true, loader);
                Object registry = Class.forName("me.shedaniel.rei.api.client.registry.screen.ScreenRegistry", true, loader)
                        .getMethod("getInstance").invoke(null);
                impl.getMethod("getLastRendererProvider", net.minecraft.client.gui.screens.Screen.class).invoke(registry, screen);
                Natives.log("Horizon's recipe viewer: REI's overlay back on " + screen.getClass().getSimpleName());
            }
            Class<?> runtime = Class.forName("me.shedaniel.rei.api.client.REIRuntime", true, loader);
            Object made = runtime.getMethod("getOverlay").invoke(runtime.getMethod("getInstance").invoke(null));
            if (made instanceof java.util.Optional<?> optional && optional.isPresent()) {
                optional.get().getClass().getMethod("queueReloadOverlay").invoke(optional.get());
            }
        } catch (ClassNotFoundException e) {
            return;
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("recipe viewer afterOwnReload")) Natives.log("Horizon's recipe viewer: REI's overlay could not come back: " + t);
        }
        refreshSearch();
    }
}
