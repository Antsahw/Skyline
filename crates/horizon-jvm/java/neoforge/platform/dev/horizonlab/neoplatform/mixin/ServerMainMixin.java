package dev.horizonlab.neoplatform.mixin;

import java.lang.reflect.Method;
import net.minecraft.SharedConstants;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.Main;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * FML's launcher calls vanilla's server main, as NeoForge patches it. Horizon is the server: at
 * the main's head the bridge runs only what it needs (as FabricBoot does for Fabric): the
 * version, vanilla's bootstrap (built-in registries), then NeoForge's mod loading (construction,
 * registration, common setup), and takes over. No eula.txt, server.properties, world or network.
 */
@Mixin(Main.class)
public abstract class ServerMainMixin {
    @Inject(method = "main", at = @At("HEAD"), cancellable = true, remap = false)
    private static void horizon$takeOver(String[] args, CallbackInfo ci) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Bootstrap.validate();
        Class<?> loader = Class.forName("net.neoforged.neoforge.server.loading.ServerModLoader");
        Method load = null;
        for (Method m : loader.getMethods()) if (m.getName().equals("load")) load = m;
        if (load == null) throw new IllegalStateException("NeoForge has no ServerModLoader.load");
        // NeoForge 26.3.0.101: load(boolean), and vanilla's main passes false (seen with a
        // ModifyArg on a normal server start). Another shape: say so rather than guess.
        if (load.getParameterCount() == 1 && load.getParameterTypes()[0] == boolean.class) load.invoke(null, false);
        else if (load.getParameterCount() == 0) load.invoke(null);
        else throw new IllegalStateException("ServerModLoader.load takes " + java.util.Arrays.toString(load.getParameterTypes()));
        Class.forName("dev.horizonlab.bridge.neo.NeoBoot", true, ClassLoader.getSystemClassLoader())
                .getMethod("ready", ClassLoader.class)
                .invoke(null, Main.class.getClassLoader());
        ci.cancel();
    }
}
