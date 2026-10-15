use super::{
    compressed, il, u16_at, u32_at, Assembly, FIELD, INTERFACE_IMPL, MEMBER_REF, METHOD_DEF,
    METHOD_IMPL, METHOD_SPEC, TYPE_DEF, TYPE_REF, TYPE_SPEC,
};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base {
    None,
    Own(usize),
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub name: String,
    pub extends: Base,
    pub implements: Vec<usize>,
    pub methods: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodDef<'a> {
    pub owner: usize,
    pub name: &'a str,
    /// The signature blob as stored: equal signatures take the same types.
    pub signature: &'a [u8],
    flags: usize,
}

impl MethodDef<'_> {
    pub fn is_virtual(&self) -> bool {
        self.flags & 0x40 != 0
    }

    pub fn is_new(&self) -> bool {
        self.flags & 0x100 != 0
    }

    pub fn is_final(&self) -> bool {
        self.flags & 0x20 != 0
    }
}

/// In file order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout<'a> {
    pub types: Vec<TypeDef>,
    pub methods: Vec<MethodDef<'a>>,
    pub stand_ins: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    Own(usize),
    /// A member of another assembly: ask `Assembly::member` for its name.
    Other(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Direct,
    Virtual,
    New,
    Handed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Call(How, Callee),
    Keeps { field: usize, method: usize },
    Reads(usize),
}

impl<'a> Assembly<'a> {
    fn generic_base(&self, type_spec: usize) -> Option<(usize, usize)> {
        let blob = self.blob(TYPE_SPEC, type_spec, 0)?;
        // GENERICINST, then CLASS or VALUETYPE, then the type.
        if blob.first() != Some(&0x15) || !matches!(blob.get(1), Some(0x11 | 0x12)) {
            return None;
        }
        let (coded, _) = compressed(blob.get(2..)?)?;
        let table = *[TYPE_DEF, TYPE_REF].get(coded & 3)?;
        Some((table, coded >> 2))
    }

    fn plain_type(&self, to: (usize, usize)) -> Option<(usize, usize)> {
        match to.0 {
            TYPE_SPEC => self.generic_base(to.1),
            _ => Some(to),
        }
    }

    pub fn layout(&self) -> Layout<'a> {
        let base = |row: usize| {
            let to = self.coded(TYPE_DEF, row, 3).filter(|to| to.1 != 0);
            match to.and_then(|to| self.plain_type(to)) {
                Some((TYPE_DEF, own)) => own.checked_sub(1).map_or(Base::None, Base::Own),
                Some(other) => self.type_name(other).map_or(Base::None, Base::Other),
                None => Base::None,
            }
        };
        let mut types: Vec<TypeDef> = (1..=self.rows[TYPE_DEF])
            .map(|row| {
                let methods = self.methods_of(row);
                TypeDef {
                    name: self.type_name((TYPE_DEF, row)).unwrap_or_default(),
                    extends: base(row),
                    implements: Vec::new(),
                    methods: methods.start - 1..methods.end - 1,
                }
            })
            .collect();
        for row in 1..=self.rows[INTERFACE_IMPL] {
            let class = self.cell(INTERFACE_IMPL, row, 0).unwrap_or(0);
            let interface = self.coded(INTERFACE_IMPL, row, 1);
            let own = interface.and_then(|to| self.plain_type(to));
            if let (Some(class), Some((TYPE_DEF, interface))) =
                (types.get_mut(class.wrapping_sub(1)), own)
            {
                if let Some(interface) = interface.checked_sub(1) {
                    class.implements.push(interface);
                }
            }
        }
        let mut methods = Vec::with_capacity(self.rows[METHOD_DEF]);
        for (owner, t) in types.iter().enumerate() {
            for row in t.methods.clone().map(|m| m + 1) {
                methods.push(MethodDef {
                    owner,
                    name: self.name(METHOD_DEF, row, 3).unwrap_or_default(),
                    signature: self.blob(METHOD_DEF, row, 4).unwrap_or_default(),
                    flags: self.cell(METHOD_DEF, row, 2).unwrap_or(0),
                });
            }
        }
        let own_method = |to: Option<(usize, usize)>| match to {
            Some((METHOD_DEF, row)) if row >= 1 && row <= methods.len() => Some(row - 1),
            _ => None,
        };
        let stand_ins = (1..=self.rows[METHOD_IMPL])
            .filter_map(|row| {
                let runs = own_method(self.coded(METHOD_IMPL, row, 1))?;
                let declared = own_method(self.coded(METHOD_IMPL, row, 2))?;
                Some((declared, runs))
            })
            .collect();
        Layout {
            types,
            methods,
            stand_ins,
        }
    }

    pub(super) fn body(&self, method_row: usize) -> Option<&'a [u8]> {
        let rva = self
            .cell(METHOD_DEF, method_row, 0)
            .filter(|rva| *rva != 0)?;
        let at = self.to_file(rva)?;
        let first = *self.dll.get(at)? as usize;
        match first & 3 {
            // Small: the size is in the same byte.
            2 => self.dll.get(at + 1..at + 1 + (first >> 2)),
            // Large: flags and the header's size in four-byte words, the
            // stack it needs, then the size of the code.
            3 => {
                let header = (u16_at(self.dll, at)? >> 12) * 4;
                let size = u32_at(self.dll, at + 4)?;
                self.dll.get(at + header..(at + header).checked_add(size)?)
            }
            _ => None,
        }
    }

    pub fn member(&self, member_ref: usize) -> Option<(String, &'a str)> {
        let owner = self.plain_type(self.coded(MEMBER_REF, member_ref, 0)?)?;
        let owner = Some(owner).filter(|to| to.0 == TYPE_REF)?;
        Some((
            self.type_name(owner)?,
            self.name(MEMBER_REF, member_ref, 1)?,
        ))
    }

    /// `Own` when the reference goes through one of this assembly's types.
    fn member_callee(&self, layout: &Layout, member_ref: usize) -> Option<Callee> {
        let own_type = match self.coded(MEMBER_REF, member_ref, 0)? {
            (METHOD_DEF, row) => return Some(Callee::Own(row.checked_sub(1)?)),
            to => self.plain_type(to).filter(|to| to.0 == TYPE_DEF),
        };
        let Some((_, type_row)) = own_type else {
            return Some(Callee::Other(member_ref));
        };
        let name = self.name(MEMBER_REF, member_ref, 1)?;
        let signature = self.blob(MEMBER_REF, member_ref, 2)?;
        let of_type = || {
            layout
                .types
                .get(type_row.checked_sub(1)?)
                .map(|t| t.methods.clone())
        };
        let named = |m: &usize| layout.methods[*m].name == name;
        let exact = of_type()?
            .filter(named)
            .find(|m| layout.methods[*m].signature == signature);
        exact.or_else(|| of_type()?.find(named)).map(Callee::Own)
    }

    fn callee(&self, layout: &Layout, token: u32) -> Option<Callee> {
        let (table, row) = ((token >> 24) as usize, (token & 0x00ff_ffff) as usize);
        match table {
            METHOD_DEF => (row >= 1 && row <= layout.methods.len()).then(|| Callee::Own(row - 1)),
            MEMBER_REF => self.member_callee(layout, row),
            METHOD_SPEC => match self.coded(METHOD_SPEC, row, 0)? {
                (METHOD_DEF, row) => Some(Callee::Own(row.checked_sub(1)?)),
                (_, row) => self.member_callee(layout, row),
            },
            _ => None,
        }
    }

    pub fn steps(&self, layout: &Layout, method: usize) -> Vec<Step> {
        let code = self.body(method + 1).unwrap_or_default();
        let own_field = |token: u32| {
            let own = token >> 24 == FIELD as u32;
            own.then_some(token as usize & 0x00ff_ffff)
        };
        let handed = |method: usize| Step::Call(How::Handed, Callee::Own(method));
        let mut steps = Vec::new();
        let mut held: Option<usize> = None;
        for i in il::instructions(code) {
            if let Some(method) = held {
                match (i.code, own_field(i.operand)) {
                    // The compiler may `dup` the delegate before storing it.
                    (il::NEWOBJ | il::DUP, _) => continue,
                    (il::STFLD | il::STSFLD, Some(field)) => {
                        steps.push(Step::Keeps { field, method });
                        held = None;
                        continue;
                    }
                    _ => {
                        steps.push(handed(method));
                        held = None;
                    }
                }
            }
            let how = match i.code {
                il::CALL => How::Direct,
                il::CALLVIRT => How::Virtual,
                il::NEWOBJ => How::New,
                il::LDFTN | il::LDVIRTFTN => How::Handed,
                il::LDFLD | il::LDSFLD => {
                    steps.extend(own_field(i.operand).map(Step::Reads));
                    continue;
                }
                _ => continue,
            };
            match (how, self.callee(layout, i.operand)) {
                (How::Handed, Some(Callee::Own(method))) => held = Some(method),
                (how, Some(callee)) => steps.push(Step::Call(how, callee)),
                _ => {}
            }
        }
        steps.extend(held.map(handed));
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::super::build::{op, Builder};
    use super::super::{il, NESTED_CLASS};
    use super::*;

    const NOTHING: &[u8] = &[0x2a];

    #[test]
    fn the_layout_says_what_types_extend_and_implement() {
        let mut b = Builder::new();
        let outside = b.type_ref("Verse", "Thing");
        let interface = b.type_def("My", "IWork", 0);
        b.method_with("Work", 0, &[]);
        let base = b.type_def("My", "Base", outside);
        let derived = b.type_def_under("My", "Derived", base);
        b.method_with("Work", 0, NOTHING);
        b.method_with("My.IWork.Work", 0, NOTHING);
        b.implements(derived, interface);
        b.row(METHOD_IMPL, &[derived, 3 << 1, 1 << 1]);
        let dll = b.finish();
        let layout = Assembly::read(&dll).unwrap().layout();
        let names: Vec<&str> = layout.types.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["My.IWork", "My.Base", "My.Derived"]);
        assert_eq!(layout.types[0].extends, Base::None);
        assert_eq!(layout.types[1].extends, Base::Other("Verse.Thing".into()));
        assert_eq!(layout.types[2].extends, Base::Own(1));
        assert_eq!(layout.types[2].implements, [0]);
        assert_eq!(layout.types[2].methods, 1..3);
        assert_eq!(layout.types[1].methods, 1..1);
        assert_eq!(layout.methods[2].name, "My.IWork.Work");
        assert_eq!(layout.methods[2].owner, 2);
        assert_eq!(layout.stand_ins, [(0, 2)]);
    }

    #[test]
    fn a_nested_type_is_named_inside_the_one_around_it() {
        let mut b = Builder::new();
        let outer = b.type_def("My", "Outer", 0);
        let inner = b.type_def("", "Inner", 0);
        let innermost = b.type_def("", "Innermost", 0);
        b.nested(inner, outer);
        b.nested(innermost, inner);
        let dll = b.finish();
        let layout = Assembly::read(&dll).unwrap().layout();
        assert_eq!(layout.types[2].name, "My.Outer+Inner+Innermost");
    }

    #[test]
    fn a_type_said_to_be_inside_itself_is_still_named() {
        let mut b = Builder::new();
        let alone = b.type_def("My", "Alone", 0);
        b.row(NESTED_CLASS, &[alone, alone]);
        let dll = b.finish();
        let layout = Assembly::read(&dll).unwrap().layout();
        assert!(layout.types[0].name.ends_with("Alone"));
    }

    #[test]
    fn calls_are_told_apart_by_how_they_are_made() {
        let mut b = Builder::new();
        let outside = b.type_ref("Verse", "Widgets");
        let label = b.member_ref(outside, "Label");
        b.type_def("My", "Code", 0);
        let code = [
            op(il::CALL, METHOD_DEF, 2),
            op(il::CALLVIRT, METHOD_DEF, 3),
            op(il::NEWOBJ, METHOD_DEF, 4),
            op(il::CALL, MEMBER_REF, label),
        ]
        .concat();
        b.method_with("Run", 0, &code);
        b.method_with("Plain", 0, NOTHING);
        b.method_with("Open", 0, NOTHING);
        b.method_with(".ctor", 0, NOTHING);
        let dll = b.finish();
        let assembly = Assembly::read(&dll).unwrap();
        let layout = assembly.layout();
        assert_eq!(
            assembly.steps(&layout, 0),
            [
                Step::Call(How::Direct, Callee::Own(1)),
                Step::Call(How::Virtual, Callee::Own(2)),
                Step::Call(How::New, Callee::Own(3)),
                Step::Call(How::Direct, Callee::Other(label as usize)),
            ]
        );
        assert_eq!(
            assembly.member(label as usize),
            Some(("Verse.Widgets".to_string(), "Label"))
        );
        assert_eq!(assembly.steps(&layout, 1), []);
    }

    #[test]
    fn a_method_made_into_a_delegate_is_handed_on_or_kept_in_a_field() {
        let mut b = Builder::new();
        let action = b.type_ref("System", "Action");
        let wrap = b.constructor(action, &[0x20, 0, 0x01]);
        let linq = b.type_ref("System.Linq", "Enumerable");
        let any = b.member_ref(linq, "Any");
        b.type_def("My", "Code", 0);
        let cache = b.field("cache");
        let delegate = [
            op(il::LDFTN, METHOD_DEF, 3),
            op(il::NEWOBJ, MEMBER_REF, wrap),
        ]
        .concat();
        b.method_with(
            "Asks",
            0,
            &[delegate.clone(), op(il::CALL, MEMBER_REF, any)].concat(),
        );
        let keeps = [delegate, vec![il::DUP as u8], op(il::STSFLD, FIELD, cache)].concat();
        b.method_with("Keeps", 0, &keeps);
        b.method_with("Lambda", 0, NOTHING);
        b.method_with("Uses", 0, &op(il::LDSFLD, FIELD, cache));
        let dll = b.finish();
        let assembly = Assembly::read(&dll).unwrap();
        let layout = assembly.layout();
        assert_eq!(
            assembly.steps(&layout, 0),
            [
                Step::Call(How::Handed, Callee::Own(2)),
                Step::Call(How::Direct, Callee::Other(any as usize)),
            ]
        );
        let field = cache as usize;
        assert_eq!(
            assembly.steps(&layout, 1),
            [Step::Keeps { field, method: 2 }]
        );
        assert_eq!(assembly.steps(&layout, 3), [Step::Reads(field)]);
    }

    #[test]
    fn a_call_through_a_generic_type_or_method_reaches_the_method() {
        let mut b = Builder::new();
        b.type_def("My", "Box`1", 0);
        b.method_with("Open", 0, NOTHING);
        b.method_with("Pick", 0, NOTHING);
        let spec_blob = b.blob(&[0x15, 0x12, 1 << 2, 1, 0x1c]);
        let spec = b.row(TYPE_SPEC, &[spec_blob]);
        let (name, signature) = (b.string("Open"), b.blob(&[0x00, 0, 0x01]));
        let through_type = b.row(MEMBER_REF, &[spec << 3 | 4, name, signature]);
        let instance = b.blob(&[0x0a, 1, 0x1c]);
        let through_method = b.row(METHOD_SPEC, &[2 << 1, instance]);
        b.type_def("My", "Code", 0);
        let code = [
            op(il::CALL, MEMBER_REF, through_type),
            op(il::CALL, METHOD_SPEC, through_method),
        ]
        .concat();
        b.method_with("Run", 0, &code);
        let dll = b.finish();
        let assembly = Assembly::read(&dll).unwrap();
        let layout = assembly.layout();
        assert_eq!(
            assembly.steps(&layout, 2),
            [
                Step::Call(How::Direct, Callee::Own(0)),
                Step::Call(How::Direct, Callee::Own(1)),
            ]
        );
    }
}
