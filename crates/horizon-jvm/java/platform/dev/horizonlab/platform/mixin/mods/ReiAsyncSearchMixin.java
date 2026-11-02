package dev.horizonlab.platform.mixin.mods;

import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Pseudo;

/**
 * REI's search, for ModsPlugin: its executor made non-final, so a search runs inside the asking
 * player's as() when several players share the client (HorizonRecipeViewer.searchPerPlayer).
 */
@Pseudo
@Mixin(targets = "me.shedaniel.rei.impl.client.search.AsyncSearchManager")
public abstract class ReiAsyncSearchMixin {}
