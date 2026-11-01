package dev.horizonlab.failmod;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;

public final class FailClient implements ClientModInitializer {
    private int ticks;

    @Override
    public void onInitializeClient() {
        ClientTickEvents.END_CLIENT_TICK.register(client -> {
            if (client.player != null && ++ticks % 200 == 100) throw new NullPointerException("failmod: the client tick fails on purpose");
        });
    }
}
