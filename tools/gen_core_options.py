#!/usr/bin/env python3
"""Generates crates/zomboid-engine/src/zombie/core/core_options_table.rs from the decompiled zombie.core.Core:
the option declarations (`this.newOption(...)` / `this.newFakeOption(...)` fields, in declaration order) and the
Lua-exposed accessors whose whole body is `return this.<option>.getValue();` or `this.<option>.setValue(<arg>);`.

    python3 tools/gen_core_options.py [decompiled/zombie/core/Core.java]

The exposed method list is read from crates/zomboid-engine/src/zombie/lua/exposed_table.rs (the measured one).
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
src_path = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "decompiled/zombie/core/Core.java"
src = src_path.read_text()
exposed = (ROOT / "crates/zomboid-engine/src/zombie/lua/exposed_table.rs").read_text()


def balanced(text, i):
    """text[i] == '(': index just past the matching ')'."""
    depth = 0
    in_str = False
    j = i
    while j < len(text):
        c = text[j]
        if in_str:
            if c == "\\":
                j += 1
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
        elif c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    raise ValueError("unbalanced")


def split_args(s):
    out, depth, in_str, cur = [], 0, False, ""
    i = 0
    while i < len(s):
        c = s[i]
        if in_str:
            cur += c
            if c == "\\":
                i += 1
                cur += s[i]
            elif c == '"':
                in_str = False
        elif c == '"':
            in_str = True
            cur += c
        elif c in "({":
            depth += 1
            cur += c
        elif c in ")}":
            depth -= 1
            cur += c
        elif c == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += c
        i += 1
    if cur.strip():
        out.append(cur.strip())
    return out


def rs_str(s):
    return s  # Java string literals are valid Rust for the characters used here


def rs_num(s, kind):
    s = s.strip()
    if kind == "d":
        return s if "." in s else s + ".0"
    return s


def element(expr):
    m = re.fullmatch(r"new (\w+)ConfigOption\((.*)\)", expr, re.S)
    kind, args = m.group(1), split_args(m.group(2))
    assert args[0] == '"element"'
    if kind == "Double":
        return f"Opt::double({args[0]}, {rs_num(args[1], 'd')}, {rs_num(args[2], 'd')}, {rs_num(args[3], 'd')})"
    if kind == "Integer":
        return f"Opt::int({args[0]}, {args[1]}, {args[2]}, {args[3]})"
    if kind == "Boolean":
        return f"Opt::boolean({args[0]}, {args[1]})"
    if kind == "String":
        return f"Opt::string_max({args[0]}, {args[1]}, {args[2]})"
    raise ValueError(kind)


decl_re = re.compile(r"private (?:final )?(\w+ConfigOption) (\w+) = this\.(newOption|newFakeOption)\(")
decls = []  # (field, kind, fake, rust expr)
kinds = {}
for m in decl_re.finditer(src):
    kind, field, fn = m.group(1), m.group(2), m.group(3)
    open_i = m.end() - 1
    close_i = balanced(src, open_i)
    args = split_args(src[open_i + 1 : close_i - 1])
    rest = src[close_i:]
    chain = []
    while True:
        cm = re.match(r"\s*\.(setFixedSize|setMultiLine)\(([^)]*)\)", rest)
        if not cm:
            break
        chain.append((cm.group(1), cm.group(2)))
        rest = rest[cm.end():]
    assert rest.lstrip().startswith(";"), (field, rest[:40])
    fake = fn == "newFakeOption"
    name = args[0]
    callback = False
    if kind == "BooleanConfigOption":
        expr = f"Opt::boolean({name}, {args[1]})"
    elif kind == "DoubleConfigOption":
        expr = f"Opt::double({name}, {rs_num(args[1], 'd')}, {rs_num(args[2], 'd')}, {rs_num(args[3], 'd')})"
    elif kind == "IntegerConfigOption":
        expr = f"Opt::int({name}, {args[1]}, {args[2]}, {args[3]})"
        callback = len(args) > 4
    elif kind == "StringConfigOption":
        a2 = args[2]
        if a2.startswith("new String[]"):
            vals = ", ".join(split_args(a2[a2.index("{") + 1 : a2.rindex("}")]))
            expr = f"Opt::string_values({name}, {args[1]}, &[{vals}])"
        elif a2.startswith("this::"):
            expr = f"Opt::string_callback({name}, {args[1]})"
            callback = True
        else:
            expr = f"Opt::string_max({name}, {args[1]}, {a2})"
    elif kind == "ArrayConfigOption":
        expr = f"Opt::array({name}, {element(args[1])}, {args[2]}, {args[3]})"
    else:
        raise ValueError(kind)
    for what, a in chain:
        if what == "setFixedSize":
            expr += f".fixed_size({a})"
        else:
            expr += ".multi_line()" if a.strip() == "true" else ""
    kinds[field] = (kind, callback)
    decls.append((field, fake, expr))

n_calls = len(re.findall(r"= this\.new(?:Fake)?Option\(", src))
assert n_calls == len(decls), (n_calls, len(decls))

# exposed Core methods
m = re.search(r'global: Some\("Core"\),.*?methods: &\[(.*?)\],\n    \},', exposed, re.S)
methods = re.findall(r'\("(\w+)", \d+\)', m.group(1))

JT = {"int": "i", "boolean": "z", "double": "d", "float": "f", "String": "s"}
accessors = []
for name in methods:
    mm = re.search(r"public (?:static )?([\w<>\[\]]+) %s\(([^)]*)\) \{\n(.*?)\n   \}\n" % re.escape(name), src, re.S)
    if not mm:
        continue
    ret, params, body = mm.group(1), mm.group(2).strip(), mm.group(3).strip()
    g = re.fullmatch(r"return (\((?:float|double)\))?this\.(\w+)\.getValue\(\);", body)
    if g and not params and g.group(2) in kinds and ret in JT:
        field = g.group(2)
        k = kinds[field][0]
        ok = (
            (k == "IntegerConfigOption" and ret in ("int", "float", "double"))
            or (k == "BooleanConfigOption" and ret == "boolean")
            or (k == "DoubleConfigOption" and ret in ("double", "float"))
            or (k == "StringConfigOption" and ret == "String")
        )
        if ok:
            accessors.append((name, field, "get", JT[ret]))
        continue
    s = re.fullmatch(r"this\.(\w+)\.setValue\((\w+)\);", body)
    if s and s.group(1) in kinds and ret == "void" and len(params.split()) == 2 and s.group(2) == params.split()[1]:
        field = s.group(1)
        k, cb = kinds[field]
        pt = params.split()[0]
        ok = (
            (k == "IntegerConfigOption" and pt == "int")
            or (k == "BooleanConfigOption" and pt == "boolean")
            or (k == "DoubleConfigOption" and pt == "double")
            or (k == "StringConfigOption" and pt == "String")
        )
        if ok and not cb:
            accessors.append((name, field, "set", JT[pt]))

out = []
out.append("//! Generated by tools/gen_core_options.py from the decompiled zombie.core.Core. Do not edit.")
out.append("//!")
out.append("//! `declare`: the `this.newOption(..)` / `this.newFakeOption(..)` fields in declaration order.")
out.append("use super::core_options::{CoreOptions, Opt};")
out.append("")
out.append("/// zombie.core.Core's field initialisers for the options, in order.")
out.append("pub fn declare(o: &mut CoreOptions) {")
for field, fake, expr in decls:
    out.append(f'    o.add("{field}", {"true" if fake else "false"}, {expr});')
out.append("}")
out.append("")
dest = ROOT / "crates/zomboid-engine/src/zombie/core/core_options_table.rs"
dest.write_text("\n".join(out))

acc = []
acc.append("//! Generated by tools/gen_core_options.py from the decompiled zombie.core.Core. Do not edit.")
acc.append("//!")
acc.append("//! The Lua-exposed Core methods whose whole body is `return this.<option>.getValue();` or")
acc.append("//! `this.<option>.setValue(<arg>);`, each as a call of `exposed_core::get` / `exposed_core::set` with the option")
acc.append("//! field and the Java type (i int, z boolean, d double, f float, s String).")
acc.append("use super::exposed_core::{get, set};")
acc.append("use super::exposer::Imp;")
acc.append("")
acc.append("/// The accessor of Core method `name`.")
acc.append("pub fn accessor(name: &str) -> Option<Imp> {")
acc.append("    Some(match name {")
for name, field, what, t in accessors:
    fn = "get" if what == "get" else "set"
    acc.append(f'        "{name}" => |s, _, cf, n| {fn}(s, cf, n, "{field}", \'{t}\'),')
acc.append("        _ => return None,")
acc.append("    })")
acc.append("}")
acc.append("")
dest2 = ROOT / "crates/zomboid-engine/src/zombie/lua/exposed_core_accessors.rs"
dest2.write_text("\n".join(acc))
print(f"{len(decls)} options, {len(accessors)} accessors -> {dest}, {dest2}")
