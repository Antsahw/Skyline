package dev.horizonlab.platform;

import com.mojang.blaze3d.platform.Monitor;
import com.mojang.blaze3d.platform.MonitorManager;
import com.mojang.blaze3d.platform.VideoMode;
import com.mojang.blaze3d.platform.Window;
import java.util.List;

/**
 * The headless window's monitors: one, of the window's size, without SDL (vanilla's manager
 * asks SDL which display the window is on). Mods size their HUD by it (JourneyMap's minimap:
 * JmUI.getQuarterMaxScale). Made with Natives.allocate: MonitorManager's constructor is SDL's.
 */
public final class HorizonMonitors extends MonitorManager {
    @Override
    public Monitor findBestMonitor(Window window) {
        return monitor(window);
    }

    @Override
    public Monitor getMonitor(int id) {
        return monitor(net.minecraft.client.Minecraft.getInstance().getWindow());
    }

    private static Monitor monitor(Window window) {
        int w = window == null ? 1280 : window.getWidth(), h = window == null ? 720 : window.getHeight();
        var mode = new VideoMode(w, h, 8, 8, 8, 60);
        return new Monitor("Horizon", 0, List.of(mode), mode, 0, 0, w, h);
    }
}
