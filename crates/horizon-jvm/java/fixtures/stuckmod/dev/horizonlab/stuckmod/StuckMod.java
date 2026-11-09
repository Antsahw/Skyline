package dev.horizonlab.stuckmod;

/** A mod like those "stuck on Fabric Loader 0.16.4": it only declares the range (horizon-jvm-remote's tests/boot120.rs). */
public final class StuckMod implements net.fabricmc.api.ModInitializer {
    @Override
    public void onInitialize() {
        System.out.println("[stuckmod] initialized on Fabric Loader " + net.fabricmc.loader.api.FabricLoader.getInstance()
                .getModContainer("fabricloader").get().getMetadata().getVersion().getFriendlyString());
    }
}
