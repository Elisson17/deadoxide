#!/usr/bin/env python3
"""Generates crates/zomboid-engine/src/zombie/lua/exposed_table.rs: the part of the Lua environment the ORIGINAL
LuaManager.init builds by reflection (LuaJavaClassExposer) for the classes listed in CLASSES, plus every global
function of LuaManager.GlobalObject.

Input is a measurement of the original, not a reading of its code: `sh tools/oracle.sh luaexpose` runs
tests/original_probe/LuaExposeProbe.java (standalone LuaManager.init) into target/oracle/luaexpose.txt. Key order
inside a statics or method table is what HotSpot's getMethods()/getFields() gave in one run on the pinned build: the
original's own order is not stable between runs for some classes (SandboxOptions, IsoZombie, ...), nor is the order
of the globals. Registry constants
(`public static final X NAME = registerBase("Id")`) take their id from the decompiled class.
  python3 tools/gen_lua_expose.py [target/oracle/luaexpose.txt]"""
import pathlib, re, sys

root = pathlib.Path(__file__).resolve().parent.parent
meas = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else root / "target/oracle/luaexpose.txt"

# The classes the game's media/lua/shared reads while loading (measured, docs/COMPATIBILITY.md), the classes of the
# objects it calls methods on, and SandboxOptions/Core. Nested classes come along with their outer class.
CLASSES = """
zombie.characters.animals.AnimalDefinitions zombie.characters.animals.AnimalGenomeDefinitions
zombie.characters.animals.AnimalPartsDefinitions java.util.ArrayList
zombie.characters.AttachedItems.AttachedLocations zombie.characters.AttachedItems.AttachedLocationGroup
zombie.characters.AttachedItems.AttachedLocation zombie.characters.WornItems.BodyLocations
zombie.characters.WornItems.BodyLocationGroup zombie.characters.WornItems.BodyLocation
zombie.characters.BodyDamage.BodyPartType zombie.scripting.objects.CharacterProfession
zombie.scripting.objects.CharacterTrait zombie.debug.DebugType zombie.iso.IsoDirections
zombie.iso.SpriteDetails.IsoFlagType zombie.iso.IsoGridSquare zombie.scripting.objects.ItemBodyLocation
zombie.scripting.objects.ItemKey zombie.scripting.objects.ItemTag zombie.input.KeybindId org.lwjglx.input.Keyboard
zombie.worldMap.symbols.MapSymbolDefinitions zombie.characters.BodyDamage.Metabolics zombie.input.Mouse
zombie.scripting.ScriptManager zombie.iso.Vector2 zombie.SandboxOptions zombie.core.Core zombie.core.textures.ColorInfo
""".split()


def parse_key(s):
    """A key as the probe prints it: an escaped string or a value form. Returns (key, rest)."""
    if s.startswith('"'):
        j = s.index('"', 1)
        return ("s", s[1:j]), s[j + 2:]
    k, _, rest = s.partition(" ")
    return ("v", k), rest


tables = {}  # path -> [(key, value)]
classes = {}  # java name -> (super, ifaces, kind)
methods = {}  # java name -> [(name, value)]
index_meta = {}
for line in meas.read_text(encoding="utf-8").splitlines():
    tag, _, rest = line.partition(" ")
    if tag == "T":
        path, _, rest = rest.partition(" ")
        key, value = parse_key(rest)
        tables.setdefault(path, []).append((key, value))
    elif tag == "C":
        name, sup, ifs, kind = rest.split(" ")
        classes[name] = (sup, ifs, kind)
    elif tag == "M":
        name, m, value = rest.split(" ", 2)
        methods.setdefault(name, []).append((m, value))
    elif tag == "I":
        name, _, v = rest.split(" ")
        index_meta[name] = v

# statics containers: a table whose "class" entry is a class
container = {}  # java name -> path
for path, entries in tables.items():
    for (kt, k), v in entries:
        if kt == "s" and k == "class" and v.startswith("class "):
            container[v[6:]] = path


def table_path(v):
    return v.split(" ", 1)[1] if v.startswith("table ") or v.startswith("ref ") else None


path_class = {p: c for c, p in container.items()}
# nested classes come along with their outer class
want = []
todo = list(CLASSES)
while todo:
    c = todo.pop(0)
    if c in want:
        continue
    if c not in container:
        sys.exit(f"{c}: no statics table in the measurement")
    want.append(c)
    for (kt, k), v in tables[container[c]]:
        p = table_path(v)
        if p and p in path_class and path_class[p].startswith(c + "$"):
            todo.append(path_class[p])

globals_of = {}  # container path -> global key
for (kt, k), v in tables["_G"]:
    p = table_path(v)
    if kt == "s" and p in path_class:
        globals_of.setdefault(p, k)


def decompiled(c):
    outer = c.split("$")[0]
    f = root / "decompiled" / (outer.replace(".", "/") + ".java")
    if not f.exists():
        return None
    src = f.read_text(encoding="utf-8")
    for nested in c.split("$")[1:]:
        m = re.search(r"\n(\s*)public (?:static )?(?:final )?class %s \{" % re.escape(nested), src)
        if not m:
            return None
        end = src.index("\n" + m.group(1) + "}", m.end())
        src = src[m.end():end]
    return src


def rs(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


java_classes = []


def cid(n):
    if n not in java_classes:
        java_classes.append(n)
    return java_classes.index(n)


for c in want:
    cid(c)
out_classes = []
for c in want:
    src = decompiled(c)
    statics = []
    for (kt, k), v in tables[container[c]]:
        if kt != "s":
            sys.exit(f"{c}: non-string static key {k}")
        if v.startswith("class "):
            s = "S::Class"
        elif v.startswith("invoker "):
            s = "S::Method(1)"
        elif v.startswith("multi "):
            s = "S::Method(%s)" % v.split()[1]
        elif v.startswith("enum "):
            _, cls, name, ordinal = v.split()
            s = "S::Enum(%d, %s, %s)" % (cid(cls), rs(name), ordinal)
        elif v.startswith("object "):
            # a lambda's class name carries an address that changes between runs
            cls = re.sub(r"\$\$Lambda/0x[0-9a-f]+$", "$$Lambda", v[7:])
            m = src and re.search(r"public static final \S+ %s = \w+\(\s*\"((?:[^\"\\]|\\.)*)\"" % re.escape(k), src)
            s = "S::Object(%d, %s)" % (cid(cls), rs(m.group(1)) if m else '""')
        elif v.startswith("d"):
            s = "S::Number(0x%s)" % v[1:]
        elif v.startswith('s"'):
            s = "S::Str(%s)" % rs(v[2:-1].encode().decode("unicode_escape"))
        elif v in ("true", "false"):
            s = "S::Bool(%s)" % v
        elif table_path(v) in path_class:
            s = "S::Nested(%s)" % rs(path_class[table_path(v)])
        else:
            sys.exit(f"{c}.{k}: unsupported static value {v}")
        statics.append("(%s, %s)" % (rs(k), s))
    ms = []
    for m, v in methods.get(c, []):
        n = 1 if v.startswith("invoker ") else int(v.split()[1])
        ms.append("(%s, %d)" % (rs(m), n))
    sup, ifs, kind = classes.get(c, ("java.lang.Object", "-", "class"))
    g = globals_of.get(container[c])
    out_classes.append(
        "    Exposed {\n        class: %d,\n        global: %s,\n        has_metatable: %s,\n        statics: &[%s],\n        methods: &[%s],\n    },"
        % (cid(c), "Some(%s)" % rs(g) if g else "None", "true" if c in classes else "false",
           ", ".join(statics), ", ".join(ms)))

glob = []
for (kt, k), v in tables["_G"]:
    if v.startswith("invoker zombie.Lua.LuaManager$GlobalObject.") or v.startswith("multi "):
        glob.append("(%s, %d)" % (rs(k), 1 if v.startswith("invoker") else int(v.split()[1])))

names = java_classes
lines = [
    "//! Generated by tools/gen_lua_expose.py from a measurement of the original LuaManager.init",
    "//! (tests/original_probe/LuaExposeProbe.java). Do not edit.",
    "#![allow(clippy::all)]",
    "use super::exposer::{Exposed, Static as S};",
    "use crate::se::krka::kahlua::vm::object::{classes::OBJECT, JavaClass};",
    "",
]
lines.append("/// Every class named below (index = the numbers in CLASSES). The superclass chain is not reproduced: each exposed")
lines.append("/// class has its own metatable, so KahluaThread.getClassMetatable stops at the class itself.")
lines.append("pub static JAVA_CLASSES: [JavaClass; %d] = [" % len(names))
for n in names:
    lines.append("    JavaClass { name: %s, superclass: Some(&OBJECT), interfaces: &[] }," % rs(n))
lines += ["];", ""]
lines.append("/// The exposed classes, outer before nested; statics and methods in the original's table order.")
lines.append("pub static CLASSES: [Exposed; %d] = [" % len(out_classes))
lines += out_classes
lines.append("];")
lines.append("")
lines.append("/// LuaManager.GlobalObject's `@LuaMethod(global = true)` functions in environment order, with their overload counts.")
lines.append("pub static GLOBALS: [(&str, u8); %d] = [" % len(glob))
lines += ["    %s," % g for g in glob]
lines.append("];")
dst = root / "crates/zomboid-engine/src/zombie/lua/exposed_table.rs"
dst.write_text("\n".join(lines) + "\n", encoding="utf-8")
print(len(out_classes), "classes,", sum(len(methods.get(c, [])) for c in want), "methods,", len(glob), "global functions,", len(names), "java classes")
