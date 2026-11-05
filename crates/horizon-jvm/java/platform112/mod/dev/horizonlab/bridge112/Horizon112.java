package dev.horizonlab.bridge112;

import java.util.Map;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.FutureTask;
import java.util.concurrent.LinkedBlockingQueue;
import java.util.function.Function;

import net.minecraftforge.common.MinecraftForge;
import net.minecraftforge.fml.common.Loader;
import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.ModContainer;
import net.minecraftforge.fml.common.event.FMLLoadCompleteEvent;
import net.minecraftforge.fml.common.eventhandler.EventPriority;
import net.minecraftforge.fml.common.eventhandler.SubscribeEvent;
import net.minecraftforge.fml.common.gameevent.TickEvent;

/**
 * The bridge's platform mod on the 1.12 side (docs/java-bridge-112.md). Written with MCP
 * names and renamed to SRG by tools/java_bridge/build112.sh, like any 1.12 mod.
 *
 * On 1.12's first client tick (every mod loaded, the main menu open) it hands Horizon a command
 * handler through Boot112's queue and keeps the client thread: from then on that thread runs
 * Horizon's commands (it owns the hidden GL context) and never returns to 1.12's game loop.
 */
@Mod(modid = Horizon112.ID, name = "Horizon bridge (1.12)", version = "1", acceptedMinecraftVersions = "[1.12.2]")
public final class Horizon112 {
    public static final String ID = "horizon112";
    private static final String HANDOVER = "horizon.bridge112.handover";

    private final BlockingQueue<Runnable> tasks = new LinkedBlockingQueue<>();
    private long loadedAt;
    private boolean handedOver;

    public Horizon112() {
        MinecraftForge.EVENT_BUS.register(this);
    }

    @Mod.EventHandler
    public void loaded(FMLLoadCompleteEvent event) {
        loadedAt = System.nanoTime();
    }

    @SubscribeEvent(priority = EventPriority.HIGHEST)
    public void clientTick(TickEvent.ClientTickEvent event) {
        if (handedOver || event.phase != TickEvent.Phase.START) return;
        handedOver = true;
        Object queue = System.getProperties().get(HANDOVER);
        if (!(queue instanceof BlockingQueue)) {
            System.out.println("[horizon112] no Horizon to hand over to: 1.12 keeps running on its own");
            return;
        }
        Function<String, String> handler = this::submit;
        try {
            @SuppressWarnings("unchecked")
            BlockingQueue<Object> handover = (BlockingQueue<Object>) queue;
            handover.put(handler);
            // The client thread is Horizon's now.
            while (true) tasks.take().run();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    /** Runs a command on the client thread and waits for its answer. */
    private String submit(String command) {
        FutureTask<String> task = new FutureTask<>(() -> run(command));
        tasks.add(task);
        try {
            return task.get();
        } catch (Exception e) {
            throw new IllegalStateException("1.12 command " + command + " failed", e);
        }
    }

    private String run(String command) throws Exception {
        if (command.startsWith("screenshot ")) {
            String[] p = command.split(" ", 3); // screenshot MODID PATH
            return Screenshot112.take(p[2], p[1]);
        }
        switch (command) {
            case "ping":
                return "pong from " + Thread.currentThread().getName();
            case "report":
                return Report112.build(mods(), (System.nanoTime() - loadedAt) / 1_000_000);
            default:
                throw new IllegalArgumentException("unknown 1.12 command: " + command);
        }
    }

    private static Map<String, String> mods() {
        Map<String, String> out = new java.util.LinkedHashMap<>();
        for (ModContainer mod : Loader.instance().getActiveModList()) out.put(mod.getModId(), mod.getVersion());
        return out;
    }
}
