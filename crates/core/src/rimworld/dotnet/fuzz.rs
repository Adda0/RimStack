use super::build::{Builder, op, ser};
use super::{Assembly, MEMBER_REF, METHOD_DEF, il};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn sound() -> Vec<u8> {
    let mut b = Builder::new();
    b.reference("0Harmony");
    let patch = b.type_ref("HarmonyLib", "HarmonyPatch");
    let postfix = b.type_ref("HarmonyLib", "HarmonyPostfix");
    let widgets = b.type_ref("Verse", "Widgets");
    let label = b.member_ref(widgets, "Label");
    let system_type = b.type_ref("System", "Type");
    let class = (system_type << 2 | 1) as u8;
    let with_type = b.constructor(patch, &[0x20, 1, 0x01, 0x12, class]);
    let with_name = b.constructor(patch, &[0x20, 1, 0x01, 0x0e]);
    let is_postfix = b.constructor(postfix, &[0x20, 0, 0x01]);
    let outer = b.type_def("My", "Patches", 0);
    let mut aimed = vec![1, 0];
    aimed.extend(ser("RimWorld.ColonistBar"));
    b.attribute(false, outer, with_type, &[aimed, vec![0, 0]].concat());
    let inner = b.type_def("", "Inner", 0);
    b.nested(inner, outer);
    let after = b.method_with(
        "AfterDraw",
        0,
        &[op(il::CALL, MEMBER_REF, label), op(il::CALL, METHOD_DEF, 2)].concat(),
    );
    b.method_with("Helper", 0, &[0x2a]);
    b.user_string("Verse.Pawn");
    b.attribute(
        true,
        after,
        with_name,
        &[vec![1, 0], ser("DrawGUI"), vec![0, 0]].concat(),
    );
    b.attribute(true, after, is_postfix, &[1, 0, 0, 0]);
    b.finish()
}

fn damaged(sound: &[u8], round: usize, random: &mut Random) -> Vec<u8> {
    let mut dll = sound.to_vec();
    match round % 4 {
        0 => dll.truncate(random.below(dll.len())),
        1 => {
            // Counts and offsets are four bytes.
            for _ in 0..1 + random.below(4) {
                let at = random.below(dll.len() - 4);
                dll[at..at + 4].copy_from_slice(&[0xff, 0xff, 0xff, 0x7f]);
            }
        }
        _ => {
            for _ in 0..1 + random.below(24) {
                let at = random.below(dll.len());
                dll[at] = random.next() as u8;
            }
        }
    }
    dll
}

#[test]
fn a_damaged_assembly_is_refused_or_read_and_never_panics() {
    let sound = sound();
    let read = Assembly::read(&sound).unwrap();
    assert_eq!(
        read.patches().len(),
        1,
        "the sound assembly holds what is damaged"
    );
    let mut random = Random(0xd11);
    let quiet = std::panic::take_hook();
    let places = std::sync::Arc::new(std::sync::Mutex::new(std::collections::BTreeSet::new()));
    let into = places.clone();
    std::panic::set_hook(Box::new(move |info| {
        if let (Some(at), Ok(mut seen)) = (info.location(), into.lock()) {
            seen.insert(format!(
                "{}:{}: {}",
                at.file(),
                at.line(),
                info.payload_as_str().unwrap_or("")
            ));
        }
    }));
    let (mut panicked, mut still_read, mut slowest) = (0, 0, Duration::ZERO);
    for round in 0..4_000 {
        let dll = damaged(&sound, round, &mut random);
        let started = Instant::now();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let Some(assembly) = Assembly::read(&dll) else {
                return false;
            };
            let _ = assembly.references();
            let _ = assembly.types();
            let _ = assembly.patches();
            let _ = assembly.hand_applied();
            let _ = assembly.looked_up();
            let layout = assembly.layout();
            for at in 0..layout.methods.len() {
                let _ = assembly.steps(&layout, at);
            }
            true
        }));
        slowest = slowest.max(started.elapsed());
        match outcome {
            Ok(true) => still_read += 1,
            Ok(false) => {}
            Err(_) => panicked += 1,
        }
    }
    std::panic::set_hook(quiet);
    assert_eq!(panicked, 0, "panicked at {:?}", places.lock().unwrap());
    assert!(
        still_read > 0,
        "some damage leaves an assembly that still reads"
    );
    assert!(
        slowest < Duration::from_millis(500),
        "one reading took {slowest:?}"
    );
}
