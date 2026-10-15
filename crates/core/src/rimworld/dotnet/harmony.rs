use super::{
    compressed, il, u32_at, Assembly, CUSTOM_ATTRIBUTE, MEMBER_REF, METHOD_DEF, TYPE_DEF, TYPE_REF,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Does {
    pub prefix: bool,
    pub postfix: bool,
    pub transpiler: bool,
    pub finalizer: bool,
}

impl Does {
    pub fn with(self, other: Does) -> Does {
        Does {
            prefix: self.prefix || other.prefix,
            postfix: self.postfix || other.postfix,
            transpiler: self.transpiler || other.transpiler,
            finalizer: self.finalizer || other.finalizer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    /// Empty if the attributes name no type.
    pub target: String,
    /// As the patched assembly names it: `get_Label` for a property's reading,
    /// `.ctor` for a constructor. Empty when none is named.
    pub method: String,
    pub does: Does,
}

#[derive(Debug, Clone, Default)]
struct Named {
    target: String,
    method: String,
    /// Harmony's `MethodType`: 0 method, 1 and 2 property get and set, 3 and 4 ctor and cctor.
    kind: usize,
}

impl Named {
    fn complete(&mut self, other: &Named) {
        if !other.target.is_empty() {
            self.target.clone_from(&other.target);
        }
        if !other.method.is_empty() {
            self.method.clone_from(&other.method);
        }
        self.kind = self.kind.max(other.kind);
    }

    fn patch(self, does: Does) -> Patch {
        let method = match self.kind {
            1 => format!("get_{}", self.method),
            2 => format!("set_{}", self.method),
            3 => ".ctor".to_string(),
            4 => ".cctor".to_string(),
            _ => self.method,
        };
        Patch {
            target: self.target,
            method,
            does,
        }
    }
}

/// A length-prefixed string; the single byte 0xFF is an absent one.
fn ser_string(bytes: &[u8]) -> Option<(String, &[u8])> {
    if bytes.first() == Some(&0xff) {
        return Some((String::new(), &bytes[1..]));
    }
    let (length, used) = compressed(bytes)?;
    let text = bytes.get(used..used + length)?;
    Some((
        String::from_utf8_lossy(text).into_owned(),
        &bytes[used + length..],
    ))
}

impl<'a> Assembly<'a> {
    fn user_string(&self, index: usize) -> Option<String> {
        let bytes = self.user_strings.get(index..)?;
        let (length, used) = compressed(bytes)?;
        // UTF-16, then one byte that is not part of the text.
        let text = bytes.get(used..used + length.checked_sub(1)?)?;
        let units = text
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        String::from_utf16(&units.collect::<Vec<u16>>()).ok()
    }

    pub fn looked_up(&self) -> Vec<(String, String)> {
        let methods = 1..=self.rows[METHOD_DEF];
        methods
            .flat_map(|method| self.looked_up_in(method))
            .collect()
    }

    fn looked_up_in(&self, method: usize) -> Vec<(String, String)> {
        let found = self.look_ups_in(method).into_iter();
        found
            .map(|(_, type_name, member)| (type_name, member))
            .collect()
    }

    /// The `bool` says whether the type is this assembly's own.
    fn look_ups_in(&self, method: usize) -> Vec<(bool, String, String)> {
        let row = |token: u32| (token & 0x00ff_ffff) as usize;
        let table = |token: u32| (token >> 24) as usize;
        let code = self.body(method).unwrap_or_default();
        let all: Vec<il::Instruction> = il::instructions(code).collect();
        // ldtoken <type>; call Type.GetTypeFromHandle; ldstr <text>.
        let fits = |w: &&[il::Instruction]| {
            w[0].code == il::LDTOKEN
                && w[1].code == il::CALL
                && w[2].code == il::LDSTR
                && table(w[1].operand) == MEMBER_REF
                && self.name(MEMBER_REF, row(w[1].operand), 1) == Some("GetTypeFromHandle")
        };
        let found = |w: &[il::Instruction]| {
            let of = (table(w[0].operand), row(w[0].operand));
            let member = self.user_string(row(w[2].operand))?;
            Some((of.0 == TYPE_DEF, self.type_name(of)?, member))
        };
        all.windows(3).filter(fits).filter_map(found).collect()
    }

    /// Kinds come from this assembly's own methods, named after the kind, like `prefix`.
    pub fn hand_applied(&self) -> Vec<Patch> {
        let part = |name: &str| {
            let name = name.to_lowercase();
            Does {
                prefix: name.contains("prefix"),
                postfix: name.contains("postfix"),
                transpiler: name.contains("transpiler"),
                finalizer: name.contains("finalizer"),
            }
        };
        let mut found: Vec<Patch> = Vec::new();
        for method in 1..=self.rows[METHOD_DEF] {
            let mut aimed = false;
            for (own, target, member) in self.look_ups_in(method) {
                if !own {
                    let does = Does::default();
                    found.push(Patch {
                        target,
                        method: member,
                        does,
                    });
                    aimed = true;
                } else if let (true, Some(last)) = (aimed, found.last_mut()) {
                    last.does = last.does.with(part(&member));
                }
            }
        }
        found.retain(|p| p.does != Does::default());
        found
    }

    fn attribute(&self, row: usize) -> Option<(&'a str, usize)> {
        let (table, constructor) = self.coded(CUSTOM_ATTRIBUTE, row, 1)?;
        if table != MEMBER_REF {
            return None;
        }
        let (owner_table, owner) = self.coded(MEMBER_REF, constructor, 0)?;
        (owner_table == TYPE_REF).then_some(())?;
        Some((self.name(TYPE_REF, owner, 1)?, constructor))
    }

    fn patch_target(&self, row: usize, constructor: usize) -> Option<Named> {
        let signature = self.blob(MEMBER_REF, constructor, 2)?;
        let (count, used) = compressed(signature.get(1..)?)?;
        // After the calling convention and the count: the return type
        // (void, one byte), then each parameter's type.
        let mut types = signature.get(1 + used + 1..)?;
        let mut value = self.blob(CUSTOM_ATTRIBUTE, row, 2)?.get(2..)?;
        let mut named = Named::default();
        for _ in 0..count {
            match *types.first()? {
                // A class: of the constructors' parameters only System.Type is one.
                0x12 => {
                    let (_, token) = compressed(types.get(1..)?)?;
                    types = types.get(1 + token..)?;
                    let (name, rest) = ser_string(value)?;
                    // The type name ends at the first `,` or `[` (assembly, type arguments).
                    let end = name.find([',', '[']).unwrap_or(name.len());
                    named.target = name[..end].to_string();
                    value = rest;
                }
                0x0e => {
                    types = &types[1..];
                    (named.method, value) = ser_string(value)?;
                }
                // An enum, four bytes: Harmony's `MethodType` says which
                // kind of method the name stands for.
                0x11 => {
                    let (coded, token) = compressed(types.get(1..)?)?;
                    types = types.get(1 + token..)?;
                    if coded & 3 == 1 && self.name(TYPE_REF, coded >> 2, 1) == Some("MethodType") {
                        named.kind = u32_at(value, 0)?;
                    }
                    value = value.get(4..)?;
                }
                // An array of parameter types comes last and is not read.
                _ => break,
            }
        }
        Some(named)
    }

    /// A patch applied by hand, with a call to `Harmony.Patch`, is not found
    /// here: see `hand_applied`.
    pub fn patches(&self) -> Vec<Patch> {
        let mut on_type: Vec<(usize, Named)> = Vec::new();
        let mut on_method: Vec<(usize, Named)> = Vec::new();
        let mut marked: Vec<(usize, &str)> = Vec::new();
        for row in 1..=self.rows[CUSTOM_ATTRIBUTE] {
            let Some((name, constructor)) = self.attribute(row) else {
                continue;
            };
            let Some((table, parent)) = self.coded(CUSTOM_ATTRIBUTE, row, 0) else {
                continue;
            };
            match (name, table) {
                // Also a mod's stand-in with the same arguments, such as `RocketPatch`.
                (patch, TYPE_DEF | METHOD_DEF) if patch.ends_with("Patch") => {
                    let named = self.patch_target(row, constructor).unwrap_or_default();
                    let list = if table == TYPE_DEF {
                        &mut on_type
                    } else {
                        &mut on_method
                    };
                    list.push((parent, named));
                }
                (
                    "HarmonyPrefix" | "HarmonyPostfix" | "HarmonyTranspiler" | "HarmonyFinalizer",
                    METHOD_DEF,
                ) => {
                    marked.push((parent, name));
                }
                _ => {}
            }
        }
        let does = |method: usize, into: &mut Does| {
            let said = marked
                .iter()
                .filter(|(m, _)| *m == method)
                .map(|(_, a)| &a[7..]);
            for kind in said.chain(self.name(METHOD_DEF, method, 3)) {
                match kind {
                    "Prefix" => into.prefix = true,
                    "Postfix" => into.postfix = true,
                    "Transpiler" => into.transpiler = true,
                    "Finalizer" => into.finalizer = true,
                    _ => {}
                }
            }
        };
        let mut found = Vec::new();
        let patch_classes: std::collections::BTreeSet<usize> =
            on_type.iter().map(|t| t.0).collect();
        for class in patch_classes {
            // Several attributes on a class add up to one target.
            let mut whole = Named::default();
            for (_, part) in on_type.iter().filter(|t| t.0 == class) {
                whole.complete(part);
            }
            let methods = self.methods_of(class);
            let own: Vec<&(usize, Named)> = on_method
                .iter()
                .filter(|m| methods.contains(&m.0))
                .collect();
            if own.is_empty() {
                let mut all = Does::default();
                methods.clone().for_each(|m| does(m, &mut all));
                // No target in the attributes: `TargetMethod` looks it up in code.
                let in_code: Vec<(String, String)> = match whole.target.is_empty() {
                    true => methods.flat_map(|m| self.looked_up_in(m)).collect(),
                    false => Vec::new(),
                };
                if in_code.is_empty() {
                    found.push(whole.patch(all));
                }
                for (target, method) in in_code {
                    found.push(Patch {
                        target,
                        method,
                        does: all,
                    });
                }
                continue;
            }
            for (m, part) in own {
                let mut one = Does::default();
                does(*m, &mut one);
                let mut aim = whole.clone();
                aim.complete(part);
                found.push(aim.patch(one));
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::super::build::{ser, Builder};
    use super::*;

    /// `[HarmonyPatch(typeof(Verse.Pawn), "Tick")] class P { Prefix; Transpiler }`
    #[test]
    fn a_patch_class_names_its_target_and_what_it_does_to_it() {
        let mut b = Builder::new();
        let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
        let system_type = b.type_ref("System", "Type");
        let class = (system_type << 2 | 1) as u8;
        let with_type_and_name = b.constructor(patch, &[0x20, 2, 0x01, 0x12, class, 0x0e]);
        let p = b.type_def("My", "Pawn_Tick_Patch", 0);
        b.method("Prefix", &[0x00, 0, 0x02]);
        b.method("Transpiler", &[0x00, 0, 0x01]);
        let mut value = vec![1, 0];
        value.extend(ser("Verse.Pawn, Assembly-CSharp, Version=0.0.0.0"));
        value.extend(ser("Tick"));
        value.extend([0, 0]);
        b.attribute(false, p, with_type_and_name, &value);
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().patches();
        assert_eq!(
            patches,
            [Patch {
                target: "Verse.Pawn".into(),
                method: "Tick".into(),
                does: Does {
                    prefix: true,
                    transpiler: true,
                    ..Does::default()
                },
            }]
        );
    }

    /// `[HarmonyPatch(typeof(ThingOwner<Thing>), "Count", MethodType.Getter)]`
    /// and `[HarmonyPatch(typeof(Pawn), MethodType.Constructor)]`.
    #[test]
    fn a_patch_on_a_property_or_a_constructor_names_the_method_behind_it() {
        let mut b = Builder::new();
        let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
        let system_type = b.type_ref("System", "Type");
        let method_type = b.type_ref("HarmonyLib", "MethodType");
        let (class, kind) = ((system_type << 2 | 1) as u8, (method_type << 2 | 1) as u8);
        let property = b.constructor(patch, &[0x20, 3, 0x01, 0x12, class, 0x0e, 0x11, kind]);
        let constructor = b.constructor(patch, &[0x20, 2, 0x01, 0x12, class, 0x11, kind]);
        let getter = b.type_def("My", "Count_Patch", 0);
        b.method("Postfix", &[0x00, 0, 0x01]);
        let maker = b.type_def("My", "Pawn_Patch", 0);
        b.method("Postfix", &[0x00, 0, 0x01]);
        let mut value = vec![1, 0];
        value.extend(ser(
            "Verse.ThingOwner`1[[Verse.Thing, Assembly-CSharp]], Assembly-CSharp",
        ));
        value.extend(ser("Count"));
        value.extend([1, 0, 0, 0, 0, 0]);
        b.attribute(false, getter, property, &value);
        let mut value = vec![1, 0];
        value.extend(ser("Verse.Pawn, Assembly-CSharp"));
        value.extend([3, 0, 0, 0, 0, 0]);
        b.attribute(false, maker, constructor, &value);
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().patches();
        let aimed: Vec<(&str, &str)> = patches
            .iter()
            .map(|p| (p.target.as_str(), p.method.as_str()))
            .collect();
        assert_eq!(
            aimed,
            [("Verse.ThingOwner`1", "get_Count"), ("Verse.Pawn", ".ctor")]
        );
    }

    #[test]
    fn attributes_on_methods_complete_the_one_on_their_class() {
        let mut b = Builder::new();
        let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
        let postfix = b.type_ref("HarmonyLib", "HarmonyPostfix");
        let system_type = b.type_ref("System", "Type");
        let class = (system_type << 2 | 1) as u8;
        let with_type = b.constructor(patch, &[0x20, 1, 0x01, 0x12, class]);
        let with_name = b.constructor(patch, &[0x20, 1, 0x01, 0x0e]);
        let is_postfix = b.constructor(postfix, &[0x20, 0, 0x01]);
        let p = b.type_def("My", "Patches", 0);
        let first = b.method("AfterDraw", &[0x00, 0, 0x01]);
        let second = b.method("Prefix", &[0x00, 0, 0x02]);
        b.method("Helper", &[0x00, 0, 0x01]);
        let mut value = vec![1, 0];
        value.extend(ser("RimWorld.ColonistBar"));
        b.attribute(false, p, with_type, &[value, vec![0, 0]].concat());
        for (method, name) in [(first, "ColonistBarOnGUI"), (second, "MarkColonistsDirty")] {
            b.attribute(
                true,
                method,
                with_name,
                &[vec![1, 0], ser(name), vec![0, 0]].concat(),
            );
        }
        b.attribute(true, first, is_postfix, &[1, 0, 0, 0]);
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().patches();
        let seen: Vec<(&str, &str, bool, bool)> = patches
            .iter()
            .map(|p| {
                (
                    p.target.as_str(),
                    p.method.as_str(),
                    p.does.prefix,
                    p.does.postfix,
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("RimWorld.ColonistBar", "ColonistBarOnGUI", false, true),
                ("RimWorld.ColonistBar", "MarkColonistsDirty", true, false),
            ]
        );
    }

    /// `AccessTools.Method(typeof(Verse.Pawn), "Tick")`, as compiled.
    #[test]
    fn a_member_looked_up_by_name_is_found_in_the_code() {
        let mut b = Builder::new();
        let pawn = b.type_ref("Verse", "Pawn");
        let system_type = b.type_ref("System", "Type");
        let from_handle = b.member_ref(system_type, "GetTypeFromHandle");
        let other = b.member_ref(system_type, "GetType");
        let tick = b.user_string("Tick");
        let token = |row: u32, table: u8| [row as u8, 0, 0, table];
        let lookup = |call: u32| {
            let mut code = vec![0xd0];
            code.extend(token(pawn, 0x01));
            code.push(0x28);
            code.extend(token(call, 0x0a));
            code.push(0x72);
            code.extend(token(tick, 0x70));
            code
        };
        b.type_def("My", "Patches", 0);
        b.method_with("Apply", 0, &[vec![0x00], lookup(from_handle)].concat());
        b.method_with("Other", 0, &lookup(other));
        let dll = b.finish();
        assert_eq!(
            Assembly::read(&dll).unwrap().looked_up(),
            [("Verse.Pawn".to_string(), "Tick".to_string())]
        );
    }

    /// `[HarmonyPatch] class P { static MethodBase TargetMethod() =>
    /// AccessTools.Method(typeof(Verse.Pawn), "Tick"); static void Postfix() {} }`
    #[test]
    fn a_class_that_names_its_target_in_code_aims_at_what_it_looks_up() {
        use super::super::build::op;
        let mut b = Builder::new();
        let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
        let bare = b.constructor(patch, &[0x20, 0, 0x01]);
        let pawn = b.type_ref("Verse", "Pawn");
        let system_type = b.type_ref("System", "Type");
        let from_handle = b.member_ref(system_type, "GetTypeFromHandle");
        let tick = b.user_string("Tick");
        let class = b.type_def("My", "Patch", 0);
        let lookup = [
            op(il::LDTOKEN, TYPE_REF, pawn),
            op(il::CALL, MEMBER_REF, from_handle),
            op(il::LDSTR, 0x70, tick),
        ]
        .concat();
        b.method_with("TargetMethod", 0, &lookup);
        b.method_with("Postfix", 0, &[0x2a]);
        b.attribute(false, class, bare, &[1, 0, 0, 0]);
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().patches();
        assert_eq!(patches.len(), 1);
        let p = &patches[0];
        assert_eq!(
            (p.target.as_str(), p.method.as_str()),
            ("Verse.Pawn", "Tick")
        );
        assert!(p.does.postfix && !p.does.prefix);
    }

    #[test]
    fn a_patch_applied_by_hand_says_what_it_does_by_the_methods_named_after_its_target() {
        use super::super::build::op;
        let mut b = Builder::new();
        let pawn = b.type_ref("Verse", "Pawn");
        let system_type = b.type_ref("System", "Type");
        let from_handle = b.member_ref(system_type, "GetTypeFromHandle");
        let (tick, kill) = (b.user_string("Tick"), b.user_string("Kill"));
        let (before, after) = (b.user_string("Tick_Prefix"), b.user_string("AfterKill"));
        let own = b.type_def("My", "Patches", 0);
        let lookup = |table: usize, of: u32, text: u32| {
            [
                op(il::LDTOKEN, table, of),
                op(il::CALL, MEMBER_REF, from_handle),
                op(il::LDSTR, 0x70, text),
            ]
            .concat()
        };
        b.method_with(
            "Apply",
            0,
            &[lookup(TYPE_REF, pawn, tick), lookup(TYPE_DEF, own, before)].concat(),
        );
        b.method_with(
            "More",
            0,
            &[lookup(TYPE_REF, pawn, kill), lookup(TYPE_DEF, own, after)].concat(),
        );
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().hand_applied();
        assert_eq!(patches.len(), 1);
        assert_eq!(
            (patches[0].target.as_str(), patches[0].method.as_str()),
            ("Verse.Pawn", "Tick")
        );
        assert!(patches[0].does.prefix && !patches[0].does.postfix);
    }

    #[test]
    fn an_attribute_cut_short_still_gives_a_patch_with_what_was_read() {
        let mut b = Builder::new();
        let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
        let with_name = b.constructor(patch, &[0x20, 1, 0x01, 0x0e]);
        let p = b.type_def("My", "Broken", 0);
        b.method("Postfix", &[0x00, 0, 0x01]);
        b.attribute(false, p, with_name, &[1, 0, 20]);
        let dll = b.finish();
        let patches = Assembly::read(&dll).unwrap().patches();
        assert_eq!(patches.len(), 1);
        assert_eq!(
            (patches[0].target.as_str(), patches[0].method.as_str()),
            ("", "")
        );
        assert!(patches[0].does.postfix);
    }
}
