package dev.horizonlab.platform120;

/**
 * The platform as a Forge mod (its META-INF/mods.toml names it): made by Forge as it loads the
 * mods, beside them. Says it is there; the bridge's connection to Horizon starts here in phase 4.
 */
@net.minecraftforge.fml.common.Mod("horizon_platform120")
public final class ForgeMod120 {
    public ForgeMod120() {
        Mirror120.log("Horizon platform loaded under Forge (" + Loader120.get().getClass().getSimpleName() + ")");
    }
}
