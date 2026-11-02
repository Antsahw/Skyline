package com.terraformersmc.modmenu.api;

/** Mod Menu's API as the fixture compiles against it (not in the jar: Horizon makes it). */
public interface ModMenuApi {
    default ConfigScreenFactory<?> getModConfigScreenFactory() {
        return null;
    }

    default java.util.Map<String, ConfigScreenFactory<?>> getProvidedConfigScreenFactories() {
        return java.util.Map.of();
    }
}
