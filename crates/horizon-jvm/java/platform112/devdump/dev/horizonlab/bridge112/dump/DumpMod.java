package dev.horizonlab.bridge112.dump;

import java.util.Set;

import net.minecraftforge.fml.common.Mod;
import net.minecraftforge.fml.common.asm.transformers.deobf.FMLDeobfuscatingRemapper;
import net.minecraftforge.fml.common.event.FMLLoadCompleteEvent;

/**
 * Dev tool's mod half (see {@link ClassDumpAgent}): once FML has loaded, loads every class of
 * the game jar through the game's class loader (names mapped by FML's own remapper, so the
 * agent sees them transformed), then exits the VM so the agent writes its jar.
 */
@Mod(modid = "horizon112dump", name = "Horizon 1.12 class dump", version = "1")
public final class DumpMod {
    @Mod.EventHandler
    public void done(FMLLoadCompleteEvent event) {
        ClassLoader loader = DumpMod.class.getClassLoader();
        Set<String> obfed = FMLDeobfuscatingRemapper.INSTANCE.getObfedClasses();
        int ok = 0, failed = 0;
        for (String notch : obfed) {
            String name = FMLDeobfuscatingRemapper.INSTANCE.map(notch);
            ok += load(loader, name) ? 1 : 0;
        }
        // Classes Mojang's jar ships under their own names (client.main.Main, realms...).
        for (String name : new String[] {"net/minecraft/client/main/Main", "net/minecraft/server/MinecraftServer"}) {
            load(loader, name);
        }
        // Forge's classes (the universal jar refers to Mojang's by their obfuscated names, so
        // they are compiled against as FML transforms them too).
        int forge = 0;
        try (java.util.zip.ZipFile jar = new java.util.zip.ZipFile(new java.io.File(Mod.class.getProtectionDomain().getCodeSource().getLocation().toURI()))) {
            for (java.util.zip.ZipEntry e : java.util.Collections.list(jar.entries())) {
                String n = e.getName();
                if (n.startsWith("net/minecraftforge/") && n.endsWith(".class")) forge += load(loader, n.substring(0, n.length() - 6)) ? 1 : 0;
            }
        } catch (Exception e) {
            System.out.println("[horizon112] Forge's jar: " + e);
        }
        failed = obfed.size() - ok;
        System.out.println("[horizon112] loaded " + ok + " game classes (" + failed + " failed), " + forge + " Forge classes");
        System.exit(0); // the agent's shutdown hook writes the jar
    }

    private static boolean load(ClassLoader loader, String name) {
        try {
            Class.forName(name.replace('/', '.'), false, loader);
            return true;
        } catch (Throwable t) {
            return false;
        }
    }
}
