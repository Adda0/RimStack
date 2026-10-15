use crate::rimworld::dotnet::{Assembly, Base, Callee, Layout, Step};
use rayon::prelude::*;
use std::collections::HashMap;

pub const NEVER: u16 = u16::MAX;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    next: Vec<Vec<u32>>,
}

type Named<'a> = HashMap<(usize, &'a str, &'a [u8]), usize>;

pub(crate) fn ancestors<'l>(layout: &'l Layout, of: usize) -> impl Iterator<Item = usize> + 'l {
    let mut at = of;
    // A damaged file may make a type extend itself: the walk is bounded.
    (0..64).map_while(move |_| match layout.types.get(at)?.extends {
        Base::Own(parent) => {
            at = parent;
            Some(parent)
        }
        _ => None,
    })
}

impl Graph {
    pub fn of(assembly: &Assembly, layout: &Layout) -> Self {
        let methods = &layout.methods;
        let mut next: Vec<Vec<u32>> = vec![Vec::new(); methods.len()];
        let all: Vec<Vec<Step>> = (0..methods.len())
            .into_par_iter()
            .map(|method| assembly.steps(layout, method))
            .collect();
        let mut kept: HashMap<usize, Vec<u32>> = HashMap::new();
        let mut reads: Vec<(usize, usize)> = Vec::new();
        for (from, steps) in all.into_iter().enumerate() {
            for step in steps {
                match step {
                    Step::Call(_, Callee::Own(to)) if to < methods.len() => {
                        if !next[from].contains(&(to as u32)) {
                            next[from].push(to as u32);
                        }
                    }
                    Step::Keeps { field, method } if method < methods.len() => {
                        kept.entry(field).or_default().push(method as u32);
                    }
                    Step::Reads(field) => reads.push((from, field)),
                    _ => {}
                }
            }
        }
        for (from, field) in reads {
            for to in kept.get(&field).into_iter().flatten() {
                if !next[from].contains(to) {
                    next[from].push(*to);
                }
            }
        }
        let named: Named = methods
            .iter()
            .enumerate()
            .filter(|(_, m)| m.is_virtual())
            .map(|(i, m)| ((m.owner, m.name, m.signature), i))
            .collect();
        let mut link = |from: usize, to: usize| {
            if from != to && !next[from].contains(&(to as u32)) {
                next[from].push(to as u32);
            }
        };
        for (declared, runs) in &layout.stand_ins {
            link(*declared, *runs);
        }
        for (owner, t) in layout.types.iter().enumerate() {
            let chain = || std::iter::once(owner).chain(ancestors(layout, owner));
            let mut for_interface: Vec<usize> = Vec::new();
            for interface in chain().flat_map(|c| &layout.types[c].implements) {
                for wanted in layout.types[*interface].methods.clone() {
                    let m = &methods[wanted];
                    let answer = chain().find_map(|c| named.get(&(c, m.name, m.signature)));
                    if let Some(answer) = answer {
                        link(wanted, *answer);
                        for_interface.push(*answer);
                    }
                }
            }
            let constructors: Vec<usize> = t
                .methods
                .clone()
                .filter(|m| methods[*m].name == ".ctor")
                .collect();
            for at in t.methods.clone().filter(|m| methods[*m].is_virtual()) {
                let m = &methods[at];
                let above = ancestors(layout, owner)
                    .find_map(|c| named.get(&(c, m.name, m.signature)))
                    .copied();
                if let Some(above) = above {
                    link(above, at);
                    continue;
                }
                let for_outside = !m.is_new() || m.is_final();
                if for_outside && !for_interface.contains(&at) {
                    constructors.iter().for_each(|c| link(*c, at));
                }
            }
        }
        Self { next }
    }

    /// `NEVER` where no root leads.
    pub fn steps_from(&self, roots: &[usize]) -> Vec<u16> {
        let mut steps = vec![NEVER; self.next.len()];
        let mut queue: std::collections::VecDeque<usize> = roots.iter().copied().collect();
        for root in roots {
            steps[*root] = 0;
        }
        while let Some(at) = queue.pop_front() {
            let further = steps[at].saturating_add(1).min(NEVER - 1);
            for to in self.next[at].iter().map(|to| *to as usize) {
                if steps[to] == NEVER {
                    steps[to] = further;
                    queue.push_back(to);
                }
            }
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rimworld::dotnet::build::{op, Builder, FINAL, NEW, VIRTUAL};

    const CALL: u16 = 0x28;
    const CALLVIRT: u16 = 0x6f;
    const NEWOBJ: u16 = 0x73;
    const METHOD: usize = 0x06;

    fn reached(dll: &[u8]) -> Vec<String> {
        let assembly = Assembly::read(dll).unwrap();
        let layout = assembly.layout();
        let steps = Graph::of(&assembly, &layout).steps_from(&[0]);
        let name = |at: usize| {
            let m = &layout.methods[at];
            format!("{}.{}", layout.types[m.owner].name, m.name)
        };
        let all = 0..layout.methods.len();
        all.filter(|at| steps[*at] != NEVER).map(name).collect()
    }

    #[test]
    fn a_call_leads_on_step_by_step() {
        let mut b = Builder::new();
        b.type_def("", "A", 0);
        b.method_with("Root", 0, &op(CALL, METHOD, 2));
        b.method_with("Near", 0, &op(CALL, METHOD, 3));
        b.method_with("Far", 0, &[0x2a]);
        b.method_with("Apart", 0, &op(CALL, METHOD, 3));
        let dll = b.finish();
        let assembly = Assembly::read(&dll).unwrap();
        let layout = assembly.layout();
        let graph = Graph::of(&assembly, &layout);
        assert_eq!(graph.steps_from(&[0]), [0, 1, 2, NEVER]);
        assert_eq!(graph.steps_from(&[]), [NEVER; 4]);
    }

    #[test]
    fn a_call_through_a_base_type_leads_to_what_subtypes_put_in_its_place() {
        let mut b = Builder::new();
        b.type_def("", "Code", 0);
        b.method_with("Root", 0, &op(CALLVIRT, METHOD, 2));
        let thing = b.type_def("", "Thing", 0);
        b.method_with("Tick", VIRTUAL | NEW, &[0x2a]);
        let pawn = b.type_def_under("", "Pawn", thing);
        b.method_with("Tick", VIRTUAL, &[0x2a]);
        b.method_with("Other", VIRTUAL | NEW, &[0x2a]);
        b.type_def_under("", "Colonist", pawn);
        b.method_with("Tick", VIRTUAL, &[0x2a]);
        assert_eq!(
            reached(&b.finish()),
            ["Code.Root", "Thing.Tick", "Pawn.Tick", "Colonist.Tick"]
        );
    }

    #[test]
    fn a_call_through_an_interface_leads_to_the_type_that_answers() {
        let mut b = Builder::new();
        b.type_def("", "Code", 0);
        b.method_with("Root", 0, &op(CALLVIRT, METHOD, 2));
        let interface = b.type_def("", "IWork", 0);
        b.method_with("Work", VIRTUAL | NEW, &[]);
        let base = b.type_def("", "Base", 0);
        b.method_with("Work", VIRTUAL | NEW | FINAL, &[0x2a]);
        let worker = b.type_def_under("", "Worker", base);
        b.implements(worker, interface);
        assert_eq!(
            reached(&b.finish()),
            ["Code.Root", "IWork.Work", "Base.Work"]
        );
    }

    #[test]
    fn a_delegate_kept_in_a_field_runs_from_whoever_reads_the_field() {
        let mut b = Builder::new();
        let action = b.type_ref("System", "Action");
        let wrap = b.constructor(action, &[0x20, 0, 0x01]);
        b.type_def("", "Window", 0);
        let kept = b.field("onGui");
        b.method_with("Draws", 0, &op(0x7b, 0x04, kept));
        let keeps = [
            op(0xfe06, METHOD, 3),
            op(NEWOBJ, 0x0a, wrap),
            op(0x7d, 0x04, kept),
        ]
        .concat();
        b.method_with("Opens", 0, &keeps);
        b.method_with("Inner", 0, &[0x2a]);
        let dll = b.finish();
        assert_eq!(reached(&dll), ["Window.Draws", "Window.Inner"]);
        let assembly = Assembly::read(&dll).unwrap();
        let layout = assembly.layout();
        assert_eq!(
            Graph::of(&assembly, &layout).steps_from(&[1]),
            [NEVER, 0, NEVER]
        );
    }

    #[test]
    fn making_an_object_leads_to_what_code_outside_calls_on_it() {
        let mut b = Builder::new();
        b.type_def("", "Code", 0);
        b.method_with("Root", 0, &op(NEWOBJ, METHOD, 2));
        b.type_def("", "Walker", 0);
        b.method_with(".ctor", 0, &[0x2a]);
        b.method_with("MoveNext", VIRTUAL | NEW | FINAL, &[0x2a]);
        b.method_with("ToString", VIRTUAL, &[0x2a]);
        b.method_with("Unused", VIRTUAL | NEW, &[0x2a]);
        assert_eq!(
            reached(&b.finish()),
            [
                "Code.Root",
                "Walker..ctor",
                "Walker.MoveNext",
                "Walker.ToString"
            ]
        );
    }

    #[test]
    fn a_type_said_to_extend_itself_does_not_hang_the_walk() {
        let mut b = Builder::new();
        b.type_def_under("", "Loop", 1);
        b.method_with("Run", VIRTUAL, &[0x2a]);
        assert_eq!(reached(&b.finish()), ["Loop.Run"]);
    }
}
