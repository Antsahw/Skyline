package dev.horizonlab.platform120.mixin;

import java.util.Map;
import net.minecraft.client.gui.screens.MenuScreens;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/**
 * The screens mods registered for their menus (`MenuScreens.register`, in their client code).
 * Raw: the constructors' interface is package-private in 1.20.1, so they are found by shape
 * rather than by type (Screens120.constructorOf).
 */
@Mixin(MenuScreens.class)
public interface MenuScreensAccess {
    @SuppressWarnings("rawtypes")
    @Accessor("SCREENS")
    static Map horizon$screens() {
        throw new AssertionError();
    }
}
