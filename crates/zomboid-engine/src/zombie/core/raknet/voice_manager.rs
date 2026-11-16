//! zombie.core.raknet.VoiceManager: the `VoiceManager` table `LuaRegister` puts in the Lua environment.
use std::rc::Rc;

use crate::se::krka::kahlua::j2se::j2se_platform::J2SEPlatform;
use crate::se::krka::kahlua::j2se::kahlua_table_impl::KahluaTableImpl;
use crate::se::krka::kahlua::vm::coroutine::FrameRef;
use crate::se::krka::kahlua::vm::kahlua_thread::KahluaThread;
use crate::se::krka::kahlua::vm::object::{
    classes, JavaClass, JavaFunction, Object, TableRef, Throwable, R,
};

/// The anonymous `JavaFunction` classes of `LuaRegister`, in source order.
static CLASSES: [JavaClass; 3] = [
    JavaClass {
        name: "zombie.core.raknet.VoiceManager$1",
        superclass: Some(&classes::OBJECT),
        interfaces: &[],
    },
    JavaClass {
        name: "zombie.core.raknet.VoiceManager$2",
        superclass: Some(&classes::OBJECT),
        interfaces: &[],
    },
    JavaClass {
        name: "zombie.core.raknet.VoiceManager$3",
        superclass: Some(&classes::OBJECT),
        interfaces: &[],
    },
];

/// `VoiceManager.instance.LuaRegister(platform, env)`'s `playerSetMute`, `playerGetMute` and `RecordDevices`.
/// `sound_disabled` is `Core.soundDisabled` (the client started with `-nosound`: no FMOD).
struct VoiceManagerFunction {
    index: usize,
    name: &'static str,
    sound_disabled: bool,
}

impl JavaFunction for VoiceManagerFunction {
    fn call(&self, thread: &mut KahluaThread, cf: &FrameRef, _n: i32) -> R<i32> {
        match self.name {
            // `!Core.soundDisabled && !VoiceManager.voipDisabled` is false: an empty table (voipDisabled is only
            // set by MainScreenState, after the menu is shown); otherwise the FMOD record drivers (UNKNOWN)
            "RecordDevices" if self.sound_disabled => {
                cf.push(Object::Table(thread.platform.new_table()))?;
                Ok(1)
            }
            name => Err(Throwable::runtime(format!(
                "not ported: zombie.core.raknet.VoiceManager.{name}"
            ))),
        }
    }
    fn java_to_string(&self) -> String {
        CLASSES[self.index].name.to_string()
    }
    fn class(&self) -> &'static JavaClass {
        &CLASSES[self.index]
    }
}

/// zombie.core.raknet.VoiceManager.LuaRegister: `environment.VoiceManager` is a table of three functions.
pub fn lua_register(platform: &Rc<J2SEPlatform>, environment: &TableRef, sound_disabled: bool) {
    let table = platform.new_table();
    for (index, name) in ["playerSetMute", "playerGetMute", "RecordDevices"]
        .into_iter()
        .enumerate()
    {
        KahluaTableImpl::rawset(
            &table,
            Object::str(name),
            Object::Function(Rc::new(VoiceManagerFunction {
                index,
                name,
                sound_disabled,
            })),
        );
    }
    KahluaTableImpl::rawset(
        environment,
        Object::str("VoiceManager"),
        Object::Table(table),
    );
}
