package dev.horizonlab.platform120;

/**
 * The platform as a Forge mod (its META-INF/mods.toml names it): made by Forge as it loads the
 * mods, beside them. Once every mod has loaded (FMLLoadCompleteEvent), and when Horizon started
 * this game, it connects back to Horizon and answers its calls (ForgeRuntime120), and hides the
 * client's window: the game is Horizon's to show.
 */
@net.minecraftforge.fml.common.Mod("horizon_platform120")
public final class ForgeMod120 {
    public ForgeMod120() {
        Mirror120.log("Horizon platform loaded under Forge (" + Loader120.get().getClass().getSimpleName() + ")");
        net.minecraftforge.fml.javafmlmod.FMLJavaModLoadingContext.get().getModEventBus()
                .addListener((net.minecraftforge.fml.event.lifecycle.FMLLoadCompleteEvent e) -> loaded());
    }

    private static void loaded() {
        if (dev.horizonlab.bridge.remote.Child.serveInGame(new ForgeRuntime120())) {
            Mirror120.log("connecting to Horizon");
            var minecraft = net.minecraft.client.Minecraft.getInstance();
            minecraft.execute(() -> org.lwjgl.glfw.GLFW.glfwHideWindow(minecraft.getWindow().getWindow()));
        }
    }
}
