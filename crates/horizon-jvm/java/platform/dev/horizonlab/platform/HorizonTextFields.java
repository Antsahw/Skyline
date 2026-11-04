package dev.horizonlab.platform;

import java.lang.ref.WeakReference;
import net.minecraft.client.gui.components.EditBox;

/**
 * The text box that last took the keyboard in the headless client (EditBox.setFocused:
 * EditBoxFocusMixin), wherever it is: one a mod keeps beside a screen (a recipe viewer's search
 * box) isn't among the screen's widgets, so the screen's focus doesn't name it. Typing goes to
 * it while it is focused and shown.
 */
public final class HorizonTextFields {
    private HorizonTextFields() {}

    private static volatile WeakReference<EditBox> last = new WeakReference<>(null);

    public static void focusChanged(EditBox box, boolean focused) {
        if (focused) last = new WeakReference<>(box);
        else if (last.get() == box) last = new WeakReference<>(null);
    }

    /** A text box has the keyboard. */
    static boolean focused() {
        EditBox box = last.get();
        return box != null && box.isFocused() && box.isVisible();
    }
}
