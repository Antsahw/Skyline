package dev.horizonlab.platform;

import com.mojang.authlib.GameProfile;
import com.mojang.authlib.minecraft.MinecraftProfileTexture;
import com.mojang.authlib.properties.Property;
import dev.horizonlab.bridge.Natives;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import net.minecraft.client.resources.DefaultPlayerSkin;
import net.minecraft.core.ClientAsset;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.player.PlayerModelType;
import net.minecraft.world.entity.player.PlayerSkin;

/**
 * Players' real skins in the headless client, for what mods build from a skin's pixels (3D
 * skin layers' voxels) and draw with it. A mirrored player's profile carries the
 * {@code textures} property Horizon's server has for it; vanilla's {@link
 * net.minecraft.client.renderer.texture.SkinTextureDownloader} fetches the skin (skins only,
 * kept in the game folder's {@code horizon-java/skins}) and registers its pixels under vanilla's
 * id, {@code minecraft:skins/<sha1>}, which is Horizon's own atlas key for that skin. A
 * launcher's offline skin, which no server knows, comes from the player's game once on join
 * ({@code horizon:java_own_skin}) and is registered as {@code horizon:offline_skin/<uuid>}.
 */
public final class HorizonSkins {
    private HorizonSkins() {}

    /** A game's own skin (an offline name's, from its launcher): its texture and arms. */
    private record Own(Identifier id, boolean slim) {}

    private static final Map<UUID, Own> OWN = new ConcurrentHashMap<>();
    /** Each player's {@code textures} property, as Horizon's server has it. */
    private static final Map<UUID, Property> TEXTURES = new ConcurrentHashMap<>();
    /** Skins being fetched or fetched, by texture id. */
    private static final Map<Identifier, CompletableFuture<ClientAsset.Texture>> FETCHED = new ConcurrentHashMap<>();
    /** The downloader the skins go through (the client's own downloads nothing: SkinDownloaderMixin). */
    private static volatile net.minecraft.client.renderer.texture.SkinTextureDownloader downloader;

    /** The texture id of a game's own skin. */
    public static Identifier ownId(UUID id) {
        return Identifier.fromNamespaceAndPath("horizon", "offline_skin/" + id);
    }

    /**
     * A game's own skin (Horizon: {@code horizon:java_own_skin}): 64x64 RGBA rows, slim arms or
     * not. Registered in the client's textures; the player's avatar is made again with it.
     */
    public static void ownSkin(String uuid, boolean slim, byte[] rgba) {
        if (!HorizonClient.headless() || rgba.length < 64 * 64 * 4) return;
        UUID id = UUID.fromString(uuid);
        var image = new com.mojang.blaze3d.platform.NativeImage(64, 64, false);
        for (int y = 0; y < 64; y++) {
            for (int x = 0; x < 64; x++) {
                int i = (y * 64 + x) * 4;
                int argb = (rgba[i + 3] & 255) << 24 | (rgba[i] & 255) << 16 | (rgba[i + 1] & 255) << 8 | (rgba[i + 2] & 255);
                image.setPixel(x, y, argb);
            }
        }
        Identifier texture = ownId(id);
        HorizonClient.minecraft().getTextureManager().register(texture,
                new net.minecraft.client.renderer.texture.DynamicTexture(texture::toString, image));
        Own before = OWN.put(id, new Own(texture, slim));
        if (before == null || before.slim() != slim) {
            Natives.log("Java mods: the player's own skin (" + (slim ? "slim" : "wide") + ") for " + uuid + " came from its game");
        }
        HorizonEntityRender.forgetAvatar(id);
    }

    /**
     * A player's {@code textures} property (Horizon's server: the profile it logged in with).
     * A mirrored player already made takes it into its profile.
     */
    public static void textures(String uuid, String value, String signature) {
        UUID id = UUID.fromString(uuid);
        Property property = new Property("textures", value, signature == null || signature.isEmpty() ? null : signature);
        Property before = TEXTURES.put(id, property);
        if (before != null && before.value().equals(value)) return;
        var player = HorizonServer.player(id);
        if (player != null) {
            try {
                HorizonClient.set(net.minecraft.world.entity.player.Player.class, player, "gameProfile",
                        profile(id, player.getGameProfile().name()));
            } catch (ReflectiveOperationException e) {
                Natives.log("Java mods: a player's skin couldn't reach its profile: " + e);
            }
        }
        HorizonEntityRender.forgetAvatar(id);
    }

    /** A mirrored player's profile: its {@code textures} property when Horizon has one. */
    public static GameProfile profile(UUID id, String name) {
        Property textures = TEXTURES.get(id);
        if (textures == null) return new GameProfile(id, name);
        var map = com.google.common.collect.ImmutableMultimap.<String, Property>of("textures", textures);
        return new GameProfile(id, name, new com.mojang.authlib.properties.PropertyMap(map));
    }

    /**
     * SkinManager.get in the headless client: the game's own skin, else the profile's
     * {@code textures} skin once fetched, else its default skin (DefaultPlayerSkin.get).
     */
    public static CompletableFuture<Optional<PlayerSkin>> get(GameProfile profile) {
        PlayerSkin fallback = DefaultPlayerSkin.get(profile);
        Own own = profile.id() == null ? null : OWN.get(profile.id());
        // Secure: PlayerInfo's lookup of another player (the headless client's players are all
        // others) keeps only secure skins; these are what Horizon's server has for the player.
        if (own != null) {
            return CompletableFuture.completedFuture(Optional.of(new PlayerSkin(
                    new ClientAsset.DownloadedTexture(own.id(), ""), null, null, own.slim() ? PlayerModelType.SLIM : PlayerModelType.WIDE, true)));
        }
        Property property = null;
        for (Property p : profile.properties().get("textures")) property = p;
        if (property == null && profile.id() != null) property = TEXTURES.get(profile.id());
        if (property == null) return CompletableFuture.completedFuture(Optional.of(fallback));
        MinecraftProfileTexture skin;
        try {
            skin = skinOf(property.value());
        } catch (RuntimeException e) {
            skin = null;
        }
        if (skin == null) return CompletableFuture.completedFuture(Optional.of(fallback));
        boolean slim = "slim".equals(skin.getMetadata("model"));
        return fetch(skin).handle((texture, failure) -> Optional.of(texture == null ? fallback
                : new PlayerSkin(texture, null, null, slim ? PlayerModelType.SLIM : PlayerModelType.WIDE, true)));
    }

    /** The SKIN texture of a {@code textures} property's value (base64 JSON), or null. */
    private static MinecraftProfileTexture skinOf(String value) {
        String json = new String(java.util.Base64.getDecoder().decode(value), java.nio.charset.StandardCharsets.UTF_8);
        var root = com.google.gson.JsonParser.parseString(json).getAsJsonObject();
        var textures = root.getAsJsonObject("textures");
        if (textures == null || !textures.has("SKIN")) return null;
        var skin = textures.getAsJsonObject("SKIN");
        String url = skin.get("url").getAsString();
        Map<String, String> metadata = new java.util.HashMap<>();
        if (skin.has("metadata")) {
            for (var e : skin.getAsJsonObject("metadata").entrySet()) metadata.put(e.getKey(), e.getValue().getAsString());
        }
        return new MinecraftProfileTexture(url, metadata);
    }

    /**
     * SkinManager.TextureCache.registerTexture: the skin under {@code skins/<sha1 of its hash>},
     * its file in the cache folder, fetched once.
     */
    @SuppressWarnings("deprecation")
    private static CompletableFuture<ClientAsset.Texture> fetch(MinecraftProfileTexture skin) {
        String hash = skin.getHash();
        Identifier id = Identifier.withDefaultNamespace("skins/" + com.google.common.hash.Hashing.sha1().hashUnencodedChars(hash));
        return FETCHED.computeIfAbsent(id, i -> {
            var mc = HorizonClient.minecraft();
            java.nio.file.Path file = mc.gameDirectory.toPath().resolve("horizon-java").resolve("skins")
                    .resolve(hash.length() > 2 ? hash.substring(0, 2) : "xx").resolve(hash);
            return downloader().downloadAndRegisterSkin(i, file, skin.getUrl(), true).whenComplete((texture, failure) -> {
                if (failure != null) {
                    Natives.log("Java mods: a player's skin couldn't be fetched (" + skin.getUrl() + "): " + failure);
                } else {
                    Natives.log("Java mods: a player's skin is " + i);
                }
            });
        });
    }

    private static net.minecraft.client.renderer.texture.SkinTextureDownloader downloader() {
        if (downloader == null) {
            java.net.Proxy proxy = java.net.Proxy.NO_PROXY;
            try {
                var chosen = java.net.ProxySelector.getDefault().select(java.net.URI.create("http://textures.minecraft.net/"));
                if (!chosen.isEmpty()) proxy = chosen.get(0);
            } catch (RuntimeException e) {
                // No selector: direct.
            }
            // Its textures registered at once (HorizonTextures' maps are concurrent).
            downloader = new net.minecraft.client.renderer.texture.SkinTextureDownloader(proxy,
                    HorizonClient.minecraft().getTextureManager(), Runnable::run);
        }
        return downloader;
    }

    /** Whether a downloader is the skins' own (SkinDownloaderMixin lets only it fetch). */
    public static boolean fetches(Object downloader) {
        return downloader != null && downloader == HorizonSkins.downloader;
    }
}
