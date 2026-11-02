package dev.horizonlab.platform.mixin.mods;

import dev.horizonlab.platform.HorizonClientConfig;
import java.io.File;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * JourneyMap's waypoint folder is each player's: a LAN guest's waypoints are kept in
 * waypoints/horizon-players/&lt;uuid&gt;/ (the host's stay where they were), so one player's
 * waypoints never land in another's file.
 */
@Pseudo
@Mixin(targets = "journeymap.client.io.FileHandler", remap = false)
public abstract class JourneyMapFilesMixin {
    @Inject(method = "getWaypointDir(Ljava/io/File;)Ljava/io/File;", at = @At("RETURN"), cancellable = true, remap = false)
    private static void horizon$playersOwn(File jmWorldDir, CallbackInfoReturnable<File> cir) {
        File dir = cir.getReturnValue();
        if (dir == null || !HorizonClientConfig.guestNow()) return;
        File mine = new File(new File(dir, "horizon-players"), String.valueOf(HorizonClientConfig.playerNow()));
        mine.mkdirs();
        cir.setReturnValue(mine);
    }
}
