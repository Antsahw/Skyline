package dev.horizonlab.configmod;

import com.terraformersmc.modmenu.api.ConfigScreenFactory;
import com.terraformersmc.modmenu.api.ModMenuApi;
import java.util.Map;

/** Its own settings screen, and one it gives "horizon_config_extra" (a mod id it doesn't own). */
public class ConfigMenu implements ModMenuApi {
    @Override
    public ConfigScreenFactory<?> getModConfigScreenFactory() {
        return parent -> new ConfigScreen("own", parent);
    }

    @Override
    public Map<String, ConfigScreenFactory<?>> getProvidedConfigScreenFactories() {
        return Map.of("horizon_config_extra", parent -> new ConfigScreen("provided", parent));
    }
}
