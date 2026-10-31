package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import java.lang.reflect.Method;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;

/**
 * Input to a screen of the headless client as vanilla's MouseHandler and KeyboardHandler give it,
 * with the events their mixins fire around the screen's own handler: Architectury's
 * ClientScreenInputEvent (REI's overlay takes its clicks, keys and scrolls there, not from the
 * screen). A PRE handler that interrupts keeps the input from the screen.
 */
final class HorizonScreenEvents {
    private HorizonScreenEvents() {}

    static void click(Minecraft mc, Screen screen, MouseButtonEvent event, boolean doubleClick) {
        if (fire("MOUSE_CLICKED_PRE", "MouseClicked", mc, screen, event, doubleClick)) return;
        screen.mouseClicked(event, doubleClick);
        fire("MOUSE_CLICKED_POST", "MouseClicked", mc, screen, event, doubleClick);
    }

    static void release(Minecraft mc, Screen screen, MouseButtonEvent event) {
        if (fire("MOUSE_RELEASED_PRE", "MouseReleased", mc, screen, event)) return;
        screen.mouseReleased(event);
        fire("MOUSE_RELEASED_POST", "MouseReleased", mc, screen, event);
    }

    static void drag(Minecraft mc, Screen screen, MouseButtonEvent event, double dx, double dy) {
        if (fire("MOUSE_DRAGGED_PRE", "MouseDragged", mc, screen, event, dx, dy)) return;
        screen.mouseDragged(event, dx, dy);
        fire("MOUSE_DRAGGED_POST", "MouseDragged", mc, screen, event, dx, dy);
    }

    static void scroll(Minecraft mc, Screen screen, double x, double y, double h, double v) {
        if (fire("MOUSE_SCROLLED_PRE", "MouseScrolled", mc, screen, x, y, h, v)) return;
        screen.mouseScrolled(x, y, h, v);
        fire("MOUSE_SCROLLED_POST", "MouseScrolled", mc, screen, x, y, h, v);
    }

    static void key(Minecraft mc, Screen screen, KeyEvent event) {
        if (fire("KEY_PRESSED_PRE", "KeyPressed", mc, screen, event)) return;
        screen.keyPressed(event);
        fire("KEY_PRESSED_POST", "KeyPressed", mc, screen, event);
    }

    static void character(Minecraft mc, Screen screen, CharacterEvent event) {
        if (fire("CHAR_TYPED_PRE", "KeyTyped", mc, screen, event)) return;
        screen.charTyped(event);
        fire("CHAR_TYPED_POST", "KeyTyped", mc, screen, event);
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
