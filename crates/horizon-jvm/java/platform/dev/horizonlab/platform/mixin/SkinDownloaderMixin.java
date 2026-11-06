package dev.horizonlab.platform.mixin;

import dev.horizonlab.platform.HorizonClient;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.renderer.texture.SkinTextureDownloader;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * The headless client downloads no textures (FrozenLib's capes, through the skin manager's
 * downloader): the asset is named and left to the player's own game, which draws it. Only
 * players' skins are fetched (HorizonSkins' own downloader), for what mods build from them.
 */
@Mixin(SkinTextureDownloader.class)
public abstract class SkinDownloaderMixin {
    @Inject(method = "downloadAndRegisterSkin", at = @At("HEAD"), cancellable = true)
    private void horizon$noDownload(Identifier id, java.nio.file.Path file, String url, boolean legacy,
                                    CallbackInfoReturnable<CompletableFuture<ClientAsset.Texture>> cir) {
        if (HorizonClient.headless() && !dev.horizonlab.platform.HorizonSkins.fetches(this)) cir.setReturnValue(CompletableFuture.completedFuture(new ClientAsset.DownloadedTexture(id, url)));
    }
}
