package dev.horizonlab.platform.mixin.mods;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;

/** JourneyMap's full-screen map, for ModsPlugin (its per-client statics made non-final). */
@Pseudo
@Mixin(targets = "journeymap.client.ui.fullscreen.Fullscreen")
public abstract class JourneyMapFullscreenMixin {}
