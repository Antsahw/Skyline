package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenKeyboardEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenMouseEvents;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;

/**
 * Input to a screen of the headless client as vanilla's MouseHandler and KeyboardHandler give it,
 * with the events their mixins fire around the screen's own handler: Architectury's
 * ClientScreenInputEvent (REI's overlay takes its clicks, keys and scrolls there, not from the
 * screen) and Fabric's ScreenMouseEvents/ScreenKeyboardEvents (JEI's overlay takes them there).
 * A PRE handler that interrupts, or a Fabric "allow" that says no, keeps the input from the
 * screen. Also the screen's tick, frame and removal with Fabric's events around them (Gui's).
 */
final class HorizonScreenEvents {
    private HorizonScreenEvents() {}

    static void click(Minecraft mc, Screen screen, MouseButtonEvent event, boolean doubleClick) {
        if (fire("MOUSE_CLICKED_PRE", "MouseClicked", mc, screen, event, doubleClick)) return;
        fabric(() -> {
            if (!ScreenMouseEvents.allowMouseClick(screen).invoker().allowMouseClick(screen, event)) return;
            ScreenMouseEvents.beforeMouseClick(screen).invoker().beforeMouseClick(screen, event);
            boolean consumed = screen.mouseClicked(event, doubleClick);
            ScreenMouseEvents.afterMouseClick(screen).invoker().afterMouseClick(screen, event, consumed);
        }, () -> screen.mouseClicked(event, doubleClick));
        fire("MOUSE_CLICKED_POST", "MouseClicked", mc, screen, event, doubleClick);
    }

    static void release(Minecraft mc, Screen screen, MouseButtonEvent event) {
        if (fire("MOUSE_RELEASED_PRE", "MouseReleased", mc, screen, event)) return;
        fabric(() -> {
            if (!ScreenMouseEvents.allowMouseRelease(screen).invoker().allowMouseRelease(screen, event)) return;
            ScreenMouseEvents.beforeMouseRelease(screen).invoker().beforeMouseRelease(screen, event);
            boolean consumed = screen.mouseReleased(event);
            ScreenMouseEvents.afterMouseRelease(screen).invoker().afterMouseRelease(screen, event, consumed);
        }, () -> screen.mouseReleased(event));
        fire("MOUSE_RELEASED_POST", "MouseReleased", mc, screen, event);
    }

    static void drag(Minecraft mc, Screen screen, MouseButtonEvent event, double dx, double dy) {
        if (fire("MOUSE_DRAGGED_PRE", "MouseDragged", mc, screen, event, dx, dy)) return;
        fabric(() -> {
            if (!ScreenMouseEvents.allowMouseDrag(screen).invoker().allowMouseDrag(screen, event, dx, dy)) return;
            ScreenMouseEvents.beforeMouseDrag(screen).invoker().beforeMouseDrag(screen, event, dx, dy);
            boolean consumed = screen.mouseDragged(event, dx, dy);
            ScreenMouseEvents.afterMouseDrag(screen).invoker().afterMouseDrag(screen, event, dx, dy, consumed);
        }, () -> screen.mouseDragged(event, dx, dy));
        fire("MOUSE_DRAGGED_POST", "MouseDragged", mc, screen, event, dx, dy);
    }

    static void scroll(Minecraft mc, Screen screen, double x, double y, double h, double v) {
        if (fire("MOUSE_SCROLLED_PRE", "MouseScrolled", mc, screen, x, y, h, v)) return;
        fabric(() -> {
            if (!ScreenMouseEvents.allowMouseScroll(screen).invoker().allowMouseScroll(screen, x, y, h, v)) return;
            ScreenMouseEvents.beforeMouseScroll(screen).invoker().beforeMouseScroll(screen, x, y, h, v);
            boolean consumed = screen.mouseScrolled(x, y, h, v);
            ScreenMouseEvents.afterMouseScroll(screen).invoker().afterMouseScroll(screen, x, y, h, v, consumed);
        }, () -> screen.mouseScrolled(x, y, h, v));
        fire("MOUSE_SCROLLED_POST", "MouseScrolled", mc, screen, x, y, h, v);
    }

    static void key(Minecraft mc, Screen screen, KeyEvent event) {
        if (fire("KEY_PRESSED_PRE", "KeyPressed", mc, screen, event)) return;
        fabric(() -> {
            if (!ScreenKeyboardEvents.allowKeyPress(screen).invoker().allowKeyPress(screen, event)) return;
            ScreenKeyboardEvents.beforeKeyPress(screen).invoker().beforeKeyPress(screen, event);
            screen.keyPressed(event);
            ScreenKeyboardEvents.afterKeyPress(screen).invoker().afterKeyPress(screen, event);
        }, () -> screen.keyPressed(event));
        fire("KEY_PRESSED_POST", "KeyPressed", mc, screen, event);
    }

    /**
     * A typed character, as KeyboardHandler.charTyped gives it: the mods' injections at its head
     * (JEI's search box takes its characters there; a cancel keeps them from the screen), the
     * screen's handler inside Fabric's events, then the injections at its tail.
     */
    static void character(Minecraft mc, Screen screen, CharacterEvent event) {
        Object[] args = {windowHandle(mc), event};
        if (HorizonHooks.fire(mc.keyboardHandler, KeyboardHandler.class, "charTyped", CHAR_TYPED, "HEAD", args, null, true).cancelled()) return;
        if (fire("CHAR_TYPED_PRE", "KeyTyped", mc, screen, event)) return;
        fabric(() -> {
            if (!ScreenKeyboardEvents.allowCharType(screen).invoker().allowCharType(screen, event)) return;
            ScreenKeyboardEvents.beforeCharType(screen).invoker().beforeCharType(screen, event);
            screen.charTyped(event);
            ScreenKeyboardEvents.afterCharType(screen).invoker().afterCharType(screen, event);
        }, () -> screen.charTyped(event));
        fire("CHAR_TYPED_POST", "KeyTyped", mc, screen, event);
        HorizonHooks.fire(mc.keyboardHandler, KeyboardHandler.class, "charTyped", CHAR_TYPED, "TAIL", args, null, true);
    }

    /** A screen's tick as Gui.tick runs it: Fabric's before and after tick events around it (JEI's list ticks after). */
    static void tick(Screen screen) {
        fabric(() -> ScreenEvents.beforeTick(screen).invoker().beforeTick(screen), () -> {});
        screen.tick();
        fabric(() -> ScreenEvents.afterTick(screen).invoker().afterTick(screen), () -> {});
    }

    /**
     * A screen's frame as Gui.extractRenderState draws it: Fabric's before and after extract
     * events around it (JEI updates what the mouse is over before the frame).
     */
    static void extract(Screen screen, net.minecraft.client.gui.GuiGraphicsExtractor graphics, int x, int y, float delta) {
        fabric(() -> ScreenEvents.beforeExtract(screen).invoker().beforeExtract(screen, graphics, x, y, delta), () -> {});
        screen.extractRenderStateWithTooltipAndSubtitles(graphics, x, y, delta);
        fabric(() -> ScreenEvents.afterExtract(screen).invoker().afterExtract(screen, graphics, x, y, delta), () -> {});
    }

    /** A screen leaves the GUI (Gui.setScreen's removed): Fabric's remove event after it. */
    static void removed(Screen screen) {
        screen.removed();
        fabric(() -> ScreenEvents.remove(screen).invoker().onRemove(screen), () -> {});
    }

    private static final String CHAR_TYPED = "(JLnet/minecraft/client/input/CharacterEvent;)V";

    static long windowHandle(Minecraft mc) {
        try {
            return mc.getWindow() != null ? mc.getWindow().handle() : 0L;
        } catch (Throwable t) {
            return 0L;
        }
    }

    /**
     * Runs {@code withEvents} (the input inside Fabric's screen events, which its MouseHandler
     * and KeyboardHandler mixins fire in a real client); a screen Fabric's init never saw has
     * none (its events throw): {@code without} then. A mod's handler that throws is logged once.
     */
    private static void fabric(Runnable withEvents, Runnable without) {
        try {
            withEvents.run();
        } catch (IllegalStateException e) {
            if (e.getMessage() != null && e.getMessage().contains("not been correctly initialised")) {
                without.run();
                return;
            }
            if (FAILED.add("fabric " + e)) Natives.log("Horizon's client: a mod's screen event handler failed: " + e);
        } catch (RuntimeException | LinkageError e) {
            if (FAILED.add("fabric " + e)) Natives.log("Horizon's client: a mod's screen event handler failed: " + e);
        }
    }

    private static final String EVENTS = "dev.architectury.event.events.client.ClientScreenInputEvent";
    private static Class<?> events;
    private static boolean none;
    private static final java.util.Map<String, Method> METHODS = new java.util.concurrent.ConcurrentHashMap<>();
    private static final java.util.Set<String> FAILED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /** Architectury's event {@code field}, fired: whether a handler interrupted it. */
    private static boolean fire(String field, String type, Object... args) {
        if (none) return false;
        try {
            ClassLoader loader = HorizonScreenEvents.class.getClassLoader();
            if (events == null) events = Class.forName(EVENTS, true, loader);
            Method method = METHODS.get(type);
            if (method == null) {
                for (Method m : Class.forName(EVENTS + "$" + type, true, loader).getMethods()) {
                    if (java.lang.reflect.Modifier.isAbstract(m.getModifiers())) method = m;
                }
                METHODS.put(type, method);
            }
            Object event = events.getField(field).get(null);
            Object invoker = Class.forName("dev.architectury.event.Event", true, loader).getMethod("invoker").invoke(event);
            Object result = method.invoke(invoker, args);
            return result != null && (boolean) Class.forName("dev.architectury.event.EventResult", true, loader)
                    .getMethod("interruptsFurtherEvaluation").invoke(result);
        } catch (ClassNotFoundException e) {
            none = true;
            return false;
        } catch (Throwable t) {
            Throwable root = t instanceof java.lang.reflect.InvocationTargetException i && i.getCause() != null ? i.getCause() : t;
            if (FAILED.add(field + root)) Natives.log("Horizon's client: a mod's " + field + " handler failed: " + root);
            return false;
        }
    }
}
