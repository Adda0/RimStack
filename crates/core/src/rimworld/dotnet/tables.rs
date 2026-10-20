//! Table layouts from ECMA-335 II.22.

#[derive(Clone, Copy)]
pub(super) enum Col {
    Fixed(usize),
    Str,
    Guid,
    Blob,
    Row(usize),
    /// The low bits of the index hold the table tag.
    Coded(&'static [usize], u32),
}

pub(super) const TYPE_DEF_OR_REF: Col = Col::Coded(&[0x02, 0x01, 0x1b], 2);
pub(super) const HAS_CONSTANT: Col = Col::Coded(&[0x04, 0x08, 0x17], 2);
pub(super) const HAS_CUSTOM_ATTRIBUTE: Col = Col::Coded(
    &[
        0x06, 0x04, 0x01, 0x02, 0x08, 0x09, 0x0a, 0x00, 0x0e, 0x17, 0x14, 0x11, 0x1a, 0x1b, 0x20,
        0x23, 0x26, 0x27, 0x28, 0x2a, 0x2c, 0x2b,
    ],
    5,
);
pub(super) const HAS_FIELD_MARSHAL: Col = Col::Coded(&[0x04, 0x08], 1);
pub(super) const HAS_DECL_SECURITY: Col = Col::Coded(&[0x02, 0x06, 0x20], 2);
pub(super) const MEMBER_REF_PARENT: Col = Col::Coded(&[0x02, 0x01, 0x1a, 0x06, 0x1b], 3);
pub(super) const HAS_SEMANTICS: Col = Col::Coded(&[0x14, 0x17], 1);
pub(super) const METHOD_DEF_OR_REF: Col = Col::Coded(&[0x06, 0x0a], 1);
pub(super) const MEMBER_FORWARDED: Col = Col::Coded(&[0x04, 0x06], 1);
// Two of its five tags are unused; only the size of the largest counts.
pub(super) const CUSTOM_ATTRIBUTE_TYPE: Col = Col::Coded(&[0x06, 0x0a], 3);
pub(super) const RESOLUTION_SCOPE: Col = Col::Coded(&[0x00, 0x1a, 0x23, 0x01], 2);
pub(super) const IMPLEMENTATION: Col = Col::Coded(&[0x26, 0x23, 0x27], 2);
pub(super) const TYPE_OR_METHOD_DEF: Col = Col::Coded(&[0x02, 0x06], 1);

pub(super) const TYPE_REF: usize = 0x01;
pub(super) const TYPE_DEF: usize = 0x02;
pub(super) const FIELD: usize = 0x04;
pub(super) const METHOD_DEF: usize = 0x06;
pub(super) const MEMBER_REF: usize = 0x0a;
pub(super) const INTERFACE_IMPL: usize = 0x09;
pub(super) const CUSTOM_ATTRIBUTE: usize = 0x0c;
pub(super) const METHOD_IMPL: usize = 0x19;
pub(super) const TYPE_SPEC: usize = 0x1b;
pub(super) const ASSEMBLY: usize = 0x20;
pub(super) const ASSEMBLY_REF: usize = 0x23;
pub(super) const NESTED_CLASS: usize = 0x29;
pub(super) const METHOD_SPEC: usize = 0x2b;
pub(super) const WIDEST: usize = 6;

/// Tables after `MethodSpec` are not listed, and nothing reads them.
pub(super) const TABLES: [&[Col]; METHOD_SPEC + 1] = [
    &[Col::Fixed(2), Col::Str, Col::Guid, Col::Guid, Col::Guid], // 00 Module
    &[RESOLUTION_SCOPE, Col::Str, Col::Str],                     // 01 TypeRef
    &[
        Col::Fixed(4),
        Col::Str,
        Col::Str,
        TYPE_DEF_OR_REF,
        Col::Row(0x04),
        Col::Row(METHOD_DEF),
    ], // 02 TypeDef
    &[Col::Row(0x04)],                                           // 03 FieldPtr
    &[Col::Fixed(2), Col::Str, Col::Blob],                       // 04 Field
    &[Col::Row(METHOD_DEF)],                                     // 05 MethodPtr
    &[
        Col::Fixed(4),
        Col::Fixed(2),
        Col::Fixed(2),
        Col::Str,
        Col::Blob,
        Col::Row(0x08),
    ], // 06 MethodDef
    &[Col::Row(0x08)],                                           // 07 ParamPtr
    &[Col::Fixed(2), Col::Fixed(2), Col::Str],                   // 08 Param
    &[Col::Row(TYPE_DEF), TYPE_DEF_OR_REF],                      // 09 InterfaceImpl
    &[MEMBER_REF_PARENT, Col::Str, Col::Blob],                   // 0A MemberRef
    &[Col::Fixed(2), HAS_CONSTANT, Col::Blob],                   // 0B Constant
    &[HAS_CUSTOM_ATTRIBUTE, CUSTOM_ATTRIBUTE_TYPE, Col::Blob],   // 0C CustomAttribute
    &[HAS_FIELD_MARSHAL, Col::Blob],                             // 0D FieldMarshal
    &[Col::Fixed(2), HAS_DECL_SECURITY, Col::Blob],              // 0E DeclSecurity
    &[Col::Fixed(2), Col::Fixed(4), Col::Row(TYPE_DEF)],         // 0F ClassLayout
    &[Col::Fixed(4), Col::Row(0x04)],                            // 10 FieldLayout
    &[Col::Blob],                                                // 11 StandAloneSig
    &[Col::Row(TYPE_DEF), Col::Row(0x14)],                       // 12 EventMap
    &[Col::Row(0x14)],                                           // 13 EventPtr
    &[Col::Fixed(2), Col::Str, TYPE_DEF_OR_REF],                 // 14 Event
    &[Col::Row(TYPE_DEF), Col::Row(0x17)],                       // 15 PropertyMap
    &[Col::Row(0x17)],                                           // 16 PropertyPtr
    &[Col::Fixed(2), Col::Str, Col::Blob],                       // 17 Property
    &[Col::Fixed(2), Col::Row(METHOD_DEF), HAS_SEMANTICS],       // 18 MethodSemantics
    &[Col::Row(TYPE_DEF), METHOD_DEF_OR_REF, METHOD_DEF_OR_REF], // 19 MethodImpl
    &[Col::Str],                                                 // 1A ModuleRef
    &[Col::Blob],                                                // 1B TypeSpec
    &[Col::Fixed(2), MEMBER_FORWARDED, Col::Str, Col::Row(0x1a)], // 1C ImplMap
    &[Col::Fixed(4), Col::Row(0x04)],                            // 1D FieldRVA
    &[Col::Fixed(4), Col::Fixed(4)],                             // 1E EncLog
    &[Col::Fixed(4)],                                            // 1F EncMap
    &[
        Col::Fixed(4),
        Col::Fixed(8),
        Col::Fixed(4),
        Col::Blob,
        Col::Str,
        Col::Str,
    ], // 20 Assembly
    &[Col::Fixed(4)],                                            // 21 AssemblyProcessor
    &[Col::Fixed(12)],                                           // 22 AssemblyOS
    &[
        Col::Fixed(8),
        Col::Fixed(4),
        Col::Blob,
        Col::Str,
        Col::Str,
        Col::Blob,
    ], // 23 AssemblyRef
    &[Col::Fixed(4), Col::Row(ASSEMBLY_REF)],                    // 24 AssemblyRefProcessor
    &[Col::Fixed(12), Col::Row(ASSEMBLY_REF)],                   // 25 AssemblyRefOS
    &[Col::Fixed(4), Col::Str, Col::Blob],                       // 26 File
    &[
        Col::Fixed(4),
        Col::Fixed(4),
        Col::Str,
        Col::Str,
        IMPLEMENTATION,
    ], // 27 ExportedType
    &[Col::Fixed(4), Col::Fixed(4), Col::Str, IMPLEMENTATION],   // 28 ManifestResource
    &[Col::Row(TYPE_DEF), Col::Row(TYPE_DEF)],                   // 29 NestedClass
    &[Col::Fixed(2), Col::Fixed(2), TYPE_OR_METHOD_DEF, Col::Str], // 2A GenericParam
    &[METHOD_DEF_OR_REF, Col::Blob],                             // 2B MethodSpec
];
