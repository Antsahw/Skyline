//! Device mods: Java mods whose client half uses the player's own sound devices (a microphone,
//! a speaker of their own: Simple Voice Chat). Their client half runs in each player's own game
//! (`HorizonDeviceMods`): a server's headless client leaves it out for other players, and a game
//! connected to another's server runs it in its own JVM (`HorizonDeviceClient`), with the mods'
//! messages over Horizon's connection.
//!
//! Server side: [`own_player`], [`runs_their_own`], [`from_game`]. Game side (connected to
//! another's server): [`join`], [`tick`], [`payload`], [`leave`], and [`take_out`] for what to
//! send the server.

use jni::objects::{GlobalRef, JClass, JString, JValue};
use std::sync::Mutex;

/// The channel a game says on, as it joins, which mods' client half it runs itself (comma
/// separated; `*` first: every mod's; `+` first: the mods the server lacked; else device mods).
pub const RUNS_OWN_CHANNEL: &str = "horizon:java_device_mods";

/// The channel a Horizon server running Java mods tells a joining game their ids on (comma
/// separated): the game runs the client half of the mods it has and the server doesn't.
pub const SERVER_MODS_CHANNEL: &str = "horizon:java_server_mods";

static OUT: Mutex<Vec<(String, Vec<u8>)>> = Mutex::new(Vec::new());

pub(crate) fn push_out(channel: String, data: Vec<u8>) {
    if let Ok(mut out) = OUT.lock() {
        out.push((channel, data));
    }
}

/// The device mods' messages for the server, in order: send each as a custom payload.
pub fn take_out() -> Vec<(String, Vec<u8>)> {
    OUT.lock()
        .map(|mut o| std::mem::take(&mut *o))
        .unwrap_or_default()
}

fn class(name: &str) -> Option<GlobalRef> {
    let bridge = crate::get()?;
    if !bridge.fabric() {
        return None;
    }
    let mut env = bridge.vm().attach_current_thread_permanently().ok()?;
    match bridge.find_class(&mut env, name) {
        Ok(class) => env.new_global_ref(class).ok(),
        Err(_) => {
            let _ = env.exception_clear();
            None
        }
    }
}

fn loop_class() -> Option<&'static GlobalRef> {
    static CLASS: std::sync::OnceLock<Option<GlobalRef>> = std::sync::OnceLock::new();
    if !crate::events::ready() {
        return None;
    }
    CLASS
        .get_or_init(|| class("dev/horizonlab/platform/HorizonLoop"))
        .as_ref()
}

fn client_class() -> Option<&'static GlobalRef> {
    static CLASS: std::sync::OnceLock<Option<GlobalRef>> = std::sync::OnceLock::new();
    if !crate::events::ready() {
        return None;
    }
    CLASS
        .get_or_init(|| class("dev/horizonlab/platform/HorizonDeviceClient"))
        .as_ref()
}

fn call<T>(
    class: &GlobalRef,
    run: impl FnOnce(&mut jni::JNIEnv, &JClass) -> jni::errors::Result<T>,
) -> Result<T, String> {
    let bridge = crate::get().ok_or("Java isn't running")?;
    let mut env = bridge
        .vm()
        .attach_current_thread_permanently()
        .map_err(|e| e.to_string())?;
    env.with_local_frame(16, |env| {
        let class = <&JClass>::from(class.as_obj());
        run(env, class)
    })
    .map_err(|e| crate::describe(&mut env, e))
}

/// Server: the player whose game this process is (the single-player world's owner), or none.
pub fn own_player(uuid: Option<&str>) -> Result<(), String> {
    let Some(class) = loop_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        let uuid = env.new_string(uuid.unwrap_or(""))?;
        env.call_static_method(
            class,
            "ownPlayer",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&uuid)],
        )?;
        Ok(())
    })
}

/// Server: this player's game runs these mods' client half itself (it said so on
/// [`RUNS_OWN_CHANNEL`], `mods` as it said them): they're left out here for that player, and
/// their messages go to it.
pub fn runs_their_own(uuid: &str, mods: &str) -> Result<(), String> {
    let Some(class) = loop_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        let uuid = env.new_string(uuid)?;
        let mods = env.new_string(mods)?;
        env.call_static_method(
            class,
            "runsTheirOwn",
            "(Ljava/lang/String;Ljava/lang/String;)V",
            &[JValue::Object(&uuid), JValue::Object(&mods)],
        )?;
        Ok(())
    })
}

/// Server: every Java mod's id (what [`SERVER_MODS_CHANNEL`] tells joining games). Empty while
/// Java isn't up.
pub fn mod_ids() -> Result<String, String> {
    static CLASS: std::sync::OnceLock<Option<GlobalRef>> = std::sync::OnceLock::new();
    if !crate::events::ready() {
        return Ok(String::new());
    }
    let Some(class) = CLASS
        .get_or_init(|| class("dev/horizonlab/platform/HorizonDeviceMods"))
        .as_ref()
    else {
        return Ok(String::new());
    };
    call(class, |env, class| {
        let value = env
            .call_static_method(class, "ids", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })
}

/// Server: a custom payload a player's game sent; Java takes it when its channel is a device
/// mod's (to the mod's server code, as that player's). Call on the server thread.
pub fn from_game(uuid: &str, channel: &str, data: &[u8]) -> Result<(), String> {
    let Some(class) = loop_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        let uuid = env.new_string(uuid)?;
        let channel = env.new_string(channel)?;
        let data = env.byte_array_from_slice(data)?;
        env.call_static_method(
            class,
            "fromGame",
            "(Ljava/lang/String;Ljava/lang/String;[B)V",
            &[
                JValue::Object(&uuid),
                JValue::Object(&channel),
                JValue::Object(&data),
            ],
        )?;
        Ok(())
    })
}

/// Game: it joined another's server at `host:port` as `uuid`; `server_mods` is what the
/// server said it runs ([`SERVER_MODS_CHANNEL`]; None: it said nothing, it runs no Java mods).
/// `Ok(None)` while Java isn't up yet (ask again later); `Ok(Some(mods))`: what runs here, as
/// the game tells the server on [`RUNS_OWN_CHANNEL`] (empty: nothing).
pub fn join(
    host: &str,
    port: u16,
    uuid: &str,
    name: &str,
    server_mods: Option<&str>,
) -> Result<Option<Vec<String>>, String> {
    let Some(class) = client_class() else {
        return Ok(None);
    };
    let mods = call(class, |env, class| {
        let host = env.new_string(host)?;
        let uuid = env.new_string(uuid)?;
        let name = env.new_string(name)?;
        let server = match server_mods {
            Some(mods) => env.new_string(mods)?.into(),
            None => jni::objects::JObject::null(),
        };
        let value = env
            .call_static_method(
                class,
                "join",
                "(Ljava/lang/String;ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                &[
                    JValue::Object(&host),
                    JValue::Int(port as i32),
                    JValue::Object(&uuid),
                    JValue::Object(&name),
                    JValue::Object(&server),
                ],
            )?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })?;
    let mods: String = mods;
    Ok(Some(
        mods.split(',')
            .filter(|m| !m.is_empty())
            .map(String::from)
            .collect(),
    ))
}

/// Game: one tick of the device client: `state` is where the player and the others stand
/// (`{"x","y","z","yaw","pitch","w","h","players":[{"u","n","x","y","z","yaw"}]}`). Returns
/// what the game shows, each only when it changed: `{"hud": ops, "screen": ops, "close": true}`.
pub fn tick(state: &str) -> Result<String, String> {
    let Some(class) = client_class() else {
        return Ok("{}".into());
    };
    call(class, |env, class| {
        let state = env.new_string(state)?;
        let value = env
            .call_static_method(
                class,
                "tick",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&state)],
            )?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })
}

/// Game: a key pressed or released while playing (SDL scancode, keycode, modifiers).
pub fn key(key: i32, keycode: i32, mods: i32, down: bool) -> Result<(), String> {
    let Some(class) = client_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        env.call_static_method(
            class,
            "key",
            "(IIIZ)V",
            &[
                JValue::Int(key),
                JValue::Int(keycode),
                JValue::Int(mods),
                JValue::Bool(down.into()),
            ],
        )?;
        Ok(())
    })
}

/// Game: input to the device client's screen (`java_screen`'s input JSON). Returns what the
/// game shows, as [`tick`] does.
pub fn input(input: &str) -> Result<String, String> {
    let Some(class) = client_class() else {
        return Ok("{}".into());
    };
    call(class, |env, class| {
        let input = env.new_string(input)?;
        let value = env
            .call_static_method(
                class,
                "input",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&input)],
            )?
            .l()?;
        Ok(env.get_string(&JString::from(value))?.into())
    })
}

/// Game: a custom payload from the server on a device mod's channel.
pub fn payload(channel: &str, data: &[u8]) -> Result<(), String> {
    let Some(class) = client_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        let channel = env.new_string(channel)?;
        let data = env.byte_array_from_slice(data)?;
        env.call_static_method(
            class,
            "payload",
            "(Ljava/lang/String;[B)V",
            &[JValue::Object(&channel), JValue::Object(&data)],
        )?;
        Ok(())
    })
}

/// Game: it left the server.
pub fn leave() -> Result<(), String> {
    let Some(class) = client_class() else {
        return Ok(());
    };
    call(class, |env, class| {
        env.call_static_method(class, "leave", "()V", &[])?;
        Ok(())
    })
}

/// A channel's namespace is one of these mods' ids.
pub fn is_device_channel(channel: &str, mods: &[String]) -> bool {
    channel
        .split_once(':')
        .is_some_and(|(ns, _)| mods.iter().any(|m| m == ns))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_channels_by_namespace() {
        let mods = vec!["voicechat".to_string()];
        assert!(is_device_channel("voicechat:secret", &mods));
        assert!(!is_device_channel("horizon:java_hud", &mods));
        assert!(!is_device_channel("voicechat", &mods));
    }
}
