package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import java.util.List;
import java.util.Optional;
import net.minecraft.world.item.ItemStack;

/**
 * JEI (Just Enough Items) in the headless client: its item list over Horizon's inventory and its
 * recipe screens, as REI's (HorizonRecipeViewer). Reached by reflection: JEI is an optional mod.
 *
 * <p>A real client starts JEI as it joins a world: the server's recipes arrive (Fabric's recipe
 * sync, ClientRecipeSynchronizedEvent), then the recipe update packet (JEI's
 * AFTER_RECIPES_UPDATED, from its ClientPacketListener mixin) starts its runtime. The headless
 * client never joins, so {@link #prepare} does both, once, with the mirror's recipes, in the
 * first player's client (JEI's start reads the client level: as()). Its listeners for those are
 * registered at CLIENT_STARTED, which JEI registers from Minecraft's constructor (HorizonHooks:
 * the first resource reload's start). JEI's overlay then lays itself out on each container
 * screen's init (Fabric's AFTER_INIT), HorizonOverlayScreen under Horizon's inventory too, and
 * takes input through Fabric's screen events (HorizonScreenEvents).
 */
public final class HorizonJei {
    private HorizonJei() {}

    private static final String INTERNAL = "mezz.jei.common.Internal";
    private static volatile boolean started;
    private static boolean absent;

    /** JEI is among the mods. */
    public static boolean present() {
        if (absent) return false;
        try {
            Class.forName(INTERNAL, false, HorizonJei.class.getClassLoader());
            return true;
        } catch (ClassNotFoundException e) {
            absent = true;
            return false;
        }
    }

    /**
     * JEI started with the server's recipes, when it is there and hasn't yet; in a player's
     * client (as()), before a container screen is laid out (JEI registers its screen's input
     * events as it is, when running).
     */
    public static void prepare() {
        if (started || !present()) return;
        started = true;
        long t0 = System.nanoTime();
        var minecraft = net.minecraft.client.Minecraft.getInstance();
        if (minecraft.level == null) {
            started = false;
            return;
        }
        try {
            var server = HorizonServer.server();
            var recipes = net.fabricmc.fabric.impl.recipe.sync.SynchronizedRecipesImpl.of(server.getRecipeManager().getRecipes());
            // What Fabric's recipe sync does as the server's recipes reach a client (every
            // serializer synced: the client is the server's own).
            net.fabricmc.fabric.api.client.recipe.v1.sync.ClientRecipeSynchronizedEvent.EVENT.invoker()
                    .onRecipesSynchronized(minecraft, recipes);
            // What the recipe update packet does next (JEI starts there).
            ClassLoader loader = HorizonJei.class.getClassLoader();
            Class<?> events = Class.forName("mezz.jei.fabric.events.JeiLifecycleEvents", true, loader);
            Object event = events.getField("AFTER_RECIPES_UPDATED").get(null);
            ((Runnable) ((net.fabricmc.fabric.api.event.Event<?>) event).invoker()).run();
            Natives.log("Horizon's JEI: started with " + recipes.recipes().size() + " of the server's recipes in "
                    + (System.nanoTime() - t0) / 1_000_000 + " ms" + (runtime().isPresent() ? "" : " (no runtime: see JEI's log lines)"));
        } catch (Throwable t) {
            Throwable root = t;
            while (root.getCause() != null && root.getCause() != root) root = root.getCause();
            Natives.log("Horizon's JEI: could not start: " + root);
            HorizonFailures.report("JEI's start", root);
            StackTraceElement[] frames = root.getStackTrace();
            for (int f = 0; f < Math.min(frames.length, 14); f++) Natives.log("    at " + frames[f]);
        }
    }

    /** JEI's runtime (Internal.getOptionalJeiRuntime), once it started. */
    static Optional<Object> runtime() {
        if (!present()) return Optional.empty();
        try {
            if (getRuntime == null) getRuntime = Class.forName(INTERNAL, true, HorizonJei.class.getClassLoader()).getMethod("getOptionalJeiRuntime");
            return ((Optional<?>) getRuntime.invoke(null)).map(o -> (Object) o);
        } catch (Throwable t) {
            return Optional.empty();
        }
    }

    private static Method getRuntime, ingredientListOverlay, hasKeyboardFocus;

    /** JEI's search box has the keyboard (its ingredient list overlay's focus). */
    public static boolean searchFocused() {
        if (!started) return false;
        var runtime = runtime();
        if (runtime.isEmpty()) return false;
        try {
            Object overlay = ingredientListOverlay(runtime.get());
            return (boolean) hasKeyboardFocus.invoke(overlay);
        } catch (Throwable t) {
            return false;
        }
    }

    /**
     * JEI's recipes ({@code uses}: uses) of {@code stack} shown (its RecipesGui opens through
     * Gui.setScreen: a client-only screen); in the player's client. False when it has none.
     */
    static boolean open(ItemStack stack, boolean uses) throws ReflectiveOperationException {
        prepare();
        var runtime = runtime().orElse(null);
        if (runtime == null) return false;
        ClassLoader loader = HorizonJei.class.getClassLoader();
        Class<?> api = Class.forName("mezz.jei.api.runtime.IJeiRuntime", true, loader);
        Object helpers = api.getMethod("getJeiHelpers").invoke(runtime);
        Object focusFactory = Class.forName("mezz.jei.api.helpers.IJeiHelpers", true, loader).getMethod("getFocusFactory").invoke(helpers);
        Class<?> roles = Class.forName("mezz.jei.api.recipe.RecipeIngredientRole", true, loader);
        Object role = roles.getField(uses ? "INPUT" : "OUTPUT").get(null);
        Class<?> types = Class.forName("mezz.jei.api.constants.VanillaTypes", true, loader);
        Object itemType = types.getField("ITEM_STACK").get(null);
        Class<?> ingredientType = Class.forName("mezz.jei.api.ingredients.IIngredientType", true, loader);
        Object focus = Class.forName("mezz.jei.api.recipe.IFocusFactory", true, loader)
                .getMethod("createFocus", roles, ingredientType, Object.class).invoke(focusFactory, role, itemType, stack);
        // Any recipe category with a recipe for the focus (what the recipes screen would show).
        Object recipeManager = api.getMethod("getRecipeManager").invoke(runtime);
        Object lookup = Class.forName("mezz.jei.api.recipe.IRecipeManager", true, loader)
                .getMethod("createRecipeCategoryLookup").invoke(recipeManager);
        Class<?> lookupType = Class.forName("mezz.jei.api.recipe.IRecipeCategoriesLookup", true, loader);
        lookup = lookupType.getMethod("limitFocus", java.util.Collection.class).invoke(lookup, List.of(focus));
        boolean any = ((java.util.stream.Stream<?>) lookupType.getMethod("get").invoke(lookup)).findAny().isPresent();
        if (!any) return false;
        Object gui = api.getMethod("getRecipesGui").invoke(runtime);
        Class<?> ifocus = Class.forName("mezz.jei.api.recipe.IFocus", true, loader);
        Class.forName("mezz.jei.api.runtime.IRecipesGui", true, loader).getMethod("show", ifocus).invoke(gui, focus);
        return true;
    }

    static final boolean DEBUG = String.valueOf(System.getenv("HORIZON_JAVA_DEBUG")).contains("jei");
    private static String lastLogged;

    /**
     * HORIZON_JAVA_DEBUG=jei: the search text of JEI's item list and how many items it shows,
     * after input to the inventory overlay (when either changed).
     */
    static void logList() {
        if (!DEBUG || !started) return;
        var runtime = runtime().orElse(null);
        if (runtime == null) return;
        try {
            ClassLoader loader = HorizonJei.class.getClassLoader();
            Object filter = Class.forName("mezz.jei.api.runtime.IJeiRuntime", true, loader).getMethod("getIngredientFilter").invoke(runtime);
            Class<?> api = Class.forName("mezz.jei.api.runtime.IIngredientFilter", true, loader);
            String text = (String) api.getMethod("getFilterText").invoke(filter);
            Object itemType = Class.forName("mezz.jei.api.constants.VanillaTypes", true, loader).getField("ITEM_STACK").get(null);
            var items = (List<?>) api.getMethod("getFilteredIngredients", Class.forName("mezz.jei.api.ingredients.IIngredientType", true, loader))
                    .invoke(filter, itemType);
            String line = "\"" + text + "\": " + items.size() + " items" + (items.isEmpty() ? "" : " (first " + items.get(0) + ")");
            if (line.equals(lastLogged)) return;
            lastLogged = line;
            var player = net.minecraft.client.Minecraft.getInstance().player;
            Natives.log("Horizon's JEI: the list of " + (player == null ? "?" : player.getName().getString()) + ": " + line);
        } catch (Throwable t) {
            Natives.log("Horizon's JEI: its list can't be read: " + t);
        }
    }

    /** Each player's search text in JEI's list, while another player's client runs. */
    private static final java.util.Map<java.util.UUID, String> SEARCHES = new java.util.concurrent.ConcurrentHashMap<>();
    private static Method filterText, setFilterText, ingredientFilter;

    /**
     * The client changes player (HorizonClientState.load): JEI's one search text is theirs. JEI
     * keeps it once, in its runtime's filter (its search box follows it), so the host's search
     * showed in a guest's list. A player new to the client starts with an empty search. The
     * list searches again when the text changes (JEI caches the last result).
     */
    static void swap(java.util.UUID from, java.util.UUID to) {
        if (!started || to == null) return;
        var runtime = runtime().orElse(null);
        if (runtime == null) return;
        try {
            if (ingredientFilter == null) {
                ClassLoader loader = HorizonJei.class.getClassLoader();
                ingredientFilter = Class.forName("mezz.jei.api.runtime.IJeiRuntime", true, loader).getMethod("getIngredientFilter");
                Class<?> api = Class.forName("mezz.jei.api.runtime.IIngredientFilter", true, loader);
                filterText = api.getMethod("getFilterText");
                setFilterText = api.getMethod("setFilterText", String.class);
            }
            swapFocus(runtime, from, to);
            Object filter = ingredientFilter.invoke(runtime);
            String text = (String) filterText.invoke(filter);
            if (from != null) SEARCHES.put(from, text);
            String theirs = SEARCHES.getOrDefault(to, "");
            if (!theirs.equals(text)) setFilterText.invoke(filter, theirs);
        } catch (Throwable t) {
            if (HorizonServer.failedOnce("jei search swap")) Natives.log("Horizon's JEI: the search can't be kept per player: " + t);
        }
    }

    /** A player left: their search goes. */
    static void forget(java.util.UUID player) {
        SEARCHES.remove(player);
        FOCUS.remove(player);
    }

    /** JEI's keyboard focus as a player left it: its screen, control, restore action, the control's focus. */
    private record Focus(Object screen, Object focused, Object restore) {}

    private static final java.util.Map<java.util.UUID, Focus> FOCUS = new java.util.concurrent.ConcurrentHashMap<>();
    private static java.lang.reflect.Field focusScreen, focusFocused, focusRestore, widgetFocused;
    private static final Runnable NOTHING = () -> {};

    /**
     * JEI's keyboard focus (its overlay's GuiKeyboardFocus: the search box JEI gave the
     * keyboard, on which screen) is one for every player, and each frame it lets go of a
     * control whose screen isn't the open one: a guest's frame took the host's search box from
     * them as they typed. Kept per player: put away with the control's own focus, and theirs
     * put back (none for a player new to the client).
     */
    private static void swapFocus(Object runtime, java.util.UUID from, java.util.UUID to) throws ReflectiveOperationException {
        ClassLoader loader = HorizonJei.class.getClassLoader();
        Object overlay = ingredientListOverlay(runtime);
        Object focus = overlay.getClass().getMethod("getKeyboardFocus").invoke(overlay);
        if (focusScreen == null) {
            Class<?> type = Class.forName("mezz.jei.gui.input.focus.GuiKeyboardFocus", true, loader);
            focusScreen = type.getDeclaredField("screen");
            focusFocused = type.getDeclaredField("focused");
            focusRestore = type.getDeclaredField("restorePreviousFocus");
            widgetFocused = net.minecraft.client.gui.components.AbstractWidget.class.getDeclaredField("focused");
            for (var f : new java.lang.reflect.Field[] {focusScreen, focusFocused, focusRestore, widgetFocused}) f.setAccessible(true);
        }
        Object control = focusFocused.get(focus);
        if (from != null) FOCUS.put(from, new Focus(focusScreen.get(focus), control, focusRestore.get(focus)));
        Focus theirs = FOCUS.getOrDefault(to, new Focus(null, null, NOTHING));
        if (control instanceof net.minecraft.client.gui.components.AbstractWidget widget) widgetFocused.setBoolean(widget, false);
        focusScreen.set(focus, theirs.screen());
        focusFocused.set(focus, theirs.focused());
        focusRestore.set(focus, theirs.restore());
        if (theirs.focused() instanceof net.minecraft.client.gui.components.AbstractWidget widget) widgetFocused.setBoolean(widget, true);
    }

    private static Object ingredientListOverlay(Object runtime) throws ReflectiveOperationException {
        if (ingredientListOverlay == null) {
            ClassLoader loader = HorizonJei.class.getClassLoader();
            ingredientListOverlay = Class.forName("mezz.jei.api.runtime.IJeiRuntime", true, loader).getMethod("getIngredientListOverlay");
            hasKeyboardFocus = Class.forName("mezz.jei.api.runtime.IIngredientListOverlay", true, loader).getMethod("hasKeyboardFocus");
        }
        return ingredientListOverlay.invoke(runtime);
    }
}
