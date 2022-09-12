use std::borrow::Cow;

use dot::{Edges, GraphWalk, Labeller, Nodes};

use crate::Operation;
use crate::OperationType;

#[derive(Default)]
pub struct OperationGraph<'a, Num> {
    nodes: Vec<&'a Operation<'a, Num>>,
    edges: Vec<(usize, usize)>,
}

impl<'a, Num> OperationGraph<'a, Num> {
    pub(crate) fn from_op(mut op: &'a Operation<'a, Num>) -> OperationGraph<'a, Num> {
        let mut nodes = Vec::with_capacity(op._allocator.len());
        nodes.push(op);
        let mut edges = Vec::with_capacity(op._allocator.len());
        let mut current_parent: usize = 0;
        use OperationType::*;
        loop {
            match &op.op {
                Source { .. } => {}
                node => {
                    for &prior in node.history() {
                        let position = nodes
                            .iter()
                            .enumerate()
                            .find(|(_, &i_op)| std::ptr::eq(i_op, prior))
                            .map(|(idx, _)| idx)
                            .unwrap_or_else(|| {
                                nodes.push(prior);
                                nodes.len() - 1
                            });
                        // edges are in data feed direction
                        edges.push((position, current_parent));
                    }
                }
            };
            current_parent += 1;
            if current_parent >= nodes.len() {
                break;
            }
            op = nodes[current_parent];
        }
        edges.sort();
        edges.dedup();
        OperationGraph { nodes, edges }
    }
}

impl<'a, 'b, Num> GraphWalk<'b, &'b Operation<'a, Num>, (usize, usize)> for OperationGraph<'a, Num>
where
    'a: 'b,
{
    fn nodes(&'b self) -> Nodes<'b, &'b Operation<'a, Num>> {
        Cow::Borrowed(&self.nodes)
    }
    fn edges(&'b self) -> Edges<'b, (usize, usize)> {
        Cow::Borrowed(&self.edges)
    }
    fn source(&'b self, edge: &(usize, usize)) -> &'b Operation<'a, Num> {
        self.nodes[edge.0]
    }
    fn target(&'b self, edge: &(usize, usize)) -> &'b Operation<'a, Num> {
        self.nodes[edge.1]
    }
}

impl<'a, 'b, Num> Labeller<'b, &'b Operation<'a, Num>, (usize, usize)> for OperationGraph<'a, Num>
where
    'a: 'b,
    Num: std::fmt::Display,
{
    fn graph_id(&'b self) -> dot::Id<'b> {
        dot::Id::new("backtraced").unwrap()
    }
    fn node_id(&'b self, n: &&'b Operation<'a, Num>) -> dot::Id<'b> {
        let n = *n;
        dot::Id::new(format!("op{n:p}")).unwrap()
    }
    fn node_label(&'b self, n: &&'b Operation<'a, Num>) -> dot::LabelText<'b> {
        let n = *n;
        let variant = n.op.variant_symbol();
        let value = n.op.value();
        let reason = n
            .reason
            .as_ref()
            .map(|r| format!(" \"{r}\""))
            .unwrap_or_default();
        dot::LabelText::label(format!("{value}{variant}{reason}"))
    }
}

impl<'a, 'b, Num> OperationGraph<'a, Num>
where
    'a: 'b,
    Num: std::fmt::Display,
{
    pub(crate) fn to_graphviz(&'b self) -> String {
        let mut writer = vec![];
        dot::render(self, &mut writer).unwrap();
        String::from_utf8(writer).unwrap()
    }
}
