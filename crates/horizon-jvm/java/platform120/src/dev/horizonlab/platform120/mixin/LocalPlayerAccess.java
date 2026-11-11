package dev.horizonlab.platform120.mixin;

import net.minecraft.client.ClientRecipeBook;
import net.minecraft.client.player.LocalPlayer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The headless client's player's recipe book, which a screen's recipe book reads (Screens120). */
@Mixin(LocalPlayer.class)
public interface LocalPlayerAccess {
    @Mutable
    @Accessor("recipeBook")
    void horizon$setRecipeBook(ClientRecipeBook book);
}
