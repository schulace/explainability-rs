use std::borrow::Cow;
use std::fmt::Debug;
use std::iter::once;
use std::ops::Add;
use std::vec;

use dot::{Edges, GraphWalk, Labeller, Nodes};

use typed_arena::Arena;

use serde::Serialize;

type Num = f32;

macro_rules! match_unordered {
    ($pattern1:pat, $pattern2:pat $(,)?) => {
        ($pattern1, $pattern2) | ($pattern2, $pattern1)
    };
}

#[derive(Default)]
struct OperationGraph<'a> {
    nodes: Vec<&'a Operation<'a>>,
    edges: Vec<(usize, usize)>,
}

impl<'a> OperationGraph<'a> {
    fn from_op(mut op: &'a Operation<'a>) -> OperationGraph<'a> {
        let mut nodes = Vec::with_capacity(op._allocator.len());
        nodes.push(op);
        let mut edges = Vec::with_capacity(op._allocator.len());
        let mut current_parent: usize = 0;
        use OperationType::*;
        loop {
            match &op.op {
                Sum { history, .. } => {
                    for &prior in history {
                        let position = nodes.len();
                        nodes.push(prior);
                        // edges are in data feed direction
                        edges.push((position, current_parent));
                    }
                }
                Source { .. } => {}
            };
            current_parent += 1;
            if current_parent >= nodes.len() {
                break;
            }
            op = nodes[current_parent];
        }
        OperationGraph { nodes, edges }
    }
}

impl<'a, 'b> GraphWalk<'b, &'b Operation<'a>, (usize, usize)> for OperationGraph<'a>
where
    'a: 'b,
{
    fn nodes(&'b self) -> Nodes<'b, &'b Operation<'a>> {
        Cow::Borrowed(&self.nodes)
    }
    fn edges(&'b self) -> Edges<'b, (usize, usize)> {
        Cow::Borrowed(&self.edges)
    }
    fn source(&'b self, edge: &(usize, usize)) -> &'b Operation<'a> {
        self.nodes[edge.0]
    }
    fn target(&'b self, edge: &(usize, usize)) -> &'b Operation<'a> {
        self.nodes[edge.1]
    }
}

impl<'a, 'b> Labeller<'b, &'b Operation<'a>, (usize, usize)> for OperationGraph<'a>
where
    'a: 'b,
{
    fn graph_id(&'b self) -> dot::Id<'b> {
        dot::Id::new("backtraced").unwrap()
    }
    fn node_id(&'b self, n: &&'b Operation<'a>) -> dot::Id<'b> {
        let n = *n;
        dot::Id::new(format!("op{n:p}")).unwrap()
    }
    fn node_label(&'b self, n: &&'b Operation<'a>) -> dot::LabelText<'b> {
        let n = *n;
        let variant = n.op.variant_symbol();
        let value = n.op.value();
        let reason = n.reason.map(|r| format!(" \"{r}\"")).unwrap_or_default();
        dot::LabelText::label(format!("{value}{variant}{reason}"))
    }
}

impl<'a, 'b> OperationGraph<'a>
where
    'a: 'b,
{
    fn to_graphviz(&'b self) -> String {
        let mut writer = vec![];
        dot::render(self, &mut writer).unwrap();
        String::from_utf8(writer).unwrap()
    }
}

#[derive(Serialize, Clone)]
pub struct Operation<'a> {
    op: OperationType<'a>,
    reason: Option<&'a str>,
    #[serde(skip)]
    _allocator: &'a Arena<Operation<'a>>,
}

impl<'a> Debug for Operation<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(reason) = self.reason {
            write!(f, "({:?}, {reason})", self.op)
        } else {
            write!(f, "{:?}", self.op)
        }
    }
}

impl<'a> Operation<'a> {
    pub fn new(i: Num, alloc: &'a Arena<Operation<'a>>) -> Self {
        Operation {
            op: OperationType::Source { value: i },
            reason: None,
            _allocator: alloc,
        }
    }
    pub fn new_with_reason(i: Num, reason: &'a str, alloc: &'a Arena<Operation<'a>>) -> Self {
        Operation {
            op: OperationType::Source { value: i },
            reason: Some(reason),
            _allocator: alloc,
        }
    }

    pub fn as_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn as_graphviz(&'a self) -> String {
        let graph = OperationGraph::from_op(self);
        graph.to_graphviz()
    }

    fn add_internal(&'a self, other: &'a Operation<'a>) -> &'a mut Self {
        use OperationType::*;
        match (self, other) {
            // Sum + Source
            // happy path: we have a summed one and we fold 1 more into it, tack it on, keep the
            // sum's reason
            match_unordered!(
                foldee @ Operation {
                    op: Source { value: a },
                    ..
                },
                Operation {
                    op: Sum { value: b, history },
                    reason,
                    ..
                },
            ) => self._allocator.alloc(Operation {
                op: OperationType::Sum {
                    value: a + b,
                    history: Vec::from_iter(history.iter().copied().chain(once(foldee))),
                },
                reason: *reason,
                _allocator: self._allocator,
            }),
            // 2 sources (just numbers) put together, no reason given, not gonna derive one
            (
                Operation {
                    op: Source { value: a },
                    ..
                },
                Operation {
                    op: Source { value: b },
                    ..
                },
            ) => self._allocator.alloc(Operation {
                op: Sum {
                    history: vec![self, other],
                    value: a + b,
                },
                reason: None,
                _allocator: self._allocator,
            }),
            // Sum + Sum, at least 1 with no reason. Fold them in and keep the chain short
            match_unordered!(
                Operation {
                    op: Sum {
                        value: a,
                        history: hist_a,
                    },
                    reason,
                    ..
                },
                Operation {
                    op: Sum {
                        value: b,
                        history: hist_b,
                    },
                    reason: None,
                    ..
                }
            ) => self._allocator.alloc(Operation {
                op: Sum {
                    history: hist_a
                        .iter()
                        .copied()
                        .chain(hist_b.iter().copied())
                        .collect(),
                    value: a + b,
                },
                reason: *reason,
                _allocator: self._allocator,
            }),
            // Sum 2 things with reasons for each, make a new sum with no reason, listing both
            // sources in the "history" since we're combining semantically different sums and
            // want to preserve the history
            (
                Operation {
                    op: Sum { value: a, .. },
                    reason: Some(_),
                    ..
                },
                Operation {
                    op: Sum { value: b, .. },
                    reason: Some(_),
                    ..
                },
            ) => self._allocator.alloc(Operation {
                op: Sum {
                    value: a + b,
                    history: vec![self, other],
                },
                reason: None,
                _allocator: self._allocator,
            }),
        }
    }
}

impl<'a> Add for &'a Operation<'a> {
    type Output = &'a Operation<'a>;
    fn add(self, other: Self) -> Self::Output {
        self.add_internal(other)
    }
}

type OpTuple<'a> = (&'a Operation<'a>, &'a str);
impl<'a> Add<OpTuple<'a>> for &'a Operation<'a> {
    type Output = &'a Operation<'a>;
    fn add(self, other: OpTuple<'a>) -> Self::Output {
        let (other, reason) = other;
        let reason = Some(reason);
        let res = self.add_internal(other);
        res.reason = reason;
        res
    }
}

#[derive(Serialize, Debug, Clone)]
enum OperationType<'a> {
    Source {
        value: Num,
    },
    Sum {
        value: Num,
        history: Vec<&'a Operation<'a>>,
    },
}

impl<'a> OperationType<'a> {
    fn variant_symbol(&self) -> &'static str {
        use OperationType::*;
        match self {
            Source { .. } => " ",
            Sum { .. } => " (+) ",
        }
    }

    fn value(&self) -> Num {
        use OperationType::*;
        match self {
            Source { value, .. } => *value,
            Sum { value, .. } => *value,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;
    #[test]
    fn test_sum_reasons() {
        fn within_point1(val: f32, target: f32) -> bool {
            target - 0.1 < val && val < target + 0.1
        }
        let arena = Arena::new();
        let a = Operation::new_with_reason(1.0, "a", &arena);
        let b = Operation::new(2.0, &arena);
        use OperationType::*;
        let a_plus_b = &a + &b;
        assert!(matches!(a_plus_b.op, Sum { .. }));
        assert!(matches!(a_plus_b.reason, None));
        let a_plus_b = &a + (&b, "b");
        assert!(matches!(a_plus_b.reason, Some("b")));
        let continuing_sum = a_plus_b + &a;
        assert!(
            matches!(&continuing_sum.op, Sum { value, history } if history.len() == 3 && *value < 4.1)
        );
        assert!(matches!(&continuing_sum.reason, Some("b")));
        let c = Operation::new_with_reason(3.0, "c", &arena);
        let continuing_sum = continuing_sum + &c;
        assert!(matches!(
            continuing_sum,
            Operation {
                op: Sum { history, value, .. },
                reason: Some("b"),
                ..
            } if matches!(history[..], [
                          Operation { op: Source{ .. }, reason: Some("a"), .. },
                          Operation { op: Source{ .. }, reason: None, .. },
                          Operation { op: Source{ .. }, reason: Some("a"), .. },
                          Operation { op: Source{ .. }, reason: Some("c"), .. }
            ]) && within_point1(*value, 7.)
        ));
        dbg!(continuing_sum);
        println!("{}", continuing_sum.as_json());
    }

    #[test]
    fn graph_render() {
        let arena = Arena::new();
        let a = Operation::new_with_reason(1.0, "a", &arena);
        let b = Operation::new(2.0, &arena);
        let a_plus_b = &a + (&b, "b");
        let continuing_sum = a_plus_b + &a;
        let c = Operation::new_with_reason(3.0, "c", &arena);
        let continuing_sum = continuing_sum + &c;
        let continuing_sum = continuing_sum + continuing_sum + &c;
        let mut file = std::fs::File::create("output.dot").unwrap();
        let graph = continuing_sum.as_graphviz();
        println!("{}", graph);
        file.write_all(graph.as_bytes()).unwrap();
        file.flush().unwrap();
    }
}
