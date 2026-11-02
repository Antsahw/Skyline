package com.terraformersmc.modmenu.api;

/** Mod Menu's API as the fixture compiles against it (not in the jar: Horizon makes it). */
@FunctionalInterface
public interface ConfigScreenFactory<S extends net.minecraft.client.gui.screens.Screen> {
    S create(net.minecraft.client.gui.screens.Screen parent);
}
