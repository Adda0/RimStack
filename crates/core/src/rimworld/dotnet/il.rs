//! ECMA-335 III: an opcode of one byte, or 0xFE plus one byte; the opcode
//! fixes the size of the operand after it.

pub const DUP: u16 = 0x25;
pub const CALL: u16 = 0x28;
pub const CALLVIRT: u16 = 0x6f;
pub const LDSTR: u16 = 0x72;
pub const NEWOBJ: u16 = 0x73;
pub const LDFLD: u16 = 0x7b;
pub const STFLD: u16 = 0x7d;
pub const LDSFLD: u16 = 0x7e;
pub const STSFLD: u16 = 0x80;
pub const LDTOKEN: u16 = 0xd0;
pub const LDFTN: u16 = 0xfe06;
pub const LDVIRTFTN: u16 = 0xfe07;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// Two-byte opcodes read as 0xFE00 plus their second byte.
    pub code: u16,
    /// Zero unless the operand is four bytes.
    pub operand: u32,
}

enum Operand {
    Bytes(usize),
    /// A count, then that many four-byte jumps.
    Switch,
}

fn operand(code: u16) -> Operand {
    Operand::Bytes(match code {
        0x45 => return Operand::Switch,
        0x0e..=0x13 | 0x1f | 0x2b..=0x37 | 0xde => 1,
        0x21 | 0x23 => 8,
        0x20 | 0x22 | 0x27..=0x29 | 0x38..=0x44 | 0x6f..=0x75 | 0x79 | 0x7b..=0x81 => 4,
        0x8c | 0x8d | 0x8f | 0xa3..=0xa5 | 0xc2 | 0xc6 | 0xd0 | 0xdd => 4,
        0xfe06 | 0xfe07 | 0xfe15 | 0xfe16 | 0xfe1c => 4,
        0xfe09..=0xfe0e => 2,
        0xfe12 | 0xfe19 => 1,
        _ => 0,
    })
}

/// Stops silently where the bytes end short of an instruction.
pub fn instructions(code: &[u8]) -> impl Iterator<Item = Instruction> + '_ {
    let mut at = 0;
    std::iter::from_fn(move || {
        let first = *code.get(at)? as u16;
        at += 1;
        let op = match first {
            0xfe => {
                at += 1;
                0xfe00 | *code.get(at - 1)? as u16
            }
            one => one,
        };
        let four = |at: usize| Some(u32::from_le_bytes(code.get(at..at + 4)?.try_into().ok()?));
        let (bytes, value) = match operand(op) {
            Operand::Switch => (4 + four(at)? as usize * 4, 0),
            Operand::Bytes(4) => (4, four(at)?),
            Operand::Bytes(bytes) => (bytes, 0),
        };
        at = at.checked_add(bytes).filter(|end| *end <= code.len())?;
        Some(Instruction {
            code: op,
            operand: value,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(bytes: &[u8]) -> Vec<(u16, u32)> {
        instructions(bytes).map(|i| (i.code, i.operand)).collect()
    }

    #[test]
    fn operands_are_stepped_over_by_their_size() {
        let code = [
            0x00, // nop
            0x1f, 0x28, // ldc.i4.s
            0x21, 0x28, 0, 0, 0, 0, 0, 0, 0, // ldc.i8
            0x28, 0x05, 0, 0, 0x0a, // call
            0xfe, 0x0c, 0x28, 0x00, // ldloc
            0xfe, 0x06, 0x07, 0, 0, 0x06, // ldftn
            0x2a, // ret
        ];
        assert_eq!(
            codes(&code),
            [
                (0x00, 0),
                (0x1f, 0),
                (0x21, 0),
                (CALL, 0x0a00_0005),
                (0xfe0c, 0),
                (LDFTN, 0x0600_0007),
                (0x2a, 0)
            ]
        );
    }

    #[test]
    fn a_switch_carries_its_jumps() {
        let code = [
            0x45, 2, 0, 0, 0, 0x28, 0, 0, 0, 0x28, 0, 0, 0, // switch
            0x6f, 1, 0, 0, 0x06, // callvirt
        ];
        assert_eq!(codes(&code), [(0x45, 0), (CALLVIRT, 0x0600_0001)]);
    }

    #[test]
    fn code_cut_short_ends_the_walk() {
        assert_eq!(codes(&[0x00, 0x28, 1, 0]), [(0x00, 0)]);
        assert_eq!(codes(&[0x45, 9, 0, 0, 0, 1]), []);
        assert_eq!(codes(&[0xfe]), []);
    }
}
