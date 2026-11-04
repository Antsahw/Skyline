package dev.horizonlab.platform;

import dev.horizonlab.bridge.Natives;
import net.fabricmc.fabric.api.client.screen.v1.ScreenEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenKeyboardEvents;
import net.fabricmc.fabric.api.client.screen.v1.ScreenMouseEvents;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.MouseHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.CharacterEvent;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;

/**
 * Input to a screen of the headless client as vanilla's MouseHandler and KeyboardHandler give it,
 * with what mods' mixins do around the screen's own handler there: their @WrapOperation of
 * the handler's call (HorizonHooks.wrapCall: a wrap may skip it), and Fabric's
 * ScreenMouseEvents/ScreenKeyboardEvents. A wrap that doesn't call on, or a Fabric "allow"
 * that says no, keeps the input from the screen. Also the screen's tick, frame and removal with Fabric's events around them (Gui's).
 */
final class HorizonScreenEvents {
    private HorizonScreenEvents() {}

    private static final String SCREEN = "Lnet/minecraft/client/gui/screens/Screen;";
    private static final String MOUSE_EVENT = "Lnet/minecraft/client/input/MouseButtonEvent;";

    static void click(Minecraft mc, Screen screen, MouseButtonEvent event, boolean doubleClick) {
        click(mc, screen, event, doubleClick, true);
    }

    /**
     * A click; {@code screenToo} false: one Horizon's own screen handles (its slots, under the
     * mods' overlay): the mods still see it as their wraps and Fabric's events do (a mod's
     * search box lets go of the keyboard), the screen's own handler doesn't run.
     */
    static void click(Minecraft mc, Screen screen, MouseButtonEvent event, boolean doubleClick, boolean screenToo) {
        // A click moves the keyboard: whether a mod takes the typing is seen again (typing).
        TOOK_CHARS.remove(screen);
        HorizonHooks.wrapCall(mc.mouseHandler, MouseHandler.class, "onButton", SCREEN + "mouseClicked(" + MOUSE_EVENT + "Z)Z",
                new Object[] {screen, event, doubleClick}, a -> {
                    Screen s = (Screen) a[0];
                    MouseButtonEvent e = (MouseButtonEvent) a[1];
                    boolean d = (boolean) a[2];
                    boolean[] consumed = new boolean[1];
                    fabric(() -> {
                        if (!ScreenMouseEvents.allowMouseClick(s).invoker().allowMouseClick(s, e)) return;
                        ScreenMouseEvents.beforeMouseClick(s).invoker().beforeMouseClick(s, e);
                        consumed[0] = screenToo && s.mouseClicked(e, d);
                        ScreenMouseEvents.afterMouseClick(s).invoker().afterMouseClick(s, e, consumed[0]);
                    }, () -> consumed[0] = screenToo && s.mouseClicked(e, d));
                    return consumed[0];
                });
    }

    static void release(Minecraft mc, Screen screen, MouseButtonEvent event) {
        HorizonHooks.wrapCall(mc.mouseHandler, MouseHandler.class, "onButton", SCREEN + "mouseReleased(" + MOUSE_EVENT + ")Z",
                new Object[] {screen, event}, a -> {
                    Screen s = (Screen) a[0];
                    MouseButtonEvent e = (MouseButtonEvent) a[1];
                    boolean[] consumed = new boolean[1];
                    fabric(() -> {
                        if (!ScreenMouseEvents.allowMouseRelease(s).invoker().allowMouseRelease(s, e)) return;
                        ScreenMouseEvents.beforeMouseRelease(s).invoker().beforeMouseRelease(s, e);
                        consumed[0] = s.mouseReleased(e);
                        ScreenMouseEvents.afterMouseRelease(s).invoker().afterMouseRelease(s, e, consumed[0]);
                    }, () -> consumed[0] = s.mouseReleased(e));
                    return consumed[0];
                });
    }

    static void drag(Minecraft mc, Screen screen, MouseButtonEvent event, double dx, double dy) {
        HorizonHooks.wrapCall(mc.mouseHandler, MouseHandler.class, "handleAccumulatedMovement", SCREEN + "mouseDragged(" + MOUSE_EVENT + "DD)Z",
                new Object[] {screen, event, dx, dy}, a -> {
                    Screen s = (Screen) a[0];
                    MouseButtonEvent e = (MouseButtonEvent) a[1];
                    double x = (double) a[2], y = (double) a[3];
                    boolean[] consumed = new boolean[1];
                    fabric(() -> {
                        if (!ScreenMouseEvents.allowMouseDrag(s).invoker().allowMouseDrag(s, e, x, y)) return;
                        ScreenMouseEvents.beforeMouseDrag(s).invoker().beforeMouseDrag(s, e, x, y);
                        consumed[0] = s.mouseDragged(e, x, y);
                        ScreenMouseEvents.afterMouseDrag(s).invoker().afterMouseDrag(s, e, x, y, consumed[0]);
                    }, () -> consumed[0] = s.mouseDragged(e, x, y));
                    return consumed[0];
                });
    }

    static void scroll(Minecraft mc, Screen screen, double x, double y, double h, double v) {
        HorizonHooks.wrapCall(mc.mouseHandler, MouseHandler.class, "onScroll", SCREEN + "mouseScrolled(DDDD)Z",
                new Object[] {screen, x, y, h, v}, a -> {
                    Screen s = (Screen) a[0];
                    double ax = (double) a[1], ay = (double) a[2], ah = (double) a[3], av = (double) a[4];
                    boolean[] consumed = new boolean[1];
                    fabric(() -> {
                        if (!ScreenMouseEvents.allowMouseScroll(s).invoker().allowMouseScroll(s, ax, ay, ah, av)) return;
                        ScreenMouseEvents.beforeMouseScroll(s).invoker().beforeMouseScroll(s, ax, ay, ah, av);
                        consumed[0] = s.mouseScrolled(ax, ay, ah, av);
                        ScreenMouseEvents.afterMouseScroll(s).invoker().afterMouseScroll(s, ax, ay, ah, av, consumed[0]);
                    }, () -> consumed[0] = s.mouseScrolled(ax, ay, ah, av));
                    return consumed[0];
                });
    }

    static void key(Minecraft mc, Screen screen, KeyEvent event) {
        HorizonHooks.wrapCall(mc.keyboardHandler, KeyboardHandler.class, "keyPress", SCREEN + "keyPressed(Lnet/minecraft/client/input/KeyEvent;)Z",
                new Object[] {screen, event}, a -> {
                    Screen s = (Screen) a[0];
                    KeyEvent e = (KeyEvent) a[1];
                    boolean[] consumed = new boolean[1];
                    fabric(() -> {
                        if (!ScreenKeyboardEvents.allowKeyPress(s).invoker().allowKeyPress(s, e)) return;
                        ScreenKeyboardEvents.beforeKeyPress(s).invoker().beforeKeyPress(s, e);
                        consumed[0] = s.keyPressed(e);
                        ScreenKeyboardEvents.afterKeyPress(s).invoker().afterKeyPress(s, e);
                    }, () -> consumed[0] = s.keyPressed(e));
                    return consumed[0];
                });
    }

    /**
     * A typed character, as KeyboardHandler.charTyped gives it: the mods' injections at its head
     * (JEI's search box takes its characters there; a cancel keeps them from the screen), the
     * screen's handler inside Fabric's events, then the injections at its tail.
     */
    static void character(Minecraft mc, Screen screen, CharacterEvent event) {
        Object[] args = {windowHandle(mc), event};
        if (HorizonHooks.fire(mc.keyboardHandler, KeyboardHandler.class, "charTyped", CHAR_TYPED, "HEAD", args, null, true).cancelled()) {
            TOOK_CHARS.add(screen);
            return;
        }
        boolean[] reached = new boolean[1];
        fabric(() -> {
            if (!ScreenKeyboardEvents.allowCharType(screen).invoker().allowCharType(screen, event)) return;
            ScreenKeyboardEvents.beforeCharType(screen).invoker().beforeCharType(screen, event);
            reached[0] = true;
            screen.charTyped(event);
            ScreenKeyboardEvents.afterCharType(screen).invoker().afterCharType(screen, event);
        }, () -> {
            reached[0] = true;
            screen.charTyped(event);
        });
        // A mod took the character before the screen saw it (a text field of its own beside
        // the screen, focused): it is typed in until the next click.
        if (reached[0]) TOOK_CHARS.remove(screen);
        else TOOK_CHARS.add(screen);
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

    private static final java.util.Set<String> FAILED = java.util.concurrent.ConcurrentHashMap.newKeySet();

    /** Screens whose typed characters a mod took last (a text field of its own has the keyboard). */
    private static final java.util.Set<Screen> TOOK_CHARS = java.util.Collections.newSetFromMap(new java.util.WeakHashMap<>());

    /** A mod takes what is typed on {@code screen} (its last character, since the last click). */
    static boolean modTakesTyping(Screen screen) {
        return TOOK_CHARS.contains(screen);
    }
}
