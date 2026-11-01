package dev.horizonlab.failmod;

import net.fabricmc.api.ModInitializer;
import net.fabricmc.fabric.api.event.lifecycle.v1.ServerTickEvents;

public final class FailMod implements ModInitializer {
    @Override
    public void onInitialize() {
        ServerTickEvents.END_SERVER_TICK.register(server -> {
            if (server.getTickCount() % 200 == 100) throw new IllegalStateException("failmod: the server tick fails on purpose");
        });
    }
}
